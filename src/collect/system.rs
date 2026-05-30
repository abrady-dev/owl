use std::fs;

pub fn hostname() -> String {
    fs::read_to_string("/etc/hostname")
        .map(|s| s.trim().to_string())
        .unwrap_or_else(|_| "unknown".to_string())
}

pub fn uptime_secs() -> u64 {
    fs::read_to_string("/proc/uptime")
        .ok()
        .and_then(|s| s.split_whitespace().next()?.parse::<f64>().ok())
        .map(|s| s as u64)
        .unwrap_or(0)
}

pub fn loadavg() -> (f64, f64, f64) {
    let s = fs::read_to_string("/proc/loadavg").unwrap_or_default();
    let mut p = s.split_whitespace();
    let l1 = p.next().and_then(|v| v.parse().ok()).unwrap_or(0.0f64);
    let l5 = p.next().and_then(|v| v.parse().ok()).unwrap_or(0.0f64);
    let l15 = p.next().and_then(|v| v.parse().ok()).unwrap_or(0.0f64);
    (l1, l5, l15)
}

pub fn cpu_model() -> String {
    fs::read_to_string("/proc/cpuinfo")
        .ok()
        .and_then(|c| {
            c.lines()
                .find(|l| l.starts_with("model name"))
                .and_then(|l| l.splitn(2, ':').nth(1))
                .map(|s| s.trim().to_string())
        })
        .unwrap_or_else(|| "CPU".to_string())
}

pub fn primary_iface() -> String {
    let content = fs::read_to_string("/proc/net/dev").unwrap_or_default();
    for line in content.lines().skip(2) {
        let line = line.trim();
        if let Some(colon) = line.find(':') {
            let name = line[..colon].trim();
            if name != "lo" {
                return name.to_string();
            }
        }
    }
    String::new()
}

pub fn local_time_str() -> String {
    unsafe {
        let t = libc::time(std::ptr::null_mut());
        let mut tm: libc::tm = std::mem::zeroed();
        libc::localtime_r(&t, &mut tm);
        format!("{:02}:{:02}:{:02}", tm.tm_hour, tm.tm_min, tm.tm_sec)
    }
}

pub fn fmt_uptime(secs: u64) -> String {
    let days = secs / 86400;
    let hours = (secs % 86400) / 3600;
    let mins = (secs % 3600) / 60;
    if days > 0 {
        format!("{}d {:02}:{:02}", days, hours, mins)
    } else {
        format!("{:02}:{:02}", hours, mins)
    }
}
