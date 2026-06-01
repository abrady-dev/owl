use std::path::{Path, PathBuf};
use std::process::Command;

// ── Source / package-manager types ───────────────────────────────────────────

/// Where an application came from.
#[derive(Debug, Clone, PartialEq)]
pub enum AppSource {
    System,  // pacman / apt / dnf / zypper
    Flatpak,
    Snap,
}

/// The system package manager detected at runtime.
#[derive(Debug, Clone, PartialEq)]
pub enum PackageManager {
    Pacman,
    Apt,
    Dnf,
    Zypper,
}

impl PackageManager {
    /// Return the first package manager found on this system, or None.
    pub fn detect() -> Option<Self> {
        let candidates = [
            ("/usr/bin/pacman", PackageManager::Pacman),
            ("/usr/bin/apt",    PackageManager::Apt),
            ("/usr/bin/dnf",    PackageManager::Dnf),
            ("/usr/bin/zypper", PackageManager::Zypper),
        ];
        for (bin, pm) in &candidates {
            if Path::new(bin).exists() {
                return Some(pm.clone());
            }
        }
        None
    }

    /// Ask the package manager which package owns `path`.
    pub fn owner_of(&self, path: &Path) -> Option<String> {
        let path_str = path.to_string_lossy();
        match self {
            PackageManager::Pacman => {
                let out = Command::new("pacman")
                    .args(["-Qo", &path_str])
                    .output().ok()?;
                if !out.status.success() { return None; }
                // "… is owned by <pkg> <version>"
                let text = std::str::from_utf8(&out.stdout).ok()?;
                let words: Vec<&str> = text.split_whitespace().collect();
                if words.len() >= 2 {
                    Some(words[words.len() - 2].to_owned())
                } else {
                    None
                }
            }
            PackageManager::Apt => {
                let out = Command::new("dpkg")
                    .args(["-S", &path_str])
                    .output().ok()?;
                if !out.status.success() { return None; }
                // "<pkg>: <path>"
                let text = std::str::from_utf8(&out.stdout).ok()?;
                text.split(':').next().map(|s| s.trim().to_owned())
            }
            PackageManager::Dnf | PackageManager::Zypper => {
                let out = Command::new("rpm")
                    .args(["-qf", "--queryformat", "%{NAME}\n", &path_str])
                    .output().ok()?;
                if !out.status.success() { return None; }
                let text = std::str::from_utf8(&out.stdout).ok()?;
                let name = text.lines().next()?.trim().to_owned();
                if name.is_empty() { None } else { Some(name) }
            }
        }
    }

    /// The command string shown to the user before they confirm.
    pub fn display_remove_cmd(&self, pkg: &str) -> String {
        match self {
            PackageManager::Pacman => format!("sudo pacman -Rns {}", pkg),
            PackageManager::Apt    => format!("sudo apt remove {}", pkg),
            PackageManager::Dnf    => format!("sudo dnf remove {}", pkg),
            PackageManager::Zypper => format!("sudo zypper remove {}", pkg),
        }
    }

    /// Spawn the removal command and wait for it to finish.
    pub fn spawn_remove(&self, pkg: &str) -> std::io::Result<std::process::ExitStatus> {
        match self {
            PackageManager::Pacman => Command::new("sudo")
                .args(["pacman", "-Rns", pkg]).status(),
            PackageManager::Apt => Command::new("sudo")
                .args(["apt", "remove", pkg]).status(),
            PackageManager::Dnf => Command::new("sudo")
                .args(["dnf", "remove", pkg]).status(),
            PackageManager::Zypper => Command::new("sudo")
                .args(["zypper", "remove", pkg]).status(),
        }
    }
}

// ── AppEntry ─────────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct AppEntry {
    pub name: String,
    pub desktop_path: PathBuf,
    pub source: AppSource,
}

pub fn list_apps() -> Vec<AppEntry> {
    let home = std::env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_default();

    let search_dirs: &[(PathBuf, AppSource)] = &[
        (PathBuf::from("/usr/share/applications"),                              AppSource::System),
        (home.join(".local/share/applications"),                                AppSource::System),
        (PathBuf::from("/var/lib/flatpak/exports/share/applications"),          AppSource::Flatpak),
        (home.join(".local/share/flatpak/exports/share/applications"),          AppSource::Flatpak),
        (PathBuf::from("/var/lib/snapd/desktop/applications"),                  AppSource::Snap),
    ];

    let mut apps: Vec<AppEntry> = Vec::new();

    for (dir, source) in search_dirs {
        let Ok(entries) = std::fs::read_dir(dir) else { continue };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) != Some("desktop") {
                continue;
            }
            if let Some(name) = parse_desktop_name(&path) {
                apps.push(AppEntry { name, desktop_path: path, source: source.clone() });
            }
        }
    }

    apps.sort_unstable_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
    apps.dedup_by(|a, b| a.name == b.name);
    apps
}

fn parse_desktop_name(path: &Path) -> Option<String> {
    let content = std::fs::read_to_string(path).ok()?;
    let mut name: Option<String> = None;
    let mut skip = false;

    for line in content.lines() {
        if line.starts_with("Name=") && name.is_none() {
            name = Some(line[5..].trim().to_owned());
        } else if line == "NoDisplay=true" || line == "Hidden=true" {
            skip = true;
        }
    }

    if skip { None } else { name.filter(|n| !n.is_empty()) }
}

/// Flatpak app ID = desktop file stem (e.g. com.discordapp.Discord).
pub fn flatpak_app_id(path: &Path) -> Option<String> {
    path.file_stem().map(|s| s.to_string_lossy().into_owned())
}

/// Snap name = part before the first `_` in the desktop file stem.
/// e.g. `discord_discord.desktop` → `discord`
pub fn snap_name(path: &Path) -> Option<String> {
    let stem = path.file_stem()?.to_string_lossy().into_owned();
    Some(stem.split('_').next()?.to_owned())
}
