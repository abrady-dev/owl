use std::collections::VecDeque;
use std::io;
use std::time::{Duration, Instant};

use ratatui::crossterm::event::{self, Event, KeyCode, KeyEventKind};
use ratatui::DefaultTerminal;

use crate::collect::{
    cpu::{CpuStats, RawCpuStats},
    disk::{DiskStats, RawDiskIo},
    memory::MemStats,
    network::{NetStats, RawNetStats},
    power::PowerStats,
    thermal::ThermalStats,
};
use crate::ui;

#[derive(Clone, Copy, PartialEq)]
pub enum View {
    Overview,
    Cpu,
    Memory,
    Network,
    Disk,
    Thermal,
}

#[derive(Clone, Copy, PartialEq)]
pub enum AppState {
    Menu,
    Dashboard,
}

pub const MENU_ITEMS: &[(&str, &str, View)] = &[
    ("Overview", "Full system dashboard",      View::Overview),
    ("CPU",      "Processor load per core",    View::Cpu),
    ("Memory",   "RAM and swap usage",         View::Memory),
    ("Network",  "Traffic and bandwidth",      View::Network),
    ("Disk",     "Storage space and I/O",      View::Disk),
    ("Thermal",  "Temperatures and battery",   View::Thermal),
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
    pub view: View,
    pub menu_idx: usize,

    cpu_raw: Option<RawCpuStats>,
    net_raw: Option<RawNetStats>,
    disk_io_raw: Option<RawDiskIo>,
}

impl App {
    pub fn new() -> Self {
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
            view: View::Overview,
            menu_idx: 0,
            cpu_raw: None,
            net_raw: None,
            disk_io_raw: None,
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

        if self.net_rx_history.len() >= 60 {
            self.net_rx_history.pop_front();
        }
        self.net_rx_history.push_back(self.net.rx_bps);

        if self.net_tx_history.len() >= 60 {
            self.net_tx_history.pop_front();
        }
        self.net_tx_history.push_back(self.net.tx_bps);

        self.thermal = crate::collect::thermal::read().unwrap_or_default();
        self.power = crate::collect::power::read().unwrap_or_default();
        self.health = self.compute_health();
    }

    fn compute_health(&self) -> f64 {
        let mut scores: Vec<f64> = Vec::new();

        scores.push(100.0 - self.cpu.total_pct);

        if self.mem.total_kb > 0 {
            let pct = self.mem.used_kb as f64 / self.mem.total_kb as f64 * 100.0;
            scores.push(100.0 - pct);
        }

        for disk in &self.disks {
            if disk.total_bytes > 0 {
                let pct = disk.used_bytes as f64 / disk.total_bytes as f64 * 100.0;
                scores.push(100.0 - pct);
            }
        }

        for sensor in &self.thermal.sensors {
            let score = if sensor.temp_c <= 60.0 {
                100.0
            } else if sensor.temp_c >= 90.0 {
                0.0
            } else {
                (90.0 - sensor.temp_c) / 30.0 * 100.0
            };
            scores.push(score);
        }

        if scores.is_empty() {
            return 100.0;
        }

        scores.iter().sum::<f64>() / scores.len() as f64
    }

    fn enter_view(&mut self, idx: usize) {
        self.menu_idx = idx;
        self.view = MENU_ITEMS[idx].2;
        self.state = AppState::Dashboard;
    }

    pub fn run(&mut self, terminal: &mut DefaultTerminal) -> io::Result<()> {
        let tick_rate = Duration::from_millis(500);
        let mut last_tick = Instant::now();

        // Prime raw baselines so the first refresh can compute deltas
        self.cpu_raw = crate::collect::cpu::read_raw();
        self.net_raw = crate::collect::network::read_raw();
        self.disk_io_raw = crate::collect::disk::read_io_raw();

        self.refresh();

        loop {
            terminal.draw(|frame| ui::draw(frame, self))?;

            let timeout = tick_rate.saturating_sub(last_tick.elapsed());
            if event::poll(timeout)? {
                if let Event::Key(key) = event::read()? {
                    if key.kind == KeyEventKind::Press {
                        match self.state {
                            AppState::Menu => match key.code {
                                KeyCode::Char('q') => return Ok(()),
                                KeyCode::Up | KeyCode::Char('k') => {
                                    if self.menu_idx > 0 {
                                        self.menu_idx -= 1;
                                    }
                                }
                                KeyCode::Down | KeyCode::Char('j') => {
                                    if self.menu_idx + 1 < MENU_ITEMS.len() {
                                        self.menu_idx += 1;
                                    }
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
                            AppState::Dashboard => match key.code {
                                KeyCode::Char('q') => return Ok(()),
                                KeyCode::Esc => self.state = AppState::Menu,
                                _ => {}
                            },
                        }
                    }
                }
            }

            if last_tick.elapsed() >= tick_rate {
                self.refresh();
                last_tick = Instant::now();
            }
        }
    }
}
