use std::collections::BTreeMap;
use std::time::{Duration, Instant};

#[derive(Debug, Clone, Default)]
pub struct NetworkReading {
    pub interface: String,
    pub received_per_second: Option<f64>,
    pub sent_per_second: Option<f64>,
    pub received_this_session: u64,
    pub sent_this_session: u64,
}

#[derive(Clone, Copy)]
struct Counters {
    received: u64,
    sent: u64,
    received_this_session: u64,
    sent_this_session: u64,
    present: bool,
}

#[derive(Default)]
pub struct NetworkSampler {
    previous: BTreeMap<String, Counters>,
    taken_at: Option<Instant>,
}

impl NetworkSampler {
    pub fn sample(&mut self, contents: &str, now: Instant) -> Vec<NetworkReading> {
        let elapsed = self.taken_at.map(|before| now.saturating_duration_since(before));
        let mut current = BTreeMap::new();
        let mut readings = Vec::new();

        for (interface, received, sent) in parse_dev(contents) {
            let before = self.previous.get(&interface).copied();
            let delta = before.filter(|before| before.present).and_then(|before| {
                Some((received.checked_sub(before.received)?, sent.checked_sub(before.sent)?))
            });
            let (received_this_session, sent_this_session) = match (before, delta) {
                (Some(before), Some((rx, tx))) => (
                    before.received_this_session.saturating_add(rx),
                    before.sent_this_session.saturating_add(tx),
                ),
                (Some(before), None) => (before.received_this_session, before.sent_this_session),
                (None, _) => (0, 0),
            };
            let rate = match (delta, elapsed) {
                (Some((rx, tx)), Some(elapsed)) if valid_gap(elapsed) => {
                    let seconds = elapsed.as_secs_f64();
                    (Some(rx as f64 / seconds), Some(tx as f64 / seconds))
                }
                _ => (None, None),
            };
            current.insert(
                interface.clone(),
                Counters {
                    received,
                    sent,
                    received_this_session,
                    sent_this_session,
                    present: true,
                },
            );
            readings.push(NetworkReading {
                interface,
                received_per_second: rate.0,
                sent_per_second: rate.1,
                received_this_session,
                sent_this_session,
            });
        }

        for (interface, mut before) in
            self.previous.iter().map(|(name, counters)| (name.clone(), *counters))
        {
            current.entry(interface).or_insert_with(|| {
                before.present = false;
                before
            });
        }

        self.previous = current;
        self.taken_at = Some(now);
        readings
    }
}

fn valid_gap(elapsed: Duration) -> bool {
    elapsed >= Duration::from_millis(100) && elapsed <= Duration::from_secs(10)
}

fn parse_dev(contents: &str) -> Vec<(String, u64, u64)> {
    let mut interfaces = Vec::new();
    for line in contents.lines() {
        let Some((name, columns)) = line.rsplit_once(':') else { continue };
        let interface = name.trim();
        if interface.is_empty() || !user_interface(interface) {
            continue;
        }
        let values: Vec<_> = columns.split_whitespace().collect();
        if values.len() < 16 {
            continue;
        }
        let (Ok(received), Ok(sent)) = (values[0].parse(), values[8].parse()) else { continue };
        interfaces.push((interface.to_owned(), received, sent));
    }
    interfaces
}

fn user_interface(name: &str) -> bool {
    name != "lo"
        && !["veth", "br-", "docker", "virbr", "podman", "cni", "flannel"]
            .iter()
            .any(|prefix| name.starts_with(prefix))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture(rx: u64, tx: u64) -> String {
        format!(
            "Inter-| Receive | Transmit\n face |bytes packets errs drop fifo frame compressed multicast|bytes packets errs drop fifo colls carrier compressed\n  lo: 10 0 0 0 0 0 0 0 10 0 0 0 0 0 0 0\n wlan0: {rx} 0 0 0 0 0 0 0 {tx} 0 0 0 0 0 0 0\n"
        )
    }

    #[test]
    fn first_sample_reset_and_long_gap_never_invent_a_rate() {
        let mut sampler = NetworkSampler::default();
        let start = Instant::now();
        let first = sampler.sample(&fixture(100, 200), start);
        assert_eq!(first.len(), 1);
        assert_eq!(first[0].received_per_second, None);

        let next = sampler.sample(&fixture(300, 260), start + Duration::from_secs(2));
        assert_eq!(next[0].received_per_second, Some(100.0));
        assert_eq!(next[0].sent_per_second, Some(30.0));
        assert_eq!(next[0].received_this_session, 200);

        let reset = sampler.sample(&fixture(5, 10), start + Duration::from_secs(4));
        assert_eq!(reset[0].received_per_second, None);
        assert_eq!(reset[0].received_this_session, 200);

        let late = sampler.sample(&fixture(105, 110), start + Duration::from_secs(30));
        assert_eq!(late[0].received_per_second, None);
        assert_eq!(late[0].received_this_session, 300);
    }

    #[test]
    fn a_missing_interface_loses_its_baseline() {
        let mut sampler = NetworkSampler::default();
        let start = Instant::now();
        sampler.sample(&fixture(100, 100), start);
        assert!(sampler.sample("", start + Duration::from_secs(2)).is_empty());
        let back = sampler.sample(&fixture(500, 500), start + Duration::from_secs(4));
        assert_eq!(back[0].received_per_second, None);
        assert_eq!(back[0].received_this_session, 0);
    }

    #[test]
    fn a_reappearing_interface_keeps_its_session_total() {
        let mut sampler = NetworkSampler::default();
        let start = Instant::now();
        sampler.sample(&fixture(100, 100), start);
        let before = sampler.sample(&fixture(300, 250), start + Duration::from_secs(2));
        assert_eq!(before[0].received_this_session, 200);
        sampler.sample("", start + Duration::from_secs(4));
        let back = sampler.sample(&fixture(900, 900), start + Duration::from_secs(6));
        assert_eq!(back[0].received_per_second, None);
        assert_eq!(back[0].received_this_session, 200);
        assert_eq!(back[0].sent_this_session, 150);
    }

    #[test]
    fn container_links_do_not_fill_the_persons_network_list() {
        assert!(!user_interface("vethd21e122"));
        assert!(!user_interface("br-4995d5ae91bb"));
        assert!(!user_interface("docker0"));
        assert!(user_interface("eno1"));
        assert!(user_interface("wlan0"));
        assert!(user_interface("tailscale0"));
    }
}
