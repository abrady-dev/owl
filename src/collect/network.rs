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

pub fn read_raw() -> Option<RawNetStats> {
    let content = std::fs::read_to_string("/proc/net/dev").ok()?;
    let mut ifaces = HashMap::new();

    // First two lines are headers
    for line in content.lines().skip(2) {
        let line = line.trim();
        let colon = line.find(':')?;
        let name = line[..colon].trim().to_string();
        let fields: Vec<u64> = line[colon + 1..]
            .split_whitespace()
            .filter_map(|s| s.parse().ok())
            .collect();
        // rx_bytes is field 0, tx_bytes is field 8
        if fields.len() >= 9 {
            ifaces.insert(name, [fields[0], fields[8]]);
        }
    }

    Some(RawNetStats {
        ifaces,
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
        if name == "lo" {
            continue;
        }
        if let Some(prev_vals) = prev.ifaces.get(name) {
            let rx_delta = curr_vals[0].saturating_sub(prev_vals[0]);
            let tx_delta = curr_vals[1].saturating_sub(prev_vals[1]);
            total_rx += (rx_delta as f64 / elapsed) as u64;
            total_tx += (tx_delta as f64 / elapsed) as u64;
        }
    }

    NetStats {
        rx_bps: total_rx,
        tx_bps: total_tx,
    }
}
