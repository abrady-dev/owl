use std::collections::HashMap;
use std::ffi::CString;
use std::mem::MaybeUninit;
use std::time::Instant;

#[derive(Default)]
pub struct DiskStats {
    pub mount: String,
    pub device: String,
    pub total_bytes: u64,
    pub used_bytes: u64,
    pub read_bps: u64,
    pub write_bps: u64,
}

pub struct RawDiskIo {
    pub stats: HashMap<String, [u64; 2]>, // device name -> [sectors_read, sectors_written]
    pub time: Instant,
}

fn is_virtual_fs(fstype: &str) -> bool {
    matches!(
        fstype,
        "tmpfs"
            | "devtmpfs"
            | "sysfs"
            | "proc"
            | "cgroup"
            | "cgroup2"
            | "debugfs"
            | "securityfs"
            | "pstore"
            | "efivarfs"
            | "bpf"
            | "configfs"
            | "tracefs"
            | "hugetlbfs"
            | "mqueue"
            | "ramfs"
            | "overlay"
            | "autofs"
            | "rpc_pipefs"
            | "nfsd"
            | "fusectl"
            | "fuse.portal"
            | "fuse.gvfsd-fuse"
            | "squashfs"
    )
}

/// Parse `/proc/mounts` content and return (device, mountpoint, fstype) tuples
/// for real (non-virtual) filesystems only, deduplicating by device path.
pub fn parse_mounts(content: &str) -> Vec<(String, String, String)> {
    let mut seen_devices = std::collections::HashSet::new();
    let mut result = Vec::new();

    for line in content.lines() {
        let mut parts = line.split_whitespace();
        let device = match parts.next() {
            Some(d) => d,
            None => continue,
        };
        let mount = match parts.next() {
            Some(m) => m,
            None => continue,
        };
        let fstype = match parts.next() {
            Some(f) => f,
            None => continue,
        };

        if !device.starts_with('/') {
            continue;
        }
        if is_virtual_fs(fstype) {
            continue;
        }
        if !seen_devices.insert(device.to_string()) {
            continue;
        }

        result.push((device.to_string(), mount.to_string(), fstype.to_string()));
    }

    result
}

/// Parse `/proc/diskstats` content and return device -> [sectors_read, sectors_written].
pub fn parse_diskstats(content: &str) -> HashMap<String, [u64; 2]> {
    let mut stats = HashMap::new();
    for line in content.lines() {
        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.len() < 14 {
            continue;
        }
        let name = parts[2].to_string();
        let sectors_read: u64 = parts[5].parse().unwrap_or(0);
        let sectors_written: u64 = parts[9].parse().unwrap_or(0);
        stats.insert(name, [sectors_read, sectors_written]);
    }
    stats
}

pub fn read_usage() -> Option<Vec<DiskStats>> {
    let mounts = std::fs::read_to_string("/proc/mounts").ok()?;
    let entries = parse_mounts(&mounts);
    let mut disks = Vec::new();

    for (device, mount, _fstype) in entries {
        if let Some((total, free)) = statvfs(&mount) {
            disks.push(DiskStats {
                mount,
                device,
                total_bytes: total,
                used_bytes: total.saturating_sub(free),
                read_bps: 0,
                write_bps: 0,
            });
        }
    }

    Some(disks)
}

fn statvfs(path: &str) -> Option<(u64, u64)> {
    let c_path = CString::new(path).ok()?;
    let mut buf = MaybeUninit::<libc::statvfs>::uninit();

    let ret = unsafe { libc::statvfs(c_path.as_ptr(), buf.as_mut_ptr()) };
    if ret != 0 {
        return None;
    }

    let stat = unsafe { buf.assume_init() };
    let total = stat.f_blocks * stat.f_frsize;
    let free = stat.f_bavail * stat.f_frsize;

    Some((total, free))
}

pub fn read_io_raw() -> Option<RawDiskIo> {
    let content = std::fs::read_to_string("/proc/diskstats").ok()?;
    Some(RawDiskIo {
        stats: parse_diskstats(&content),
        time: Instant::now(),
    })
}

pub fn compute_io(prev: &RawDiskIo, curr: &RawDiskIo, disks: &mut [DiskStats]) {
    let elapsed = curr.time.duration_since(prev.time).as_secs_f64();
    if elapsed < 0.001 {
        return;
    }

    for disk in disks.iter_mut() {
        let dev_name = disk.device.rsplit('/').next().unwrap_or(&disk.device);
        if let (Some(p), Some(c)) = (prev.stats.get(dev_name), curr.stats.get(dev_name)) {
            let delta_read = c[0].saturating_sub(p[0]);
            let delta_write = c[1].saturating_sub(p[1]);
            // /proc/diskstats sectors are always 512 bytes
            disk.read_bps = (delta_read as f64 * 512.0 / elapsed) as u64;
            disk.write_bps = (delta_write as f64 * 512.0 / elapsed) as u64;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const MOUNTS_FIXTURE: &str = "\
sysfs /sys sysfs rw,nosuid,nodev,noexec,relatime 0 0
proc /proc proc rw,nosuid,nodev,noexec,relatime 0 0
devtmpfs /dev devtmpfs rw,nosuid,size=8192k,nr_inodes=4096,mode=755 0 0
/dev/sda1 / ext4 rw,relatime 0 0
/dev/sda2 /home ext4 rw,relatime 0 0
tmpfs /tmp tmpfs rw,nosuid,nodev 0 0
overlay /var/lib/docker/overlay2/abc overlay rw,relatime 0 0
/dev/sdb1 /data btrfs rw,relatime 0 0
";

    #[test]
    fn parse_proc_mounts() {
        let entries = parse_mounts(MOUNTS_FIXTURE);
        let fstypes: Vec<&str> = entries.iter().map(|(_, _, fs)| fs.as_str()).collect();
        // Only ext4 and btrfs should survive
        assert!(fstypes.iter().all(|&f| f == "ext4" || f == "btrfs"));
        // sysfs, proc, devtmpfs, tmpfs, overlay excluded
        assert!(!fstypes.contains(&"sysfs"));
        assert!(!fstypes.contains(&"tmpfs"));
        assert!(!fstypes.contains(&"overlay"));
        assert_eq!(entries.len(), 3); // sda1, sda2, sdb1
    }

    const DISKSTATS_FIXTURE: &str = "\
   8       0 sda 1000 0 20000 5000 500 0 8000 2000 0 0 0 0 0 0
   8       1 sda1 800 0 16000 4000 400 0 6400 1600 0 0 0 0 0 0
   8      16 sdb 200 0 4000 1000 100 0 1600 400 0 0 0 0 0 0
";

    #[test]
    fn parse_diskstats_fixture() {
        let stats = parse_diskstats(DISKSTATS_FIXTURE);
        assert_eq!(stats["sda"], [20000, 8000]);
        assert_eq!(stats["sda1"], [16000, 6400]);
        assert_eq!(stats["sdb"], [4000, 1600]);
    }
}
