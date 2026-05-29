#[derive(Default)]
pub struct MemStats {
    pub total_kb: u64,
    pub used_kb: u64,
    pub swap_total_kb: u64,
    pub swap_used_kb: u64,
}

impl MemStats {
    pub fn used_ratio(&self) -> f64 {
        if self.total_kb == 0 {
            return 0.0;
        }
        self.used_kb as f64 / self.total_kb as f64
    }
}

pub fn parse(raw: &str) -> MemStats {
    let mut total = 0u64;
    let mut available = 0u64;
    let mut swap_total = 0u64;
    let mut swap_free = 0u64;

    for line in raw.lines() {
        let mut parts = line.split_whitespace();
        let key = match parts.next() {
            Some(k) => k,
            None => continue,
        };
        let val: u64 = match parts.next().and_then(|v| v.parse().ok()) {
            Some(v) => v,
            None => continue,
        };

        match key {
            "MemTotal:" => total = val,
            "MemAvailable:" => available = val,
            "SwapTotal:" => swap_total = val,
            "SwapFree:" => swap_free = val,
            _ => {}
        }
    }

    MemStats {
        total_kb: total,
        used_kb: total.saturating_sub(available),
        swap_total_kb: swap_total,
        swap_used_kb: swap_total.saturating_sub(swap_free),
    }
}

pub fn read() -> Option<MemStats> {
    let content = std::fs::read_to_string("/proc/meminfo").ok()?;
    Some(parse(&content))
}

#[cfg(test)]
mod tests {
    use super::*;

    const FIXTURE: &str = "\
MemTotal:       16384000 kB
MemFree:         2048000 kB
MemAvailable:    8192000 kB
Buffers:          512000 kB
Cached:          4096000 kB
SwapCached:            0 kB
Active:          6144000 kB
Inactive:        3072000 kB
Active(anon):    2048000 kB
Inactive(anon):   512000 kB
SwapTotal:       8192000 kB
SwapFree:        7680000 kB
Dirty:              1024 kB
Writeback:             0 kB
";

    #[test]
    fn parse_real_meminfo() {
        let s = parse(FIXTURE);
        assert_eq!(s.total_kb, 16_384_000);
        assert_eq!(s.used_kb, 16_384_000 - 8_192_000);
        assert_eq!(s.swap_total_kb, 8_192_000);
        assert_eq!(s.swap_used_kb, 8_192_000 - 7_680_000);
    }

    #[test]
    fn parse_missing_available() {
        let raw = "MemTotal: 8192000 kB\nMemFree: 1024000 kB\n";
        let s = parse(raw);
        assert_eq!(s.total_kb, 8_192_000);
        // used = total - 0 when MemAvailable is absent
        assert_eq!(s.used_kb, 8_192_000);
    }

    #[test]
    fn parse_empty() {
        let s = parse("");
        assert_eq!(s.total_kb, 0);
        assert_eq!(s.used_kb, 0);
        assert_eq!(s.used_ratio(), 0.0);
    }

    #[test]
    fn used_ratio_zero_total() {
        let s = MemStats {
            total_kb: 0,
            used_kb: 0,
            ..Default::default()
        };
        assert_eq!(s.used_ratio(), 0.0);
        assert!(!s.used_ratio().is_nan());
    }
}
