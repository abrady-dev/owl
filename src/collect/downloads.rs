use std::path::PathBuf;

#[derive(Debug, Clone)]
pub struct FileEntry {
    pub name: String,
    pub path: PathBuf,
    pub size_bytes: u64,
    pub is_dir: bool,
}

pub fn read_downloads() -> Vec<FileEntry> {
    let home = match std::env::var_os("HOME").map(PathBuf::from) {
        Some(h) => h,
        None => return Vec::new(),
    };
    let dir = home.join("Downloads");

    let read = match std::fs::read_dir(&dir) {
        Ok(r) => r,
        Err(_) => return Vec::new(),
    };

    let mut entries: Vec<FileEntry> = read
        .flatten()
        .filter_map(|e| {
            let meta = e.metadata().ok()?;
            let is_dir = meta.is_dir();
            Some(FileEntry {
                name: e.file_name().to_string_lossy().into_owned(),
                path: e.path(),
                size_bytes: if is_dir { 0 } else { meta.len() },
                is_dir,
            })
        })
        .collect();

    // largest files first; directories sink to bottom
    entries.sort_unstable_by(|a, b| {
        b.is_dir
            .cmp(&a.is_dir)
            .reverse()
            .then(b.size_bytes.cmp(&a.size_bytes))
            .then(a.name.cmp(&b.name))
    });
    entries
}
