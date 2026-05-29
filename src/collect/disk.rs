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

pub fn read_usage() -> Option<Vec<DiskStats>> {
    let mounts = std::fs::read_to_string("/proc/mounts").ok()?;
    let mut disks = Vec::new();
    let mut seen_devices = std::collections::HashSet::new();

    for line in mounts.lines() {
        let mut parts = line.split_whitespace();
        let device = parts.next()?;
        let mount = parts.next()?;
        let fstype = parts.next()?;

        if !device.starts_with('/') {
            continue;
        }
        if is_virtual_fs(fstype) {
            continue;
        }
        if !seen_devices.insert(device.to_string()) {
            continue;
        }

        if let Some((total, free)) = statvfs(mount) {
            disks.push(DiskStats {
                mount: mount.to_string(),
                device: device.to_string(),
                total_bytes: total,
                used_bytes: total.saturating_sub(free),
                read_bps: 0,
                write_bps: 0,
            });
        }
    }

    Some(disks)
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
    )
}

fn statvfs(path: &str) -> Option<(u64, u64)> {
    let c_path = CString::new(path).ok()?;
    let mut buf = MaybeUninit::<libc::statvfs>::uninit();

    let ret = unsafe { libc::statvfs(c_path.as_ptr(), buf.as_mut_ptr()) };
    if ret != 0 {
        return None;
    }

    let stat = unsafe { buf.assume_init() };
    let total = (stat.f_blocks as u64) * (stat.f_frsize as u64);
    let free = (stat.f_bavail as u64) * (stat.f_frsize as u64);

    Some((total, free))
}

pub fn read_io_raw() -> Option<RawDiskIo> {
    let content = std::fs::read_to_string("/proc/diskstats").ok()?;
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

    Some(RawDiskIo {
        stats,
        time: Instant::now(),
    })
}

pub fn compute_io(prev: &RawDiskIo, curr: &RawDiskIo, disks: &mut Vec<DiskStats>) {
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
