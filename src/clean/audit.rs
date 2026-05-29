use std::fs::{self, OpenOptions};
use std::io::{self, Write};
use std::path::PathBuf;
use std::time::SystemTime;

use super::manifest::RunMode;

/// Path to the append-only audit log.
pub fn audit_log_path() -> Option<PathBuf> {
    let home = std::env::var_os("HOME").map(PathBuf::from)?;
    Some(home.join(".local/state/owl/audit.log"))
}

/// Append one line to the audit log.
///
/// The log is created (including parent directories) on first write.
/// Each entry is a single line: `<ISO-8601 UTC>  <mode>  <message>`.
pub fn append_audit_entry(mode: RunMode, message: &str) -> io::Result<()> {
    let path = audit_log_path()
        .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "$HOME is not set"))?;

    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }

    let mut file = OpenOptions::new().create(true).append(true).open(&path)?;
    let ts = utc_timestamp();
    writeln!(file, "{ts}  {mode}  {message}")?;
    Ok(())
}

// ── Timestamp ───────────────────────────────────────────────────────────────

fn utc_timestamp() -> String {
    let secs = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    let (y, mo, d, h, mi, s) = epoch_to_ymd_hms(secs);
    format!("{y:04}-{mo:02}-{d:02}T{h:02}:{mi:02}:{s:02}Z")
}

/// Convert a Unix epoch (seconds) to (year, month, day, hour, minute, second) UTC.
///
/// Uses the civil-calendar algorithm from Howard Hinnant's date library
/// (<https://howardhinnant.github.io/date_algorithms.html>).
fn epoch_to_ymd_hms(secs: u64) -> (u64, u64, u64, u64, u64, u64) {
    let ss = secs % 60;
    let mi = (secs / 60) % 60;
    let hh = (secs / 3600) % 24;
    let days = secs / 86400;

    // Shift epoch to 0000-03-01 so leap days fall at year boundaries.
    let z = days + 719_468;
    let era = z / 146_097;
    let doe = z - era * 146_097; // day-of-era [0, 146096]
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365; // year-of-era [0, 399]
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100); // day-of-year [0, 365]
    let mp = (5 * doy + 2) / 153; // month-of-year (March=0) [0, 11]
    let d = doy - (153 * mp + 2) / 5 + 1; // day [1, 31]
    let mo = if mp < 10 { mp + 3 } else { mp - 9 }; // month [1, 12]
    let y = if mo <= 2 { y + 1 } else { y };

    (y, mo, d, hh, mi, ss)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::BufRead;

    #[test]
    fn epoch_to_ymd_hms_known_dates() {
        // 2024-01-01 00:00:00 UTC = 1704067200
        let (y, mo, d, h, mi, s) = epoch_to_ymd_hms(1_704_067_200);
        assert_eq!((y, mo, d, h, mi, s), (2024, 1, 1, 0, 0, 0));

        // 2000-03-01 12:30:45 UTC = 951_913_845
        let (y, mo, d, h, mi, s) = epoch_to_ymd_hms(951_913_845);
        assert_eq!((y, mo, d), (2000, 3, 1));
        assert_eq!((h, mi, s), (12, 30, 45));

        // Unix epoch itself: 1970-01-01 00:00:00
        let (y, mo, d, h, mi, s) = epoch_to_ymd_hms(0);
        assert_eq!((y, mo, d, h, mi, s), (1970, 1, 1, 0, 0, 0));
    }

    #[test]
    fn append_creates_file_and_appends() {
        let tmp = std::env::temp_dir().join("owl_audit_test_append");
        let log = tmp.join("audit.log");
        let _ = fs::remove_dir_all(&tmp);
        fs::create_dir_all(&tmp).unwrap();

        // Write via OpenOptions directly (bypassing HOME env) to test the core logic.
        let write = |mode: &str, msg: &str| {
            let mut f = OpenOptions::new()
                .create(true)
                .append(true)
                .open(&log)
                .unwrap();
            writeln!(f, "2024-01-01T00:00:00Z  {mode}  {msg}").unwrap();
        };

        write("DRY_RUN", "first entry");
        write("EXECUTE", "second entry");

        let lines: Vec<String> = io::BufReader::new(fs::File::open(&log).unwrap())
            .lines()
            .map(|l| l.unwrap())
            .collect();

        assert_eq!(lines.len(), 2);
        assert!(lines[0].contains("first entry"));
        assert!(lines[1].contains("second entry"));
        // Append — original first line must still be there.
        assert!(lines[0].contains("DRY_RUN"));

        fs::remove_dir_all(&tmp).unwrap();
    }

    #[test]
    fn audit_log_path_contains_state_owl() {
        // We can't rely on HOME being set in all test environments, but we
        // can confirm the path structure when it is.
        if let Some(p) = audit_log_path() {
            let s = p.to_string_lossy();
            assert!(s.contains(".local/state/owl/audit.log"), "unexpected path: {s}");
        }
    }
}
