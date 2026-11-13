//! What each process draws from an amdgpu card, read from the DRM client statistics the
//! driver prints into `/proc/<pid>/fdinfo`.

use std::collections::HashMap;

/// One DRM client as one file descriptor reports it. Several descriptors, and after a fork
/// several processes, can report the same client.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ClientReading {
    pub client_id: u64,
    pub driver: Option<String>,
    pub pci_slot: Option<String>,
    /// Busy time per engine, cumulative. amdgpu leaves out an engine it has never used, so a
    /// missing engine is zero rather than unknown.
    pub engines: Vec<(String, u64)>,
    pub vram_bytes: Option<u64>,
}

pub(crate) fn parse_fdinfo(contents: &str) -> Option<ClientReading> {
    let mut client_id = None;
    let mut driver = None;
    let mut pci_slot = None;
    let mut engines = Vec::new();
    let mut total_vram = None;
    let mut legacy_vram = None;
    for line in contents.lines() {
        let Some((name, value)) = line.split_once(':') else {
            continue;
        };
        let text = || Some(value.trim().to_owned()).filter(|text| !text.is_empty());
        match name {
            "drm-client-id" => client_id = value.split_whitespace().next()?.parse().ok(),
            "drm-driver" => driver = text(),
            "drm-pdev" => pci_slot = text(),
            "drm-total-vram" => total_vram = memory_bytes(value),
            "drm-memory-vram" => legacy_vram = memory_bytes(value),
            // `drm-engine-capacity-<name>` is a count of rings, not time.
            name if name.starts_with("drm-engine-")
                && !name.starts_with("drm-engine-capacity-") =>
            {
                if let Some(ns) = value.split_whitespace().next().and_then(|v| v.parse().ok()) {
                    engines.push((name["drm-engine-".len()..].to_owned(), ns));
                }
            }
            _ => {}
        }
    }
    Some(ClientReading {
        client_id: client_id?,
        driver,
        pci_slot,
        engines,
        // `drm-memory-*` is the older spelling of the same figure; a kernel that prints both
        // prints them equal, and one that prints only the old key is still read.
        vram_bytes: total_vram.or(legacy_vram),
    })
}

/// DRM memory keys carry a unit, and the documentation makes it bytes when there is none.
/// amdgpu prints a bare `0` for an empty region.
fn memory_bytes(value: &str) -> Option<u64> {
    let mut parts = value.split_whitespace();
    let amount: u64 = parts.next()?.parse().ok()?;
    let scale = match parts.next() {
        None => 1,
        Some("KiB") => 1024,
        Some("MiB") => 1024 * 1024,
        Some("GiB") => 1024 * 1024 * 1024,
        Some(_) => return None,
    };
    amount.checked_mul(scale)
}

/// A client is one GPU context, so it is counted once whichever descriptors and processes
/// report it. It is credited to the lowest pid holding it: a forked child inherits its
/// parent's descriptor, and the parent is the one that opened it.
#[derive(Default)]
pub(crate) struct ClientLedger {
    /// Keyed by card and client, since client ids are only unique within one device.
    holders: HashMap<(usize, u64), (u32, ClientReading)>,
}

impl ClientLedger {
    pub fn record(&mut self, card: usize, pid: u32, reading: ClientReading) {
        let holder = self.holders.entry((card, reading.client_id)).or_insert((pid, reading));
        if pid < holder.0 {
            holder.0 = pid;
        }
    }

    /// Per pid and card: busy fraction since the previous call and VRAM held. `previous`
    /// holds each client's engine counters from the last sample and is replaced with this
    /// sample's.
    pub fn settle(
        self,
        previous: &mut HashMap<(usize, u64), Vec<(String, u64)>>,
        elapsed_ns: Option<u64>,
    ) -> HashMap<(u32, usize), (Option<f32>, Option<u64>)> {
        let mut shares: HashMap<(u32, usize), (Option<f32>, Option<u64>)> = HashMap::new();
        let mut counters = HashMap::with_capacity(self.holders.len());
        for ((card, client), (pid, reading)) in self.holders {
            let share = shares.entry((pid, card)).or_default();
            if let Some(vram) = reading.vram_bytes {
                share.1 = Some(share.1.unwrap_or(0).saturating_add(vram));
            }
            if let (Some(before), Some(elapsed)) =
                (previous.get(&(card, client)), elapsed_ns.filter(|ns| *ns > 0))
            {
                let busy = busiest_engine(before, &reading.engines, elapsed);
                share.0 = Some(share.0.unwrap_or(0.0) + busy);
            }
            counters.insert((card, client), reading.engines);
        }
        for share in shares.values_mut() {
            share.0 = share.0.map(|busy| busy.clamp(0.0, 1.0));
        }
        *previous = counters;
        shares
    }
}

/// The busiest engine rather than the sum: the engines run side by side, so a client
/// rendering and decoding video at once would otherwise read as more than the whole card.
/// This is the same question `gpu_busy_percent` answers for the card.
fn busiest_engine(before: &[(String, u64)], now: &[(String, u64)], elapsed_ns: u64) -> f32 {
    now.iter()
        .map(|(engine, ns)| {
            let earlier = before.iter().find(|(name, _)| name == engine).map_or(0, |(_, ns)| *ns);
            ns.saturating_sub(earlier) as f64 / elapsed_ns as f64
        })
        .fold(0.0_f64, f64::max) as f32
}

#[cfg(test)]
mod tests {
    use super::*;

    /// As this machine's kernel prints it for an idle client: no engine lines at all.
    const IDLE: &str = "pos:\t0\nflags:\t0100002\nmnt_id:\t36\nino:\t1020\ndrm-driver:\tamdgpu\n\
        drm-client-id:\t238\ndrm-pdev:\t0000:0c:00.0\npasid:\t117\ndrm-total-cpu:\t0\n\
        drm-total-gtt:\t2 MiB\ndrm-total-vram:\t12 KiB\ndrm-memory-vram:\t12 KiB\n\
        drm-memory-gtt: \t2048 KiB\namd-requested-vram:\t12 KiB\n";

    #[test]
    fn an_idle_client_reads_its_vram_and_no_engines() {
        let reading = parse_fdinfo(IDLE).unwrap();
        assert_eq!(reading.client_id, 238);
        assert_eq!(reading.driver.as_deref(), Some("amdgpu"));
        assert_eq!(reading.pci_slot.as_deref(), Some("0000:0c:00.0"));
        assert!(reading.engines.is_empty());
        assert_eq!(reading.vram_bytes, Some(12 * 1024));
    }

    #[test]
    fn engines_are_read_by_name_and_capacity_is_not_an_engine() {
        let reading = parse_fdinfo(
            "drm-driver:\tamdgpu\ndrm-client-id:\t7\ndrm-engine-gfx:\t120 ns\n\
             drm-engine-compute:\t80 ns\ndrm-engine-capacity-gfx:\t2\ndrm-memory-vram:\t3 MiB\n",
        )
        .unwrap();
        assert_eq!(reading.engines, vec![("gfx".into(), 120), ("compute".into(), 80)]);
        assert_eq!(reading.vram_bytes, Some(3 * 1024 * 1024));
    }

    #[test]
    fn a_descriptor_without_a_client_is_not_a_client() {
        assert!(parse_fdinfo("pos:\t0\nflags:\t0100002\nmnt_id:\t36\nino:\t474\n").is_none());
    }

    #[test]
    fn memory_units_follow_the_drm_documentation() {
        assert_eq!(memory_bytes(" 0"), Some(0));
        assert_eq!(memory_bytes(" 8"), Some(8));
        assert_eq!(memory_bytes(" 2 MiB"), Some(2 * 1024 * 1024));
        assert_eq!(memory_bytes(" 2 parsecs"), None);
    }

    fn client(id: u64, gfx_ns: u64, vram: u64) -> ClientReading {
        ClientReading {
            client_id: id,
            driver: Some("amdgpu".into()),
            pci_slot: None,
            engines: vec![("gfx".into(), gfx_ns)],
            vram_bytes: Some(vram),
        }
    }

    #[test]
    fn a_shared_client_is_counted_once_for_the_lowest_pid() {
        let mut previous = HashMap::new();
        let mut first = ClientLedger::default();
        first.record(0, 201, client(9, 0, 100));
        first.record(0, 200, client(9, 0, 100));
        let shares = first.settle(&mut previous, None);
        assert_eq!(shares[&(200, 0)], (None, Some(100)), "engine time needs two samples");
        assert!(!shares.contains_key(&(201, 0)));

        let mut second = ClientLedger::default();
        second.record(0, 201, client(9, 25_000_000, 100));
        second.record(0, 200, client(9, 25_000_000, 100));
        let shares = second.settle(&mut previous, Some(100_000_000));
        let (busy, vram) = shares[&(200, 0)];
        assert!((busy.unwrap() - 0.25).abs() < 1e-6);
        assert_eq!(vram, Some(100));
    }

    #[test]
    fn the_same_client_number_on_two_cards_is_two_clients() {
        let mut ledger = ClientLedger::default();
        ledger.record(0, 300, client(1, 0, 10));
        ledger.record(1, 300, client(1, 0, 20));
        let shares = ledger.settle(&mut HashMap::new(), None);
        assert_eq!(shares[&(300, 0)].1, Some(10));
        assert_eq!(shares[&(300, 1)].1, Some(20));
    }

    #[test]
    fn engines_running_together_read_as_the_busiest_one() {
        let before = vec![("gfx".to_owned(), 0), ("dec".to_owned(), 0)];
        let now = vec![("gfx".to_owned(), 60), ("dec".to_owned(), 90), ("enc".to_owned(), 10)];
        assert!((busiest_engine(&before, &now, 100) - 0.9).abs() < 1e-6);
    }
}
