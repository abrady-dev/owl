use std::fs;
use std::path::Path;

#[derive(Default)]
pub struct PowerStats {
    pub battery_pct: Option<u8>,
    pub status: Option<String>,
}

pub fn read() -> Option<PowerStats> {
    for bat in &["BAT0", "BAT1", "BAT2"] {
        let base = format!("/sys/class/power_supply/{}", bat);
        if !Path::new(&base).exists() {
            continue;
        }

        let capacity = fs::read_to_string(format!("{}/capacity", base))
            .ok()
            .and_then(|s| s.trim().parse::<u8>().ok());

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
