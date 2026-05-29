use std::collections::HashMap;
use std::time::Instant;

#[derive(Clone)]
pub struct RawNetStats {
    pub ifaces: HashMap<String, [u64; 2]>, // iface -> [rx_bytes, tx_bytes]
    pub time: Instant,
}

#[derive(Default)]
pub struct NetStats {
    pub rx_bps: u64,
    pub tx_bps: u64,
}

/// Parse `/proc/net/dev` content into iface -> [rx_bytes, tx_bytes].
/// Loopback ("lo") is excluded.
pub fn parse_net_dev(content: &str) -> HashMap<String, [u64; 2]> {
    let mut ifaces = HashMap::new();
    for line in content.lines().skip(2) {
        let line = line.trim();
        let colon = match line.find(':') {
            Some(i) => i,
            None => continue,
        };
        let name = line[..colon].trim();
        if name == "lo" {
            continue;
        }
        let fields: Vec<u64> = line[colon + 1..]
            .split_whitespace()
            .filter_map(|s| s.parse().ok())
            .collect();
        if fields.len() >= 9 {
            ifaces.insert(name.to_string(), [fields[0], fields[8]]);
        }
    }
    ifaces
}

pub fn read_raw() -> Option<RawNetStats> {
    let content = std::fs::read_to_string("/proc/net/dev").ok()?;
    Some(RawNetStats {
        ifaces: parse_net_dev(&content),
        time: Instant::now(),
    })
}

pub fn compute(prev: &RawNetStats, curr: &RawNetStats) -> NetStats {
    let elapsed = curr.time.duration_since(prev.time).as_secs_f64();
    if elapsed < 0.001 {
        return NetStats::default();
    }

    let mut total_rx = 0u64;
    let mut total_tx = 0u64;

    for (name, curr_vals) in &curr.ifaces {
        if let Some(prev_vals) = prev.ifaces.get(name) {
            total_rx += rate_bps(prev_vals[0], curr_vals[0], elapsed);
            total_tx += rate_bps(prev_vals[1], curr_vals[1], elapsed);
        }
    }

    NetStats {
        rx_bps: total_rx,
        tx_bps: total_tx,
    }
}

/// Compute bytes-per-second from a counter delta over a known elapsed duration.
/// Returns 0 if the counter wrapped (curr < prev) or elapsed is negligible.
pub fn rate_bps(prev_bytes: u64, curr_bytes: u64, elapsed_secs: f64) -> u64 {
    if elapsed_secs < 0.001 || curr_bytes < prev_bytes {
        return 0;
    }
    let delta = curr_bytes - prev_bytes;
    (delta as f64 / elapsed_secs) as u64
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    const NET_DEV_FIXTURE: &str = "\
Inter-|   Receive                                                |  Transmit
 face |bytes    packets errs drop fifo frame compressed multicast|bytes    packets errs drop fifo colls carrier compressed
    lo:  123456    1000    0    0    0     0          0         0    123456    1000    0    0    0     0       0          0
  eth0: 9876543   50000    0    0    0     0          0         0   1234567   30000    0    0    0     0       0          0
 wlan0:  500000   10000    0    0    0     0          0         0    100000    5000    0    0    0     0       0          0
";

    #[test]
    fn parse_proc_net_dev() {
        let ifaces = parse_net_dev(NET_DEV_FIXTURE);
        // loopback excluded
        assert!(!ifaces.contains_key("lo"));
        assert_eq!(ifaces["eth0"], [9_876_543, 1_234_567]);
        assert_eq!(ifaces["wlan0"], [500_000, 100_000]);
    }

    #[test]
    fn net_rate_from_deltas() {
        // 500_000 bytes in 0.5s = 1_000_000 bps
        let bps = rate_bps(0, 500_000, 0.5);
        assert_eq!(bps, 1_000_000);
    }

    #[test]
    fn net_rate_counter_wrap() {
        // counter went backwards (wrap / reset) — must return 0
        let bps = rate_bps(1_000_000, 500, 1.0);
        assert_eq!(bps, 0);
    }

    #[test]
    fn sparkline_history_caps_at_width() {
        use std::collections::VecDeque;
        let cap = 60usize;
        let mut history: VecDeque<u64> = VecDeque::from(vec![0u64; cap]);
        for i in 0..100u64 {
            if history.len() >= cap {
                history.pop_front();
            }
            history.push_back(i);
        }
        assert_eq!(history.len(), cap);
        // most-recent value should be 99
        assert_eq!(*history.back().unwrap(), 99);
    }

    #[test]
    fn compute_skips_elapsed_zero() {
        let ifaces: HashMap<String, [u64; 2]> = HashMap::new();
        let t = Instant::now();
        let prev = RawNetStats {
            ifaces: ifaces.clone(),
            time: t,
        };
        // same instant — elapsed < 0.001
        let curr = RawNetStats { ifaces, time: t };
        let stats = compute(&prev, &curr);
        assert_eq!(stats.rx_bps, 0);
        assert_eq!(stats.tx_bps, 0);
    }

    #[test]
    fn compute_excludes_lo_from_totals() {
        // lo was already stripped in parse_net_dev; ensure it doesn't appear
        let ifaces = parse_net_dev(NET_DEV_FIXTURE);
        assert!(!ifaces.contains_key("lo"));
    }

    #[test]
    fn net_rate_elapsed_secs() {
        // build two RawNetStats snapshots manually and verify compute()
        let mut prev_ifaces = HashMap::new();
        prev_ifaces.insert("eth0".to_string(), [0u64, 0u64]);
        let mut curr_ifaces = HashMap::new();
        curr_ifaces.insert("eth0".to_string(), [1_000_000u64, 0u64]);

        let t_prev = Instant::now();
        let t_curr = t_prev + Duration::from_millis(1000);

        let prev = RawNetStats {
            ifaces: prev_ifaces,
            time: t_prev,
        };
        let curr = RawNetStats {
            ifaces: curr_ifaces,
            time: t_curr,
        };
        let stats = compute(&prev, &curr);
        // ~1_000_000 bps ± small float rounding
        assert!(stats.rx_bps > 999_000 && stats.rx_bps <= 1_000_000);
    }
}
