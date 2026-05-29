use std::fs;

pub struct TempSensor {
    pub name: String,
    pub temp_c: f64,
}

#[derive(Default)]
pub struct ThermalStats {
    pub sensors: Vec<TempSensor>,
}

/// Parse a raw millidegrees Celsius string (e.g. "45000") into °C.
pub fn parse_temp_millideg(s: &str) -> Option<f64> {
    let millideg: i64 = s.trim().parse().ok()?;
    if millideg <= 0 {
        return None;
    }
    Some(millideg as f64 / 1000.0)
}

/// Preference rank for hwmon chip names: higher = more preferred.
/// Prefer real CPU sensors over generic ACPI ones.
pub fn hwmon_priority(name: &str) -> u8 {
    match name {
        "coretemp" => 4,
        "k10temp" => 4,
        "amdgpu" => 3,
        "nct6775" | "nct6776" | "nct6779" => 2,
        "acpitz" => 1,
        _ => 0,
    }
}

/// Given a list of (hwmon_name, temperature_°C) pairs, return a reference to
/// the entry from the highest-priority chip. Ties broken by higher temp.
#[allow(dead_code)]
pub fn select_preferred(sources: &[(String, f64)]) -> Option<&(String, f64)> {
    sources.iter().max_by(|(a_name, a_temp), (b_name, b_temp)| {
        let pa = hwmon_priority(a_name);
        let pb = hwmon_priority(b_name);
        pa.cmp(&pb).then(
            a_temp
                .partial_cmp(b_temp)
                .unwrap_or(std::cmp::Ordering::Equal),
        )
    })
}

pub fn read() -> Option<ThermalStats> {
    let mut sensors = Vec::new();

    let dir = match fs::read_dir("/sys/class/hwmon") {
        Ok(d) => d,
        Err(_) => return Some(ThermalStats::default()),
    };

    for entry in dir.flatten() {
        let hwmon_path = entry.path();

        let hw_name = fs::read_to_string(hwmon_path.join("name"))
            .map(|s| s.trim().to_string())
            .unwrap_or_else(|_| {
                hwmon_path
                    .file_name()
                    .map(|n| n.to_string_lossy().into_owned())
                    .unwrap_or_default()
            });

        let files = match fs::read_dir(&hwmon_path) {
            Ok(f) => f,
            Err(_) => continue,
        };

        for file_entry in files.flatten() {
            let fname = file_entry.file_name();
            let fname = fname.to_string_lossy();

            if !fname.starts_with("temp") || !fname.ends_with("_input") {
                continue;
            }

            let raw = match fs::read_to_string(file_entry.path()).ok() {
                Some(s) => s,
                None => continue,
            };

            let temp_c = match parse_temp_millideg(&raw) {
                Some(t) => t,
                None => continue,
            };

            let label_path = file_entry
                .path()
                .to_string_lossy()
                .replace("_input", "_label");
            let label = fs::read_to_string(&label_path)
                .map(|s| s.trim().to_string())
                .unwrap_or_else(|_| fname.replace("_input", ""));

            sensors.push(TempSensor {
                name: format!("{}/{}", hw_name, label),
                temp_c,
            });
        }
    }

    // Sort by chip priority first (preferred sources at top), then hottest within each source.
    sensors.sort_by(|a, b| {
        let pa = hwmon_priority(a.name.split('/').next().unwrap_or(""));
        let pb = hwmon_priority(b.name.split('/').next().unwrap_or(""));
        pb.cmp(&pa).then(
            b.temp_c
                .partial_cmp(&a.temp_c)
                .unwrap_or(std::cmp::Ordering::Equal),
        )
    });

    Some(ThermalStats { sensors })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_hwmon_temp() {
        assert_eq!(parse_temp_millideg("45000"), Some(45.0));
        assert_eq!(parse_temp_millideg("45000\n"), Some(45.0));
        assert_eq!(parse_temp_millideg("1000"), Some(1.0));
    }

    #[test]
    fn parse_hwmon_temp_zero_or_negative() {
        assert_eq!(parse_temp_millideg("0"), None);
        assert_eq!(parse_temp_millideg("-1000"), None);
    }

    #[test]
    fn parse_hwmon_temp_invalid() {
        assert_eq!(parse_temp_millideg(""), None);
        assert_eq!(parse_temp_millideg("notanumber"), None);
    }

    #[test]
    fn select_preferred_hwmon() {
        let sources = vec![
            ("acpitz".to_string(), 45.0_f64),
            ("coretemp".to_string(), 52.0_f64),
            ("k10temp".to_string(), 48.0_f64),
        ];
        let best = select_preferred(&sources).unwrap();
        // coretemp and k10temp both have priority 4; coretemp wins on higher temp
        assert_eq!(best.0, "coretemp");
    }

    #[test]
    fn select_preferred_single() {
        let sources = vec![("acpitz".to_string(), 38.0_f64)];
        let best = select_preferred(&sources).unwrap();
        assert_eq!(best.0, "acpitz");
    }

    #[test]
    fn select_preferred_empty() {
        let sources: Vec<(String, f64)> = vec![];
        assert!(select_preferred(&sources).is_none());
    }
}
