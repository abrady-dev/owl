use std::path::{Path, PathBuf};

#[derive(Clone)]
pub enum CacheAction {
    RemoveDir(PathBuf),        // delete entire directory
    RemoveDirs(Vec<PathBuf>),  // delete specific subdirectories
    Shell(Vec<String>),        // shell out (may require sudo)
}

#[derive(Clone)]
pub struct CacheEntry {
    pub label: &'static str,
    pub path_display: &'static str,
    pub size_bytes: u64,
    pub action: CacheAction,
}

pub fn scan() -> Vec<CacheEntry> {
    let home = match std::env::var_os("HOME").map(PathBuf::from) {
        Some(h) => h,
        None => return Vec::new(),
    };

    let mut entries = Vec::new();

    add_dir(&mut entries, "Thumbnails",   "~/.cache/thumbnails",  home.join(".cache/thumbnails"));
    add_dir(&mut entries, "pip cache",    "~/.cache/pip",         home.join(".cache/pip"));
    add_dir(&mut entries, "npm cache",    "~/.npm/_cacache",      home.join(".npm/_cacache"));
    add_dir(&mut entries, "Gradle cache", "~/.gradle/caches",     home.join(".gradle/caches"));
    add_dir(&mut entries, "Maven repo",   "~/.m2/repository",     home.join(".m2/repository"));

    // Cargo: only the download cache and extracted sources, not installed binaries
    {
        let cache = home.join(".cargo/registry/cache");
        let src   = home.join(".cargo/registry/src");
        let size  = dir_size_if_exists(&cache) + dir_size_if_exists(&src);
        if size > 0 {
            let mut dirs = Vec::new();
            if cache.exists() { dirs.push(cache); }
            if src.exists()   { dirs.push(src); }
            entries.push(CacheEntry {
                label:        "Cargo registry",
                path_display: "~/.cargo/registry/",
                size_bytes:   size,
                action:       CacheAction::RemoveDirs(dirs),
            });
        }
    }

    // User systemd journal
    {
        let journal = home.join(".local/share/systemd/journal");
        let size = dir_size_if_exists(&journal);
        if size > 0 {
            entries.push(CacheEntry {
                label:        "Journal (user)",
                path_display: "~/.local/share/systemd/journal",
                size_bytes:   size,
                action: CacheAction::Shell(vec![
                    "journalctl".into(), "--user".into(), "--vacuum-size=50M".into(),
                ]),
            });
        }
    }

    // Pacman cache — only if paccache is installed
    {
        let pkg      = PathBuf::from("/var/cache/pacman/pkg");
        let paccache = PathBuf::from("/usr/bin/paccache");
        if pkg.exists() && paccache.exists() {
            let size = dir_size_if_exists(&pkg);
            if size > 0 {
                entries.push(CacheEntry {
                    label:        "Pacman cache",
                    path_display: "/var/cache/pacman/pkg",
                    size_bytes:   size,
                    action: CacheAction::Shell(vec![
                        "sudo".into(), "paccache".into(), "-rk2".into(),
                    ]),
                });
            }
        }
    }

    entries
}

fn add_dir(entries: &mut Vec<CacheEntry>, label: &'static str, display: &'static str, path: PathBuf) {
    let size = dir_size_if_exists(&path);
    if size > 0 {
        entries.push(CacheEntry {
            label,
            path_display: display,
            size_bytes: size,
            action: CacheAction::RemoveDir(path),
        });
    }
}

fn dir_size_if_exists(path: &Path) -> u64 {
    if path.exists() { dir_size(path) } else { 0 }
}

pub fn dir_size(path: &Path) -> u64 {
    let Ok(rd) = std::fs::read_dir(path) else { return 0 };
    let mut total = 0u64;
    for entry in rd.flatten() {
        let Ok(meta) = entry.metadata() else { continue };
        if meta.is_dir() {
            total += dir_size(&entry.path());
        } else {
            total += meta.len();
        }
    }
    total
}
