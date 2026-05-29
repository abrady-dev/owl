use std::path::PathBuf;

/// Whether this invocation should actually delete anything.
/// `DryRun` is the default — `Execute` is opt-in via `--execute`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum RunMode {
    #[default]
    DryRun,
    Execute,
}

impl std::fmt::Display for RunMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RunMode::DryRun => write!(f, "DRY_RUN"),
            RunMode::Execute => write!(f, "EXECUTE"),
        }
    }
}

/// A single file or directory that a cleaner proposes to remove.
#[derive(Debug, Clone)]
pub struct CleanOp {
    /// Canonical path of the item to remove.
    pub path: PathBuf,
    /// Estimated bytes that would be reclaimed.
    pub size_bytes: u64,
    /// Human-readable description of what this is and why it can be removed.
    pub description: String,
}

impl CleanOp {
    pub fn new(path: impl Into<PathBuf>, size_bytes: u64, description: impl Into<String>) -> Self {
        Self {
            path: path.into(),
            size_bytes,
            description: description.into(),
        }
    }
}

/// The result of a scan phase: a list of proposed deletions with a total size.
///
/// Phase flow:
///   1. **Scan** — populate a `Manifest` without touching anything.
///   2. **Present** — show the manifest to the user (`display`).
///   3. **Confirm** — the user explicitly approves.
///   4. **Execute** — only then perform the deletions (if `RunMode::Execute`).
#[derive(Debug, Default)]
pub struct Manifest {
    pub ops: Vec<CleanOp>,
}

impl Manifest {
    pub fn add(&mut self, op: CleanOp) {
        self.ops.push(op);
    }

    pub fn is_empty(&self) -> bool {
        self.ops.is_empty()
    }

    pub fn total_bytes(&self) -> u64 {
        self.ops.iter().map(|op| op.size_bytes).sum()
    }

    /// Print the manifest to stdout so the user can review before confirming.
    pub fn display(&self, mode: RunMode) {
        if self.ops.is_empty() {
            println!("  Nothing to clean.");
            return;
        }
        println!("  [{mode}] Proposed deletions:");
        for op in &self.ops {
            println!(
                "    {:>8}  {}  ({})",
                fmt_bytes(op.size_bytes),
                op.path.display(),
                op.description,
            );
        }
        println!("  Total: {}", fmt_bytes(self.total_bytes()));
    }
}

fn fmt_bytes(bytes: u64) -> String {
    const GIB: u64 = 1 << 30;
    const MIB: u64 = 1 << 20;
    const KIB: u64 = 1 << 10;
    if bytes >= GIB {
        format!("{:.1} GiB", bytes as f64 / GIB as f64)
    } else if bytes >= MIB {
        format!("{:.1} MiB", bytes as f64 / MIB as f64)
    } else if bytes >= KIB {
        format!("{:.1} KiB", bytes as f64 / KIB as f64)
    } else {
        format!("{bytes} B")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_manifest_total_is_zero() {
        let m = Manifest::default();
        assert_eq!(m.total_bytes(), 0);
        assert!(m.is_empty());
    }

    #[test]
    fn manifest_totals_sum_correctly() {
        let mut m = Manifest::default();
        m.add(CleanOp::new("/tmp/a", 1024, "test a"));
        m.add(CleanOp::new("/tmp/b", 2048, "test b"));
        m.add(CleanOp::new("/tmp/c", 512, "test c"));
        assert_eq!(m.total_bytes(), 3584);
        assert!(!m.is_empty());
        assert_eq!(m.ops.len(), 3);
    }

    #[test]
    fn run_mode_default_is_dry_run() {
        assert_eq!(RunMode::default(), RunMode::DryRun);
    }

    #[test]
    fn run_mode_display() {
        assert_eq!(RunMode::DryRun.to_string(), "DRY_RUN");
        assert_eq!(RunMode::Execute.to_string(), "EXECUTE");
    }

    #[test]
    fn clean_op_fields() {
        let op = CleanOp::new("/home/user/.cache/foo", 4096, "thumbnail cache");
        assert_eq!(op.size_bytes, 4096);
        assert_eq!(op.description, "thumbnail cache");
    }
}
