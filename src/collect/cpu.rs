// /proc/stat fields per line: user nice system idle iowait irq softirq steal guest guest_nice
#[derive(Default, Clone)]
pub struct RawCpuStats {
    pub aggregate: [u64; 10],
    pub cores: Vec<[u64; 10]>,
}

#[derive(Default)]
pub struct CpuStats {
    pub total_pct: f64,
    pub per_core_pct: Vec<f64>,
}

fn parse_cpu_line(line: &str) -> Option<[u64; 10]> {
    let mut parts = line.split_whitespace();
    parts.next(); // skip "cpuN" / "cpu" label
    let mut vals = [0u64; 10];
    for v in vals.iter_mut() {
        *v = parts.next()?.parse().ok()?;
    }
    Some(vals)
}

pub fn parse_raw(content: &str) -> Option<RawCpuStats> {
    let mut aggregate = [0u64; 10];
    let mut cores = Vec::new();
    let mut found_aggregate = false;

    for line in content.lines() {
        if !line.starts_with("cpu") {
            continue;
        }
        let suffix = &line[3..];
        if suffix.starts_with(' ') {
            // aggregate "cpu " line
            if let Some(vals) = parse_cpu_line(line) {
                aggregate = vals;
                found_aggregate = true;
            }
        } else if suffix.starts_with(|c: char| c.is_ascii_digit()) {
            if let Some(vals) = parse_cpu_line(line) {
                cores.push(vals);
            }
        }
    }

    if !found_aggregate && cores.is_empty() {
        return None;
    }

    Some(RawCpuStats { aggregate, cores })
}

pub fn read_raw() -> Option<RawCpuStats> {
    let content = std::fs::read_to_string("/proc/stat").ok()?;
    parse_raw(&content)
}

fn core_pct(prev: &[u64; 10], curr: &[u64; 10]) -> f64 {
    let prev_idle = prev[3] + prev[4]; // idle + iowait
    let curr_idle = curr[3] + curr[4];
    let prev_total: u64 = prev.iter().sum();
    let curr_total: u64 = curr.iter().sum();

    let delta_total = curr_total.saturating_sub(prev_total);
    let delta_idle = curr_idle.saturating_sub(prev_idle);

    if delta_total == 0 {
        return 0.0;
    }

    let busy = delta_total.saturating_sub(delta_idle);
    (busy as f64 / delta_total as f64) * 100.0
}

pub fn compute(prev: Option<&RawCpuStats>, curr: &RawCpuStats) -> CpuStats {
    let Some(prev) = prev else {
        return CpuStats {
            total_pct: 0.0,
            per_core_pct: vec![0.0; curr.cores.len()],
        };
    };

    let per_core_pct: Vec<f64> = prev
        .cores
        .iter()
        .zip(curr.cores.iter())
        .map(|(p, c)| core_pct(p, c))
        .collect();

    // Use the aggregate line for total accuracy; fall back to core average.
    let total_pct = if curr.cores.is_empty() || prev.cores.is_empty() {
        core_pct(&prev.aggregate, &curr.aggregate)
    } else {
        per_core_pct.iter().sum::<f64>() / per_core_pct.len() as f64
    };

    CpuStats {
        total_pct,
        per_core_pct,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const FIXTURE: &str = "\
cpu  100 10 50 800 20 5 5 0 0 0
cpu0  50  5 25 400 10 2 3 0 0 0
cpu1  50  5 25 400 10 3 2 0 0 0
intr 12345 0 0
ctxt 67890
";

    #[test]
    fn parse_proc_stat() {
        let r = parse_raw(FIXTURE).unwrap();
        assert_eq!(r.aggregate, [100, 10, 50, 800, 20, 5, 5, 0, 0, 0]);
    }

    #[test]
    fn parse_proc_stat_per_core() {
        let r = parse_raw(FIXTURE).unwrap();
        assert_eq!(r.cores.len(), 2);
        assert_eq!(r.cores[0], [50, 5, 25, 400, 10, 2, 3, 0, 0, 0]);
        assert_eq!(r.cores[1], [50, 5, 25, 400, 10, 3, 2, 0, 0, 0]);
    }

    #[test]
    fn cpu_usage_from_deltas() {
        let prev = RawCpuStats {
            aggregate: [100, 0, 0, 800, 0, 0, 0, 0, 0, 0],
            cores: vec![[100, 0, 0, 800, 0, 0, 0, 0, 0, 0]],
        };
        let curr = RawCpuStats {
            aggregate: [200, 0, 0, 850, 0, 0, 0, 0, 0, 0],
            cores: vec![[200, 0, 0, 850, 0, 0, 0, 0, 0, 0]],
        };
        // delta_total = (200+850) - (100+800) = 150, delta_idle = 850-800 = 50
        // busy = 100, pct = 100/150 * 100 ≈ 66.67%
        let stats = compute(Some(&prev), &curr);
        let expected = (100.0_f64 / 150.0) * 100.0;
        assert!((stats.total_pct - expected).abs() < 0.01);
    }

    #[test]
    fn cpu_usage_no_change() {
        let raw = RawCpuStats {
            aggregate: [100, 0, 0, 900, 0, 0, 0, 0, 0, 0],
            cores: vec![[100, 0, 0, 900, 0, 0, 0, 0, 0, 0]],
        };
        let stats = compute(Some(&raw.clone()), &raw);
        assert_eq!(stats.total_pct, 0.0);
        assert!(!stats.total_pct.is_nan());
    }

    #[test]
    fn cpu_usage_all_busy() {
        let prev = RawCpuStats {
            aggregate: [0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
            cores: vec![[0, 0, 0, 0, 0, 0, 0, 0, 0, 0]],
        };
        let curr = RawCpuStats {
            aggregate: [1000, 0, 0, 0, 0, 0, 0, 0, 0, 0],
            cores: vec![[1000, 0, 0, 0, 0, 0, 0, 0, 0, 0]],
        };
        let stats = compute(Some(&prev), &curr);
        assert!((stats.total_pct - 100.0).abs() < 0.01);
    }
}
