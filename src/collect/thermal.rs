use std::fs;

pub struct TempSensor {
    pub name: String,
    pub temp_c: f64,
}

#[derive(Default)]
pub struct ThermalStats {
    pub sensors: Vec<TempSensor>,
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

            let millideg: i64 = match fs::read_to_string(file_entry.path())
                .ok()
                .and_then(|s| s.trim().parse().ok())
            {
                Some(v) => v,
                None => continue,
            };

            if millideg <= 0 {
                continue;
            }

            let temp_c = millideg as f64 / 1000.0;

            // Try to get a human-readable label for this specific temp input
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

    // Hottest sensors first
    sensors.sort_by(|a, b| {
        b.temp_c
            .partial_cmp(&a.temp_c)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    Some(ThermalStats { sensors })
}
