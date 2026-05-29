// /proc/stat fields per core: user nice system idle iowait irq softirq steal guest guest_nice
#[derive(Default, Clone)]
pub struct RawCpuStats {
    pub cores: Vec<[u64; 10]>,
}

#[derive(Default)]
pub struct CpuStats {
    pub total_pct: f64,
    pub per_core_pct: Vec<f64>,
}

fn parse_cpu_line(line: &str) -> Option<[u64; 10]> {
    let mut parts = line.split_whitespace();
    parts.next(); // skip "cpuN" label
    let mut vals = [0u64; 10];
    for v in vals.iter_mut() {
        *v = parts.next()?.parse().ok()?;
    }
    Some(vals)
}

pub fn read_raw() -> Option<RawCpuStats> {
    let content = std::fs::read_to_string("/proc/stat").ok()?;
    let mut cores = Vec::new();

    for line in content.lines() {
        if line.starts_with("cpu") {
            let suffix = &line[3..];
            // Only individual core lines (cpu0, cpu1, ...), not the aggregate "cpu " line
            if suffix.starts_with(|c: char| c.is_ascii_digit()) {
                if let Some(vals) = parse_cpu_line(line) {
                    cores.push(vals);
                }
            }
        }
    }

    // Fall back to the aggregate line if no per-core entries were found
    if cores.is_empty() {
        for line in content.lines() {
            if line.starts_with("cpu ") {
                if let Some(vals) = parse_cpu_line(line) {
                    return Some(RawCpuStats { cores: vec![vals] });
                }
            }
        }
        return None;
    }

    Some(RawCpuStats { cores })
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

    let total_pct = if per_core_pct.is_empty() {
        0.0
    } else {
        per_core_pct.iter().sum::<f64>() / per_core_pct.len() as f64
    };

    CpuStats {
        total_pct,
        per_core_pct,
    }
}
