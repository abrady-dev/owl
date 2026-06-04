use std::collections::VecDeque;
use std::io;
use std::time::{Duration, Instant};

use ratatui::crossterm::event::{self, Event, KeyCode, KeyEventKind};
use ratatui::crossterm::terminal::{
    disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
};
use ratatui::crossterm::ExecutableCommand;
use ratatui::DefaultTerminal;

use crate::collect::{
    apps::{AppEntry, AppSource, PackageManager},
    cpu::{CpuStats, RawCpuStats},
    disk::{DiskStats, RawDiskIo},
    downloads::FileEntry,
    memory::MemStats,
    network::{NetStats, RawNetStats},
    power::PowerStats,
    thermal::ThermalStats,
};
use crate::ui;

#[derive(Clone, Copy, PartialEq)]
pub enum View {
    Overview,
    Help,
    Downloads,
    Apps,
    Clean,
}

#[derive(Clone, Copy, PartialEq)]
pub enum AppState {
    Menu,
    Dashboard,
}

/// State machine for the Downloads file browser.
#[derive(Clone, PartialEq)]
pub enum BrowseMode {
    Navigate,
    Search,
    /// (display_name, path_to_delete, size_bytes)
    Confirm(String, std::path::PathBuf, u64),
}

/// How to remove an installed application.
#[derive(Clone, PartialEq)]
pub enum UninstallCmd {
    SystemPkg(String, PackageManager), // (pkg_name, pm)
    Flatpak(String),                   // app ID
    Snap(String),                      // snap name
    Unknown,                           // couldn't resolve
}

/// State machine for the Apps view.
#[derive(Clone, PartialEq)]
pub enum AppMode {
    Navigate,
    Search,
    Confirm { name: String, cmd: UninstallCmd },
}

/// State machine for the Clean view.
#[derive(Clone, PartialEq)]
pub enum CleanMode {
    Navigate,
    Confirm(usize), // index of the target being confirmed
}

pub const MENU_ITEMS: &[(&str, &str, View)] = &[
    ("Overview",  "Full system dashboard",        View::Overview),
    ("Apps",      "Remove installed applications", View::Apps),
    ("Downloads", "Browse and delete ~/Downloads", View::Downloads),
    ("Clean",     "Free up disk space",            View::Clean),
    ("Help",      "Keybindings and usage",         View::Help),
];

pub struct App {
    pub mem: MemStats,
    pub cpu: CpuStats,
    pub disks: Vec<DiskStats>,
    pub net: NetStats,
    pub net_rx_history: VecDeque<u64>,
    pub net_tx_history: VecDeque<u64>,
    pub thermal: ThermalStats,
    pub power: PowerStats,
    pub health: f64,
    pub state: AppState,
    pub current_view: View,
    pub menu_idx: usize,
    pub paused: bool,
    pub owl_frames: Vec<Vec<u8>>,  // RGBA 32×32 per frame, idle sheet
    pub owl_frame_idx: usize,
    pub owl_frame_timer: Instant,

    // system info
    pub hostname: String,
    pub uptime_secs: u64,
    pub load_1m: f64,
    pub load_5m: f64,
    pub load_15m: f64,
    pub cpu_model: String,
    pub iface_name: String,
    pub load_history: VecDeque<u64>,

    cpu_raw: Option<RawCpuStats>,
    net_raw: Option<RawNetStats>,
    disk_io_raw: Option<RawDiskIo>,

    // downloads view
    pub dl_files: Vec<FileEntry>,
    pub dl_idx: usize,
    pub dl_search: String,
    pub dl_mode: BrowseMode,

    // apps view
    pub app_list: Vec<AppEntry>,
    pub app_idx: usize,
    pub app_search: String,
    pub app_mode: AppMode,

    // clean view
    pub clean_targets: Vec<crate::collect::caches::CacheEntry>,
    pub clean_idx: usize,
    pub clean_mode: CleanMode,
}

impl App {
    pub fn new() -> Self {
        let hostname = crate::collect::system::hostname();
        let cpu_model = crate::collect::system::cpu_model();
        let iface_name = crate::collect::system::primary_iface();

        Self {
            mem: MemStats::default(),
            cpu: CpuStats::default(),
            disks: Vec::new(),
            net: NetStats::default(),
            net_rx_history: VecDeque::from(vec![0u64; 60]),
            net_tx_history: VecDeque::from(vec![0u64; 60]),
            thermal: ThermalStats::default(),
            power: PowerStats::default(),
            health: 100.0,
            state: AppState::Menu,
            current_view: View::Overview,
            menu_idx: 0,
            paused: false,
            owl_frames: crate::splash::load_idle_frames(),
            owl_frame_idx: 0,
            owl_frame_timer: Instant::now(),
            hostname,
            uptime_secs: 0,
            load_1m: 0.0,
            load_5m: 0.0,
            load_15m: 0.0,
            cpu_model,
            iface_name,
            load_history: VecDeque::from(vec![0u64; 60]),
            cpu_raw: None,
            net_raw: None,
            disk_io_raw: None,
            dl_files: Vec::new(),
            dl_idx: 0,
            dl_search: String::new(),
            dl_mode: BrowseMode::Navigate,
            app_list: Vec::new(),
            app_idx: 0,
            app_search: String::new(),
            app_mode: AppMode::Navigate,
            clean_targets: Vec::new(),
            clean_idx: 0,
            clean_mode: CleanMode::Navigate,
        }
    }

    pub fn refresh(&mut self) {
        self.mem = crate::collect::memory::read().unwrap_or_default();

        let curr_cpu = crate::collect::cpu::read_raw();
        if let Some(curr) = curr_cpu {
            self.cpu = crate::collect::cpu::compute(self.cpu_raw.as_ref(), &curr);
            self.cpu_raw = Some(curr);
        }

        self.disks = crate::collect::disk::read_usage().unwrap_or_default();

        let curr_disk_io = crate::collect::disk::read_io_raw();
        if let Some(curr) = curr_disk_io {
            if let Some(prev) = &self.disk_io_raw {
                crate::collect::disk::compute_io(prev, &curr, &mut self.disks);
            }
            self.disk_io_raw = Some(curr);
        }

        let curr_net = crate::collect::network::read_raw();
        if let Some(curr) = curr_net {
            if let Some(prev) = &self.net_raw {
                self.net = crate::collect::network::compute(prev, &curr);
            }
            self.net_raw = Some(curr);
        }

        push_ring(&mut self.net_rx_history, self.net.rx_bps);
        push_ring(&mut self.net_tx_history, self.net.tx_bps);

        self.thermal = crate::collect::thermal::read().unwrap_or_default();
        self.power = crate::collect::power::read().unwrap_or_default();
        self.health = self.compute_health();

        // system info
        self.uptime_secs = crate::collect::system::uptime_secs();
        let (l1, l5, l15) = crate::collect::system::loadavg();
        self.load_1m = l1;
        self.load_5m = l5;
        self.load_15m = l15;
        push_ring(&mut self.load_history, (l1 * 100.0) as u64);
    }

    fn compute_health(&self) -> f64 {
        let disk_pcts: Vec<f64> = self
            .disks
            .iter()
            .filter(|d| d.total_bytes > 0)
            .map(|d| d.used_bytes as f64 / d.total_bytes as f64 * 100.0)
            .collect();

        let max_temp = self
            .thermal
            .sensors
            .iter()
            .map(|s| s.temp_c)
            .fold(f64::NEG_INFINITY, f64::max);
        let max_temp = if max_temp.is_infinite() { 0.0 } else { max_temp };

        let mem_pct = self.mem.used_ratio() * 100.0;
        compute_health_score(self.cpu.total_pct, mem_pct, &disk_pcts, max_temp)
    }

    fn enter_view(&mut self, idx: usize) {
        self.menu_idx = idx;
        self.current_view = MENU_ITEMS[idx].2;
        self.state = AppState::Dashboard;
        if self.current_view == View::Downloads {
            self.dl_files = crate::collect::downloads::read_downloads();
            self.dl_idx = 0;
            self.dl_search.clear();
            self.dl_mode = BrowseMode::Navigate;
        }
        if self.current_view == View::Apps {
            self.app_list = crate::collect::apps::list_apps();
            self.app_idx = 0;
            self.app_search.clear();
            self.app_mode = AppMode::Navigate;
        }
        if self.current_view == View::Clean {
            self.clean_targets = crate::collect::caches::scan();
            self.clean_idx = 0;
            self.clean_mode = CleanMode::Navigate;
        }
    }

    pub fn filtered_files(&self) -> Vec<&FileEntry> {
        if self.dl_search.is_empty() {
            self.dl_files.iter().collect()
        } else {
            let q = self.dl_search.to_lowercase();
            self.dl_files
                .iter()
                .filter(|f| f.name.to_lowercase().contains(&q))
                .collect()
        }
    }

    fn filtered_files_count(&self) -> usize {
        self.filtered_files().len()
    }

    fn run_delete(&mut self, path: &std::path::Path) -> io::Result<()> {
        let home = std::env::var_os("HOME")
            .map(std::path::PathBuf::from)
            .unwrap_or_default();
        // Canonicalize both sides so symlinks and `..` components can't escape
        // ~/Downloads. We check the resolved real path, but delete the original
        // path (so a symlink inside Downloads removes the link, not its target).
        let downloads_real = std::fs::canonicalize(home.join("Downloads"))?;
        let real = std::fs::canonicalize(path)?;
        if !real.starts_with(&downloads_real) {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "path is outside ~/Downloads",
            ));
        }

        if path.is_dir() {
            std::fs::remove_dir_all(path)?;
        } else {
            std::fs::remove_file(path)?;
        }

        self.dl_files.retain(|f| f.path != path);
        let count = self.filtered_files_count();
        if count == 0 {
            self.dl_idx = 0;
        } else if self.dl_idx >= count {
            self.dl_idx = count - 1;
        }
        Ok(())
    }

    pub fn filtered_apps(&self) -> Vec<&AppEntry> {
        if self.app_search.is_empty() {
            self.app_list.iter().collect()
        } else {
            let q = self.app_search.to_lowercase();
            self.app_list
                .iter()
                .filter(|a| a.name.to_lowercase().contains(&q))
                .collect()
        }
    }

    fn filtered_apps_count(&self) -> usize {
        self.filtered_apps().len()
    }

    fn resolve_uninstall(app: &AppEntry) -> UninstallCmd {
        match &app.source {
            AppSource::Flatpak => {
                match crate::collect::apps::flatpak_app_id(&app.desktop_path) {
                    Some(id) => UninstallCmd::Flatpak(id),
                    None => UninstallCmd::Unknown,
                }
            }
            AppSource::Snap => {
                match crate::collect::apps::snap_name(&app.desktop_path) {
                    Some(name) => UninstallCmd::Snap(name),
                    None => UninstallCmd::Unknown,
                }
            }
            AppSource::System => {
                match PackageManager::detect() {
                    Some(pm) => match pm.owner_of(&app.desktop_path) {
                        Some(pkg) => UninstallCmd::SystemPkg(pkg, pm),
                        None => UninstallCmd::Unknown,
                    },
                    None => UninstallCmd::Unknown,
                }
            }
        }
    }

    fn run_app_remove(
        &mut self,
        name: &str,
        cmd: &UninstallCmd,
        terminal: &mut DefaultTerminal,
    ) -> io::Result<()> {
        disable_raw_mode()?;
        std::io::stdout().execute(LeaveAlternateScreen)?;

        let status = match cmd {
            UninstallCmd::SystemPkg(pkg, pm) => {
                println!("\nRemoving {} ({})...\n", name, pm.display_remove_cmd(pkg));
                pm.spawn_remove(pkg)
            }
            UninstallCmd::Flatpak(id) => {
                println!("\nRemoving {} (flatpak uninstall {})...\n", name, id);
                std::process::Command::new("flatpak")
                    .args(["uninstall", "--assumeyes", id])
                    .status()
            }
            UninstallCmd::Snap(snap) => {
                println!("\nRemoving {} (snap remove {})...\n", name, snap);
                std::process::Command::new("sudo")
                    .args(["snap", "remove", snap])
                    .status()
            }
            UninstallCmd::Unknown => {
                println!("\n  Could not determine how to remove '{}'.", name);
                println!("  Try: sudo pacman -Rns <package-name>");
                std::io::stdin().read_line(&mut String::new())?;
                std::io::stdout().execute(EnterAlternateScreen)?;
                enable_raw_mode()?;
                terminal.clear()?;
                return Ok(());
            }
        };

        match status {
            Ok(s) if s.success() => {
                let app_name = name.to_owned();
                self.app_list.retain(|a| a.name != app_name);
                let count = self.filtered_apps_count();
                if count == 0 {
                    self.app_idx = 0;
                } else if self.app_idx >= count {
                    self.app_idx = count - 1;
                }
            }
            Ok(s) => println!("\n  command exited with {}", s),
            Err(e) => println!("\n  failed to run command: {}", e),
        }

        println!("\nPress Enter to return to owl...");
        std::io::stdin().read_line(&mut String::new())?;

        std::io::stdout().execute(EnterAlternateScreen)?;
        enable_raw_mode()?;
        terminal.clear()?;
        Ok(())
    }

    fn handle_apps_key(
        &mut self,
        code: KeyCode,
        terminal: &mut DefaultTerminal,
    ) -> io::Result<bool> {
        let mode = self.app_mode.clone();
        match mode {
            AppMode::Confirm { name, cmd } => {
                if code == KeyCode::Char('y') || code == KeyCode::Char('Y') {
                    self.app_mode = AppMode::Navigate;
                    self.run_app_remove(&name, &cmd, terminal)?;
                } else {
                    self.app_mode = AppMode::Navigate;
                }
            }
            AppMode::Search => match code {
                KeyCode::Esc | KeyCode::Enter => self.app_mode = AppMode::Navigate,
                KeyCode::Backspace => {
                    self.app_search.pop();
                    self.app_idx =
                        self.app_idx.min(self.filtered_apps_count().saturating_sub(1));
                }
                KeyCode::Char(c) => {
                    self.app_search.push(c);
                    self.app_idx = 0;
                }
                _ => {}
            },
            AppMode::Navigate => match code {
                KeyCode::Char('q') => return Ok(true),
                KeyCode::Esc => {
                    if !self.app_search.is_empty() {
                        self.app_search.clear();
                        self.app_idx = 0;
                    } else {
                        self.state = AppState::Menu;
                    }
                }
                KeyCode::Up | KeyCode::Char('k') => {
                    self.app_idx = self.app_idx.saturating_sub(1);
                }
                KeyCode::Down | KeyCode::Char('j') => {
                    let max = self.filtered_apps_count().saturating_sub(1);
                    if self.app_idx < max {
                        self.app_idx += 1;
                    }
                }
                KeyCode::PageUp => {
                    self.app_idx = self.app_idx.saturating_sub(15);
                }
                KeyCode::PageDown => {
                    let max = self.filtered_apps_count().saturating_sub(1);
                    self.app_idx = (self.app_idx + 15).min(max);
                }
                KeyCode::Char('/') => self.app_mode = AppMode::Search,
                KeyCode::Char('d') | KeyCode::Enter => {
                    if let Some(app) = self.filtered_apps().get(self.app_idx).copied() {
                        let cmd = Self::resolve_uninstall(app);
                        self.app_mode = AppMode::Confirm {
                            name: app.name.clone(),
                            cmd,
                        };
                    }
                }
                _ => {}
            },
        }
        Ok(false)
    }

    fn handle_downloads_key(&mut self, code: KeyCode) -> io::Result<bool> {
        let mode = self.dl_mode.clone();
        match mode {
            BrowseMode::Confirm(_, path, _) => {
                if code == KeyCode::Char('y') || code == KeyCode::Char('Y') {
                    self.dl_mode = BrowseMode::Navigate;
                    self.run_delete(&path)?;
                } else {
                    self.dl_mode = BrowseMode::Navigate;
                }
            }
            BrowseMode::Search => match code {
                KeyCode::Esc | KeyCode::Enter => self.dl_mode = BrowseMode::Navigate,
                KeyCode::Backspace => {
                    self.dl_search.pop();
                    self.dl_idx =
                        self.dl_idx.min(self.filtered_files_count().saturating_sub(1));
                }
                KeyCode::Char(c) => {
                    self.dl_search.push(c);
                    self.dl_idx = 0;
                }
                _ => {}
            },
            BrowseMode::Navigate => match code {
                KeyCode::Char('q') => return Ok(true),
                KeyCode::Esc => {
                    if !self.dl_search.is_empty() {
                        self.dl_search.clear();
                        self.dl_idx = 0;
                    } else {
                        self.state = AppState::Menu;
                    }
                }
                KeyCode::Up | KeyCode::Char('k') => {
                    self.dl_idx = self.dl_idx.saturating_sub(1);
                }
                KeyCode::Down | KeyCode::Char('j') => {
                    let max = self.filtered_files_count().saturating_sub(1);
                    if self.dl_idx < max {
                        self.dl_idx += 1;
                    }
                }
                KeyCode::PageUp => {
                    self.dl_idx = self.dl_idx.saturating_sub(15);
                }
                KeyCode::PageDown => {
                    let max = self.filtered_files_count().saturating_sub(1);
                    self.dl_idx = (self.dl_idx + 15).min(max);
                }
                KeyCode::Char('/') => {
                    self.dl_mode = BrowseMode::Search;
                }
                KeyCode::Char('d') | KeyCode::Enter => {
                    if let Some(f) = self.filtered_files().get(self.dl_idx) {
                        self.dl_mode = BrowseMode::Confirm(
                            f.name.clone(),
                            f.path.clone(),
                            f.size_bytes,
                        );
                    }
                }
                _ => {}
            },
        }
        Ok(false)
    }

    fn handle_clean_key(
        &mut self,
        code: KeyCode,
        terminal: &mut DefaultTerminal,
    ) -> io::Result<bool> {
        match self.clean_mode.clone() {
            CleanMode::Confirm(idx) => {
                if code == KeyCode::Char('y') || code == KeyCode::Char('Y') {
                    self.clean_mode = CleanMode::Navigate;
                    self.run_clean(idx, terminal)?;
                } else {
                    self.clean_mode = CleanMode::Navigate;
                }
            }
            CleanMode::Navigate => match code {
                KeyCode::Char('q') => return Ok(true),
                KeyCode::Esc => self.state = AppState::Menu,
                KeyCode::Up | KeyCode::Char('k') => {
                    self.clean_idx = self.clean_idx.saturating_sub(1);
                }
                KeyCode::Down | KeyCode::Char('j') => {
                    let max = self.clean_targets.len().saturating_sub(1);
                    if self.clean_idx < max {
                        self.clean_idx += 1;
                    }
                }
                KeyCode::Char('d') | KeyCode::Enter => {
                    if self.clean_idx < self.clean_targets.len() {
                        self.clean_mode = CleanMode::Confirm(self.clean_idx);
                    }
                }
                _ => {}
            },
        }
        Ok(false)
    }

    fn run_clean(&mut self, idx: usize, terminal: &mut DefaultTerminal) -> io::Result<()> {
        use crate::collect::caches::CacheAction;

        if idx >= self.clean_targets.len() {
            return Ok(());
        }

        let action = self.clean_targets[idx].action.clone();

        let success = match action {
            CacheAction::RemoveDir(ref path) => {
                std::fs::remove_dir_all(path).is_ok()
            }
            CacheAction::RemoveDirs(ref paths) => {
                paths.iter().all(|p| !p.exists() || std::fs::remove_dir_all(p).is_ok())
            }
            CacheAction::Shell(ref args) => {
                if args.is_empty() {
                    return Ok(());
                }
                disable_raw_mode()?;
                std::io::stdout().execute(LeaveAlternateScreen)?;

                println!("\nRunning: {}\n", args.join(" "));
                let result = std::process::Command::new(&args[0])
                    .args(&args[1..])
                    .status();

                let ok = match result {
                    Ok(s) if s.success() => true,
                    Ok(s) => { println!("\n  exited with {}", s); false }
                    Err(e) => { println!("\n  failed to run: {}", e); false }
                };

                println!("\nPress Enter to return to owl...");
                std::io::stdin().read_line(&mut String::new())?;
                std::io::stdout().execute(EnterAlternateScreen)?;
                enable_raw_mode()?;
                terminal.clear()?;
                ok
            }
        };

        if success {
            self.clean_targets.remove(idx);
            if !self.clean_targets.is_empty() && self.clean_idx >= self.clean_targets.len() {
                self.clean_idx = self.clean_targets.len() - 1;
            }
        }

        Ok(())
    }

    pub fn run(&mut self, terminal: &mut DefaultTerminal) -> io::Result<()> {
        let tick_rate = Duration::from_millis(1000);
        let anim_rate = Duration::from_millis(130);
        let mut last_tick = Instant::now();

        self.cpu_raw = crate::collect::cpu::read_raw();
        self.net_raw = crate::collect::network::read_raw();
        self.disk_io_raw = crate::collect::disk::read_io_raw();

        self.refresh();

        loop {
            terminal.draw(|frame| ui::draw(frame, self))?;

            let anim_remaining = anim_rate.saturating_sub(self.owl_frame_timer.elapsed());
            let timeout = tick_rate.saturating_sub(last_tick.elapsed()).min(anim_remaining);
            if event::poll(timeout)? {
                if let Event::Key(key) = event::read()? {
                    if key.kind == KeyEventKind::Press {
                        match self.state {
                            AppState::Menu => match key.code {
                                KeyCode::Char('q') => return Ok(()),
                                KeyCode::Up | KeyCode::Char('k') if self.menu_idx > 0 => {
                                    self.menu_idx -= 1;
                                }
                                KeyCode::Down | KeyCode::Char('j')
                                    if self.menu_idx + 1 < MENU_ITEMS.len() =>
                                {
                                    self.menu_idx += 1;
                                }
                                KeyCode::Enter => {
                                    self.enter_view(self.menu_idx);
                                }
                                KeyCode::Char(c) if c.is_ascii_digit() => {
                                    let n = (c as usize).saturating_sub('1' as usize);
                                    if n < MENU_ITEMS.len() {
                                        self.enter_view(n);
                                    }
                                }
                                _ => {}
                            },
                            AppState::Dashboard => {
                                if self.current_view == View::Downloads {
                                    let quit = self.handle_downloads_key(key.code)?;
                                    if quit {
                                        return Ok(());
                                    }
                                } else if self.current_view == View::Apps {
                                    let quit = self.handle_apps_key(key.code, terminal)?;
                                    if quit {
                                        return Ok(());
                                    }
                                } else if self.current_view == View::Clean {
                                    let quit = self.handle_clean_key(key.code, terminal)?;
                                    if quit {
                                        return Ok(());
                                    }
                                } else {
                                    match key.code {
                                        KeyCode::Char('q') => return Ok(()),
                                        KeyCode::Char('p') => self.paused = !self.paused,
                                        KeyCode::Esc => self.state = AppState::Menu,
                                        _ => {}
                                    }
                                }
                            }
                        }
                    }
                }
            }

            if self.owl_frame_timer.elapsed() >= anim_rate {
                if !self.owl_frames.is_empty() {
                    self.owl_frame_idx = (self.owl_frame_idx + 1) % self.owl_frames.len();
                }
                self.owl_frame_timer = Instant::now();
            }

            if last_tick.elapsed() >= tick_rate {
                if !self.paused {
                    self.refresh();
                }
                last_tick = Instant::now();
            }
        }
    }
}

fn push_ring(buf: &mut VecDeque<u64>, val: u64) {
    if buf.len() >= 60 {
        buf.pop_front();
    }
    buf.push_back(val);
}

pub fn compute_health_score(cpu_pct: f64, mem_pct: f64, disk_pcts: &[f64], max_temp_c: f64) -> f64 {
    let mut scores: Vec<f64> = Vec::new();

    scores.push((100.0 - cpu_pct).clamp(0.0, 100.0));
    scores.push((100.0 - mem_pct).clamp(0.0, 100.0));

    for &pct in disk_pcts {
        scores.push((100.0 - pct).clamp(0.0, 100.0));
    }

    if max_temp_c > 0.0 {
        let temp_score = if max_temp_c <= 60.0 {
            100.0
        } else if max_temp_c >= 90.0 {
            0.0
        } else {
            (90.0 - max_temp_c) / 30.0 * 100.0
        };
        scores.push(temp_score);
    }

    if scores.is_empty() {
        return 100.0;
    }

    scores.iter().sum::<f64>() / scores.len() as f64
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn health_score_bounds() {
        for cpu in [0.0, 50.0, 100.0] {
            for mem in [0.0, 50.0, 100.0] {
                for disk in [&[][..], &[0.0_f64][..], &[100.0_f64][..]] {
                    for temp in [0.0, 60.0, 90.0, 100.0] {
                        let s = compute_health_score(cpu, mem, disk, temp);
                        assert!(
                            (0.0..=100.0).contains(&s),
                            "out of bounds: cpu={cpu} mem={mem} temp={temp} → {s}"
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn health_score_thresholds() {
        let quiet = compute_health_score(5.0, 20.0, &[30.0], 40.0);
        assert!(quiet > 70.0, "expected green, got {quiet}");

        let stressed = compute_health_score(95.0, 90.0, &[95.0], 88.0);
        assert!(stressed < 40.0, "expected red, got {stressed}");
    }
}
