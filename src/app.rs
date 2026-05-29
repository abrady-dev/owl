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
        let max_temp = if max_temp.is_infinite() {
            0.0
        } else {
            max_temp
        };

        let mem_pct = self.mem.used_ratio() * 100.0;

        compute_health_score(self.cpu.total_pct, mem_pct, &disk_pcts, max_temp)
    }

    fn enter_view(&mut self, idx: usize) {
        self.menu_idx = idx;
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

/// Pure health-score computation, usable in tests without a live system.
///
/// Inputs are all percentages (0–100).  Returns a score in [0, 100]:
/// green > 70, yellow 40–70, red < 40.
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
        // sweep across the full input range and confirm output stays in [0, 100]
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
        // Idle system: low cpu, low mem, no disk pressure, cool temp → green (>70)
        let quiet = compute_health_score(5.0, 20.0, &[30.0], 40.0);
        assert!(quiet > 70.0, "expected green, got {quiet}");

        // Stressed system: high everything → red (<40)
        let stressed = compute_health_score(95.0, 90.0, &[95.0], 88.0);
        assert!(stressed < 40.0, "expected red, got {stressed}");
    }
}
