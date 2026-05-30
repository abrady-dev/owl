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
}

#[derive(Clone, Copy, PartialEq)]
pub enum AppState {
    Menu,
    Dashboard,
}

pub const MENU_ITEMS: &[(&str, &str, View)] = &[
    ("Overview", "Full system dashboard", View::Overview),
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
    pub menu_idx: usize,
    pub paused: bool,

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
            state: AppState::Dashboard,
            menu_idx: 0,
            paused: false,
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
        self.state = AppState::Dashboard;
    }

    pub fn run(&mut self, terminal: &mut DefaultTerminal) -> io::Result<()> {
        let tick_rate = Duration::from_millis(1000);
        let mut last_tick = Instant::now();

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
                            AppState::Dashboard => match key.code {
                                KeyCode::Char('q') => return Ok(()),
                                KeyCode::Char('p') => self.paused = !self.paused,
                                KeyCode::Esc => self.state = AppState::Menu,
                                _ => {}
                            },
                        }
                    }
                }
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
