use std::path::{Path, PathBuf};

/// System roots that owl must never touch, regardless of any other logic.
/// Checked against the **canonicalized** path (post-symlink-resolution) so that
/// a symlink inside `~/.cache` pointing at `/etc` is caught correctly.
static SYSTEM_ROOTS: &[&str] = &[
    "/bin",
    "/boot",
    "/dev",
    "/etc",
    // /home is intentionally absent: cross-user paths are blocked by the
    // "must start with $HOME" rule below, not by a blanket /home prefix check.
    "/lib",
    "/lib32",
    "/lib64",
    "/libx32",
    "/opt",
    "/proc",
    "/root",
    "/run",
    "/sbin",
    "/srv",
    "/sys",
    "/tmp",
    "/usr",
    "/var",
];

/// Subdirectories of `home` where owl is permitted to operate.
/// A path is only a valid target if it is strictly *inside* one of these
/// (not the prefix directory itself, which is the user's working data root).
fn safe_zone_prefixes(home: &Path) -> [PathBuf; 4] {
    [
        home.join(".cache"),
        home.join(".local").join("share").join("Trash"),
        home.join(".local").join("share"),
        home.join(".config"),
    ]
}

/// Return `true` if `canon` is a protected path that owl must never modify.
///
/// **`canon` must already be canonicalized** — call `std::fs::canonicalize`
/// first so that symlinks and `..` segments cannot bypass this check.
/// `home` is the current user's home directory (also canonical).
pub fn is_protected(canon: &Path, home: &Path) -> bool {
    // Non-absolute paths are malformed → refuse.
    if !canon.is_absolute() {
        return true;
    }

    // Protect root itself.
    if canon == Path::new("/") {
        return true;
    }

    // Protect every system root and anything underneath it.
    for &root in SYSTEM_ROOTS {
        let root_path = Path::new(root);
        if canon == root_path || canon.starts_with(root_path) {
            return true;
        }
    }

    // Protect anything outside the user's home directory.
    if !canon.starts_with(home) {
        return true;
    }

    // Protect the home directory itself.
    if canon == home {
        return true;
    }

    // Within home: the path must be strictly inside a safe zone…
    let zones = safe_zone_prefixes(home);
    let in_safe_zone = zones
        .iter()
        .any(|prefix| canon.starts_with(prefix) && canon != prefix);

    if !in_safe_zone {
        return true; // not under any safe zone → protected
    }

    // …AND must not itself be a zone-root directory.  Without this second
    // check, ~/.local/share/Trash would pass the in_safe_zone test via the
    // broader ~/.local/share prefix and be incorrectly marked as a target.
    let is_zone_root = zones.iter().any(|prefix| canon == prefix);
    if is_zone_root {
        return true;
    }

    false // inside a safe zone and not a zone root — may be a valid target
}

/// Canonicalize `path` and check whether it falls inside a safe zone.
///
/// Returns `false` (i.e. protected / unsafe) if the path cannot be resolved
/// or resolves to a protected location.
pub fn is_safe_target(path: &Path) -> bool {
    let home = match std::env::var_os("HOME").map(PathBuf::from) {
        Some(h) => h,
        None => return false,
    };

    let canon = match std::fs::canonicalize(path) {
        Ok(p) => p,
        Err(_) => return false,
    };

    !is_protected(&canon, &home)
}

/// Print a command that requires elevated privileges for the user to run manually.
/// owl never invokes `sudo` itself.
pub fn suggest_sudo(cmd: &str) {
    println!("  Requires root — run manually:");
    println!("    sudo {cmd}");
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn home() -> PathBuf {
        PathBuf::from("/home/testuser")
    }

    // ── Path structure tests ────────────────────────────────────────────────

    #[test]
    fn root_is_protected() {
        assert!(is_protected(Path::new("/"), &home()));
    }

    #[test]
    fn empty_path_is_protected() {
        assert!(is_protected(Path::new(""), &home()));
    }

    #[test]
    fn dot_is_protected() {
        assert!(is_protected(Path::new("."), &home()));
    }

    #[test]
    fn relative_path_is_protected() {
        // Any non-absolute path is treated as malformed.
        assert!(is_protected(Path::new("some/relative/path"), &home()));
        assert!(is_protected(Path::new("../escape"), &home()));
    }

    // ── System path tests ───────────────────────────────────────────────────

    #[test]
    fn system_roots_are_protected() {
        for root in SYSTEM_ROOTS {
            let p = PathBuf::from(root);
            assert!(is_protected(&p, &home()), "{root} should be protected");
            // A file deep inside the root must also be protected.
            let deep = p.join("some/deep/file");
            assert!(is_protected(&deep, &home()), "{deep:?} should be protected");
        }
    }

    #[test]
    fn etc_passwd_is_protected() {
        assert!(is_protected(Path::new("/etc/passwd"), &home()));
    }

    #[test]
    fn usr_bin_is_protected() {
        assert!(is_protected(Path::new("/usr/bin/ls"), &home()));
    }

    // ── Home-directory boundary tests ──────────────────────────────────────

    #[test]
    fn home_dir_itself_is_protected() {
        assert!(is_protected(&home(), &home()));
    }

    #[test]
    fn other_user_home_is_protected() {
        let other = PathBuf::from("/home/otheruser/.cache/thumbnails/test.png");
        assert!(is_protected(&other, &home()));
    }

    #[test]
    fn home_dotfile_is_protected() {
        // ~/.bashrc, ~/.ssh/, ~/.gnupg/ — none of these are in a safe zone.
        for path in &[".bashrc", ".zshrc", ".ssh/id_rsa", ".gnupg/private-keys-v1.d"] {
            let p = home().join(path);
            assert!(is_protected(&p, &home()), "{p:?} should be protected");
        }
    }

    #[test]
    fn home_zone_roots_are_protected() {
        // The zone prefix dirs themselves must not be deletable.
        for dir in &[
            ".cache",
            ".local/share/Trash",
            ".local/share",
            ".config",
        ] {
            let p = home().join(dir);
            assert!(is_protected(&p, &home()), "{p:?} (zone root) should be protected");
        }
    }

    // ── Safe-zone tests ─────────────────────────────────────────────────────

    #[test]
    fn cache_subdir_is_allowed() {
        let p = home().join(".cache/thumbnails/normal/abc123.png");
        assert!(!is_protected(&p, &home()));
    }

    #[test]
    fn trash_subdir_is_allowed() {
        let p = home().join(".local/share/Trash/files/old_document.pdf");
        assert!(!is_protected(&p, &home()));
    }

    #[test]
    fn local_share_app_subdir_is_allowed() {
        let p = home().join(".local/share/some-uninstalled-app/data.db");
        assert!(!is_protected(&p, &home()));
    }

    #[test]
    fn config_app_subdir_is_allowed() {
        let p = home().join(".config/some-uninstalled-app/settings.conf");
        assert!(!is_protected(&p, &home()));
    }

    // ── Adversarial / traversal tests ──────────────────────────────────────

    #[test]
    fn dotdot_resolved_to_system_path_is_blocked() {
        // After canonicalization, ~/.cache/../../etc would resolve to /etc.
        // We verify that the post-canonical /etc is protected.
        let resolved = PathBuf::from("/etc");
        assert!(is_protected(&resolved, &home()));
    }

    #[test]
    fn dotdot_resolved_to_other_home_is_blocked() {
        // ~/.cache/../../../home/other/.ssh resolved → /home/other/.ssh
        let resolved = PathBuf::from("/home/other/.ssh");
        assert!(is_protected(&resolved, &home()));
    }

    #[test]
    fn symlink_pointing_to_etc_is_blocked() {
        use std::fs;
        use std::os::unix::fs::symlink;

        let tmp = std::env::temp_dir().join("owl_safety_symlink_test");
        let fake_home = tmp.join("fakeuser");
        let cache = fake_home.join(".cache");
        fs::create_dir_all(&cache).unwrap();

        let evil = cache.join("evil_link");
        let _ = fs::remove_file(&evil);
        symlink("/etc", &evil).expect("symlink creation failed");

        // Canonicalize resolves the symlink: evil_link → /etc
        let canon = fs::canonicalize(&evil).unwrap_or_else(|_| PathBuf::from("/etc"));
        assert!(
            is_protected(&canon, &fake_home),
            "symlink target /etc must be protected"
        );

        let _ = fs::remove_file(&evil);
        let _ = fs::remove_dir_all(&tmp);
    }

    #[test]
    fn symlink_within_safe_zone_resolved_stays_in_safe_zone() {
        // When a symlink inside .cache resolves to another path still inside
        // .cache, the canonical result is a valid target.
        // We test is_protected directly because the /tmp dir used by temp-file
        // helpers is itself in SYSTEM_ROOTS and would cause a false failure.
        let fake_home = PathBuf::from("/home/testuser");
        let canon = fake_home.join(".cache/thumbnails/large/abc123.png");
        assert!(!is_protected(&canon, &fake_home));
    }

    #[test]
    fn tilde_literal_is_protected() {
        // "~" as a literal path component — not expanded by the kernel.
        // It's a relative path → protected.
        assert!(is_protected(Path::new("~"), &home()));
        assert!(is_protected(Path::new("~/.cache/file"), &home()));
    }
}
