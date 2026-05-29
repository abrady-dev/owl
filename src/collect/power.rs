use std::fs;
use std::path::Path;

#[derive(Default)]
pub struct PowerStats {
    pub battery_pct: Option<u8>,
    pub status: Option<String>,
}

/// Parse a battery capacity string ("87") into a ratio in [0.0, 1.0].
/// Returns None for blank or unparseable input.
pub fn parse_capacity(s: &str) -> Option<f64> {
    let pct: u8 = s.trim().parse().ok()?;
    Some(pct as f64 / 100.0)
}

pub fn read() -> Option<PowerStats> {
    for bat in &["BAT0", "BAT1", "BAT2"] {
        let base = format!("/sys/class/power_supply/{}", bat);
        if !Path::new(&base).exists() {
            continue;
        }

        let capacity = fs::read_to_string(format!("{}/capacity", base))
            .ok()
            .and_then(|s| parse_capacity(&s).map(|r| (r * 100.0) as u8));

        let status = fs::read_to_string(format!("{}/status", base))
            .map(|s| s.trim().to_string())
            .ok();

        return Some(PowerStats {
            battery_pct: capacity,
            status,
        });
    }

    Some(PowerStats::default())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_battery_capacity() {
        let ratio = parse_capacity("87").unwrap();
        assert!((ratio - 0.87).abs() < 1e-9);
    }

    #[test]
    fn parse_battery_capacity_with_newline() {
        let ratio = parse_capacity("100\n").unwrap();
        assert!((ratio - 1.0).abs() < 1e-9);
    }

    #[test]
    fn parse_battery_missing() {
        assert!(parse_capacity("").is_none());
        assert!(parse_capacity("N/A").is_none());
        assert!(parse_capacity("abc").is_none());
    }

    #[test]
    fn parse_battery_zero() {
        let ratio = parse_capacity("0").unwrap();
        assert_eq!(ratio, 0.0);
    }
}
