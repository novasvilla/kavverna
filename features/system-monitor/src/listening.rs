//! Which of the current user's programs are waiting for connections, read from the kernel's
//! socket tables and matched to processes through the socket inodes their descriptors hold.
//! Read when asked rather than on a timer: the descriptor walk is the expensive part, and the
//! answer changes when a program starts listening, not every two seconds.

use crate::processes::ProcessKey;
use crate::processes::process_name;
use crate::processes::procfs::parse_stat_line;
use std::collections::HashMap;
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};
use std::os::unix::fs::MetadataExt;
use std::path::Path;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Transport {
    Tcp,
    Udp,
}

/// Who can reach the socket, which is the question a port list exists to answer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Exposure {
    /// Bound to every interface, `0.0.0.0` or `::`: the network can reach it.
    Everywhere,
    /// Only this machine.
    Loopback,
    /// One address of one interface.
    Interface,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SocketHolder {
    pub key: ProcessKey,
    pub command: String,
    pub name: String,
    pub icon: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ListeningSocket {
    pub transport: Transport,
    pub address: IpAddr,
    pub port: u16,
    pub exposure: Exposure,
    /// Usually one. A forked server shares its socket with its workers, and a socket passed in
    /// by a socket-activating manager can have none of this user's processes holding it.
    pub holders: Vec<SocketHolder>,
}

pub fn listening_sockets() -> Vec<ListeningSocket> {
    listening_sockets_in(Path::new("/proc"), nix::unistd::geteuid().as_raw())
}

/// Reads `net/{tcp,tcp6,udp,udp6}` and the processes under `proc_root`, keeping only sockets
/// owned by `uid`.
pub fn listening_sockets_in(proc_root: &Path, uid: u32) -> Vec<ListeningSocket> {
    let mut sockets: Vec<(u64, ListeningSocket)> = Vec::new();
    for (file, transport) in [
        ("tcp", Transport::Tcp),
        ("tcp6", Transport::Tcp),
        ("udp", Transport::Udp),
        ("udp6", Transport::Udp),
    ] {
        let Ok(table) = std::fs::read_to_string(proc_root.join("net").join(file)) else {
            continue;
        };
        for row in table.lines().skip(1).filter_map(|line| parse_row(line, transport)) {
            if row.uid == uid && row.listening {
                sockets.push((
                    row.inode,
                    ListeningSocket {
                        transport,
                        address: row.address,
                        port: row.port,
                        exposure: exposure(row.address),
                        holders: Vec::new(),
                    },
                ));
            }
        }
    }
    if sockets.is_empty() {
        return Vec::new();
    }
    let wanted: HashMap<u64, usize> =
        sockets.iter().enumerate().map(|(at, (inode, _))| (*inode, at)).collect();
    for (inode, holder) in holders(proc_root, uid, &wanted) {
        let socket = &mut sockets[wanted[&inode]].1;
        if !socket.holders.iter().any(|known| known.key == holder.key) {
            socket.holders.push(holder);
        }
    }
    let mut sockets: Vec<ListeningSocket> = sockets.into_iter().map(|(_, socket)| socket).collect();
    sockets.sort_by(|left, right| {
        (left.port, left.transport, left.address).cmp(&(right.port, right.transport, right.address))
    });
    sockets
}

fn holders(proc_root: &Path, uid: u32, wanted: &HashMap<u64, usize>) -> Vec<(u64, SocketHolder)> {
    let mut found = Vec::new();
    let Ok(entries) = std::fs::read_dir(proc_root) else {
        return found;
    };
    for entry in entries.flatten() {
        let Some(pid) = entry.file_name().to_str().and_then(|name| name.parse::<u32>().ok()) else {
            continue;
        };
        if !entry.metadata().is_ok_and(|metadata| metadata.uid() == uid) {
            continue;
        }
        let Ok(descriptors) = std::fs::read_dir(entry.path().join("fd")) else {
            continue;
        };
        let mut inodes: Vec<u64> = descriptors
            .flatten()
            .filter_map(|descriptor| std::fs::read_link(descriptor.path()).ok())
            .filter_map(|target| socket_inode(target.to_str()?))
            .filter(|inode| wanted.contains_key(inode))
            .collect();
        if inodes.is_empty() {
            continue;
        }
        inodes.dedup();
        let Some(line) = std::fs::read_to_string(entry.path().join("stat"))
            .ok()
            .and_then(|text| parse_stat_line(&text))
        else {
            continue;
        };
        let executable = std::fs::read_link(entry.path().join("exe")).ok();
        let (name, icon) = process_name(proc_root, pid, &line.command, executable);
        let holder = SocketHolder {
            key: ProcessKey { pid, started: line.started },
            name,
            icon,
            command: line.command,
        };
        found.extend(inodes.into_iter().map(|inode| (inode, holder.clone())));
    }
    found
}

fn socket_inode(target: &str) -> Option<u64> {
    target.strip_prefix("socket:[")?.strip_suffix(']')?.parse().ok()
}

struct Row {
    address: IpAddr,
    port: u16,
    listening: bool,
    uid: u32,
    inode: u64,
}

/// `sl local_address rem_address st tx:rx tr:when retrnsmt uid timeout inode ...`
fn parse_row(line: &str, transport: Transport) -> Option<Row> {
    let fields: Vec<&str> = line.split_ascii_whitespace().collect();
    let (address, port) = endpoint(fields.get(1)?)?;
    let (remote, _) = endpoint(fields.get(2)?)?;
    let state = u8::from_str_radix(fields.get(3)?, 16).ok()?;
    let listening = match transport {
        // TCP_LISTEN.
        Transport::Tcp => state == 0x0A,
        // UDP has no listening state. A bound socket with no peer, TCP_CLOSE in the table, is
        // one that takes datagrams from anyone, which is the same thing for this question.
        Transport::Udp => state == 0x07 && remote.is_unspecified(),
    };
    Some(Row {
        address,
        port,
        listening,
        uid: fields.get(7)?.parse().ok()?,
        inode: fields.get(9)?.parse().ok()?,
    })
}

/// The kernel prints each 32-bit word of the address as hex in host byte order, and the port
/// as a plain hex number.
fn endpoint(text: &str) -> Option<(IpAddr, u16)> {
    let (address, port) = text.split_once(':')?;
    let port = u16::from_str_radix(port, 16).ok()?;
    let word = |at: usize| u32::from_str_radix(address.get(at..at + 8)?, 16).ok();
    let address = match address.len() {
        8 => IpAddr::V4(Ipv4Addr::from(word(0)?.to_ne_bytes())),
        32 => {
            let mut bytes = [0_u8; 16];
            for chunk in 0..4 {
                bytes[chunk * 4..chunk * 4 + 4].copy_from_slice(&word(chunk * 8)?.to_ne_bytes());
            }
            IpAddr::V6(Ipv6Addr::from(bytes))
        }
        _ => return None,
    };
    Some((address, port))
}

fn exposure(address: IpAddr) -> Exposure {
    let address = match address {
        IpAddr::V6(v6) => v6.to_ipv4_mapped().map_or(IpAddr::V6(v6), IpAddr::V4),
        v4 => v4,
    };
    if address.is_unspecified() {
        Exposure::Everywhere
    } else if address.is_loopback() {
        Exposure::Loopback
    } else {
        Exposure::Interface
    }
}

#[cfg(all(test, target_endian = "little"))]
mod tests {
    use super::*;

    const HEADER: &str = "  sl  local_address rem_address   st tx_queue rx_queue tr tm->when retrnsmt   uid  timeout inode\n";

    fn table(rows: &[&str]) -> String {
        let mut text = HEADER.to_owned();
        for row in rows {
            text.push_str(row);
            text.push('\n');
        }
        text
    }

    #[test]
    fn addresses_are_read_in_the_kernels_byte_order() {
        assert_eq!(endpoint("0100007F:0035"), Some(("127.0.0.1".parse().unwrap(), 53)));
        assert_eq!(endpoint("00000000:1F90"), Some(("0.0.0.0".parse().unwrap(), 8080)));
        assert_eq!(
            endpoint("00000000000000000000000001000000:0277"),
            Some(("::1".parse().unwrap(), 631))
        );
        assert_eq!(
            endpoint("0000000000000000FFFF00000100007F:0016"),
            Some(("::ffff:127.0.0.1".parse().unwrap(), 22))
        );
    }

    #[test]
    fn exposure_tells_the_network_from_this_machine() {
        assert_eq!(exposure("0.0.0.0".parse().unwrap()), Exposure::Everywhere);
        assert_eq!(exposure("::".parse().unwrap()), Exposure::Everywhere);
        assert_eq!(exposure("127.0.0.1".parse().unwrap()), Exposure::Loopback);
        assert_eq!(exposure("::1".parse().unwrap()), Exposure::Loopback);
        assert_eq!(exposure("::ffff:127.0.0.1".parse().unwrap()), Exposure::Loopback);
        assert_eq!(exposure("192.168.1.20".parse().unwrap()), Exposure::Interface);
    }

    #[test]
    fn only_this_users_listening_sockets_are_listed_with_who_holds_them() {
        let room = tempfile::tempdir().unwrap();
        let root = room.path();
        let uid = nix::unistd::geteuid().as_raw();
        let other = uid + 1;
        std::fs::create_dir_all(root.join("net")).unwrap();
        std::fs::write(
            root.join("net/tcp"),
            table(&[
                &format!("   0: 00000000:1F90 00000000:0000 0A 00000000:00000000 00:00000000 00000000  {uid}        0 1111 1 0"),
                &format!("   1: 0100007F:0035 00000000:0000 0A 00000000:00000000 00:00000000 00000000  {other}        0 2222 1 0"),
                &format!("   2: 0100007F:9C40 0100007F:1F90 01 00000000:00000000 00:00000000 00000000  {uid}        0 3333 1 0"),
            ]),
        )
        .unwrap();
        std::fs::write(
            root.join("net/udp6"),
            table(&[
                &format!("   0: 00000000000000000000000001000000:14E9 00000000000000000000000000000000:0000 07 00000000:00000000 00:00000000 00000000  {uid}        0 4444 2 0"),
            ]),
        )
        .unwrap();
        for (pid, command, socket) in
            [(700, "devserver", 1111), (701, "devserver", 1111), (702, "mdns", 4444)]
        {
            let dir = root.join(pid.to_string());
            std::fs::create_dir_all(dir.join("fd")).unwrap();
            std::fs::write(
                dir.join("stat"),
                crate::processes::fixture::stat_text(pid, command, 1, 0, 70 + u64::from(pid)),
            )
            .unwrap();
            std::os::unix::fs::symlink(format!("socket:[{socket}]"), dir.join("fd/3")).unwrap();
            std::os::unix::fs::symlink("/dev/null", dir.join("fd/0")).unwrap();
        }

        let sockets = listening_sockets_in(root, uid);
        assert_eq!(sockets.len(), 2, "another user's socket and a connection are left out");
        assert_eq!(sockets[0].port, 5353);
        assert_eq!(sockets[0].transport, Transport::Udp);
        assert_eq!(sockets[0].exposure, Exposure::Loopback);
        assert_eq!(sockets[0].holders[0].command, "mdns");
        assert_eq!(sockets[1].port, 8080);
        assert_eq!(sockets[1].exposure, Exposure::Everywhere);
        let pids: Vec<u32> = sockets[1].holders.iter().map(|holder| holder.key.pid).collect();
        assert_eq!(pids.len(), 2);
        assert!(pids.contains(&700) && pids.contains(&701));
        assert_eq!(
            sockets[1].holders[0].key.started,
            70 + u64::from(sockets[1].holders[0].key.pid)
        );
    }
}
