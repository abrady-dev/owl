use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span, Text},
    widgets::{Block, Borders, Gauge, Paragraph, Sparkline},
    Frame,
};

use crate::app::{App, MENU_ITEMS};
use crate::splash;

// ── Launch / menu screen ────────────────────────────────────────────────────

pub fn render_launch(f: &mut Frame, app: &App, area: Rect) {
    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(7),  // ASCII art (5 lines + blank top/bottom)
            Constraint::Length(1),  // tagline
            Constraint::Length(1),  // spacer
            Constraint::Min(0),     // menu list
            Constraint::Length(1),  // footer hints
        ])
        .split(area);

    // ASCII art
    f.render_widget(
        Paragraph::new(splash::ART).style(Style::default().fg(Color::Cyan)),
        rows[0],
    );

    // Tagline
    f.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled("owl", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
            Span::styled("  ·  system monitor", Style::default().fg(Color::DarkGray)),
        ])),
        rows[1],
    );

    // Menu items
    let items: Vec<Line> = MENU_ITEMS
        .iter()
        .enumerate()
        .map(|(i, (label, desc, _))| {
            if i == app.menu_idx {
                Line::from(vec![
                    Span::styled(
                        "▶ ",
                        Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD),
                    ),
                    Span::styled(
                        format!("{}. {:<10}", i + 1, label),
                        Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD),
                    ),
                    Span::styled(*desc, Style::default().fg(Color::White)),
                ])
            } else {
                Line::from(vec![
                    Span::raw("  "),
                    Span::styled(
                        format!("{}. {:<10}", i + 1, label),
                        Style::default().fg(Color::Gray),
                    ),
                    Span::styled(*desc, Style::default().fg(Color::DarkGray)),
                ])
            }
        })
        .collect();

    f.render_widget(Paragraph::new(Text::from(items)), rows[3]);

    // Footer
    let footer = Paragraph::new(Line::from(vec![
        hint("↑↓"), sep(), hint("Enter"), sep(), hint("Q quit"),
    ]));
    f.render_widget(footer, rows[4]);
}

// ── Dashboard footer ────────────────────────────────────────────────────────

pub fn render_dashboard_footer(f: &mut Frame, area: Rect) {
    let footer = Paragraph::new(Line::from(vec![
        hint("Esc"), Span::styled(" menu", Style::default().fg(Color::DarkGray)),
        sep(),
        hint("Q"), Span::styled(" quit", Style::default().fg(Color::DarkGray)),
    ]));
    f.render_widget(footer, area);
}

fn hint(s: &'static str) -> Span<'static> {
    Span::styled(s, Style::default().fg(Color::Cyan))
}

fn sep() -> Span<'static> {
    Span::styled("  │  ", Style::default().fg(Color::DarkGray))
}

// ── Helpers ─────────────────────────────────────────────────────────────────

// Blue → Yellow → Red for CPU load
fn cpu_color(pct: f64) -> Color {
    if pct < 60.0 { Color::LightBlue }
    else if pct < 85.0 { Color::Yellow }
    else { Color::Red }
}

// Cyan → Yellow → Red for memory
fn mem_color(pct: f64) -> Color {
    if pct < 70.0 { Color::Cyan }
    else if pct < 90.0 { Color::Yellow }
    else { Color::Red }
}

// LightBlue → Yellow → Red for disk
fn disk_color(pct: f64) -> Color {
    if pct < 75.0 { Color::LightBlue }
    else if pct < 90.0 { Color::Yellow }
    else { Color::Red }
}

// Green → Yellow → Red for health (semantic: green = good)
fn health_color(score: f64) -> Color {
    if score > 70.0 { Color::Green }
    else if score > 40.0 { Color::Yellow }
    else { Color::Red }
}

// Green → Yellow → Red for temperature (semantic: green = cool)
fn temp_color(temp_c: f64) -> Color {
    if temp_c < 60.0 { Color::Green }
    else if temp_c < 80.0 { Color::Yellow }
    else { Color::Red }
}

fn fmt_bytes(bytes: u64) -> String {
    const GIB: u64 = 1 << 30;
    const MIB: u64 = 1 << 20;
    const KIB: u64 = 1 << 10;
    if bytes >= GIB {
        format!("{:.1}G", bytes as f64 / GIB as f64)
    } else if bytes >= MIB {
        format!("{:.1}M", bytes as f64 / MIB as f64)
    } else if bytes >= KIB {
        format!("{:.1}K", bytes as f64 / KIB as f64)
    } else {
        format!("{}B", bytes)
    }
}

fn fmt_bps(bps: u64) -> String {
    format!("{}/s", fmt_bytes(bps))
}

// ── Widgets ──────────────────────────────────────────────────────────────────

pub fn render_cpu(f: &mut Frame, app: &App, area: Rect) {
    let block = Block::default().title(" CPU ").borders(Borders::ALL);
    let inner = block.inner(area);
    f.render_widget(block, area);

    if app.cpu.per_core_pct.is_empty() {
        f.render_widget(Paragraph::new("Waiting for CPU data..."), inner);
        return;
    }

    let cols = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Length(22), Constraint::Min(0)])
        .split(inner);

    let total_pct = app.cpu.total_pct.clamp(0.0, 100.0) as u16;
    let gauge = Gauge::default()
        .block(Block::default().title(" All "))
        .gauge_style(Style::default().fg(cpu_color(app.cpu.total_pct)))
        .percent(total_pct)
        .label(format!("{:.1}%", app.cpu.total_pct));
    f.render_widget(gauge, cols[0]);

    let bar_width: usize = 16;
    let lines: Vec<Line> = app
        .cpu
        .per_core_pct
        .iter()
        .enumerate()
        .map(|(i, &pct)| {
            let filled = (pct.clamp(0.0, 100.0) / 100.0 * bar_width as f64) as usize;
            let empty = bar_width - filled;
            let color = cpu_color(pct);
            Line::from(vec![
                Span::raw(format!("C{:<2} ", i)),
                Span::styled("█".repeat(filled), Style::default().fg(color)),
                Span::styled("░".repeat(empty), Style::default().fg(Color::DarkGray)),
                Span::raw(format!(" {:5.1}%", pct)),
            ])
        })
        .collect();

    f.render_widget(Paragraph::new(Text::from(lines)), cols[1]);
}

pub fn render_memory(f: &mut Frame, app: &App, area: Rect) {
    let block = Block::default().title(" Memory ").borders(Borders::ALL);
    let inner = block.inner(area);
    f.render_widget(block, area);

    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Ratio(1, 2), Constraint::Ratio(1, 2)])
        .split(inner);

    let mem_pct = if app.mem.total_kb > 0 {
        (app.mem.used_kb as f64 / app.mem.total_kb as f64 * 100.0).clamp(0.0, 100.0) as u16
    } else {
        0
    };
    let ram_gauge = Gauge::default()
        .block(Block::default().title(" RAM "))
        .gauge_style(Style::default().fg(mem_color(mem_pct as f64)))
        .percent(mem_pct)
        .label(format!(
            "{} / {}",
            fmt_bytes(app.mem.used_kb * 1024),
            fmt_bytes(app.mem.total_kb * 1024)
        ));
    f.render_widget(ram_gauge, rows[0]);

    let swap_pct = if app.mem.swap_total_kb > 0 {
        (app.mem.swap_used_kb as f64 / app.mem.swap_total_kb as f64 * 100.0).clamp(0.0, 100.0)
            as u16
    } else {
        0
    };
    let swap_gauge = Gauge::default()
        .block(Block::default().title(" Swap "))
        .gauge_style(Style::default().fg(mem_color(swap_pct as f64)))
        .percent(swap_pct)
        .label(format!(
            "{} / {}",
            fmt_bytes(app.mem.swap_used_kb * 1024),
            fmt_bytes(app.mem.swap_total_kb * 1024)
        ));
    f.render_widget(swap_gauge, rows[1]);
}

pub fn render_network(f: &mut Frame, app: &App, area: Rect) {
    let block = Block::default().title(" Network ").borders(Borders::ALL);
    let inner = block.inner(area);
    f.render_widget(block, area);

    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Ratio(1, 2), Constraint::Ratio(1, 2)])
        .split(inner);

    let rx_data: Vec<u64> = app.net_rx_history.iter().copied().collect();
    let tx_data: Vec<u64> = app.net_tx_history.iter().copied().collect();

    let rx_sparkline = Sparkline::default()
        .block(Block::default().title(format!(" RX  {} ", fmt_bps(app.net.rx_bps))))
        .style(Style::default().fg(Color::Cyan))
        .data(&rx_data);
    f.render_widget(rx_sparkline, rows[0]);

    let tx_sparkline = Sparkline::default()
        .block(Block::default().title(format!(" TX  {} ", fmt_bps(app.net.tx_bps))))
        .style(Style::default().fg(Color::Magenta))
        .data(&tx_data);
    f.render_widget(tx_sparkline, rows[1]);
}

pub fn render_disk(f: &mut Frame, app: &App, area: Rect) {
    let block = Block::default().title(" Disk ").borders(Borders::ALL);
    let inner = block.inner(area);
    f.render_widget(block, area);

    if app.disks.is_empty() {
        f.render_widget(Paragraph::new("No disks found."), inner);
        return;
    }

    let bar_width: usize = 12;
    let lines: Vec<Line> = app
        .disks
        .iter()
        .map(|disk| {
            let pct = if disk.total_bytes > 0 {
                (disk.used_bytes as f64 / disk.total_bytes as f64 * 100.0).clamp(0.0, 100.0)
            } else {
                0.0
            };
            let filled = (pct / 100.0 * bar_width as f64) as usize;
            let empty = bar_width - filled;
            let color = disk_color(pct);
            let io_str = if disk.read_bps > 0 || disk.write_bps > 0 {
                format!(
                    "  R:{} W:{}",
                    fmt_bps(disk.read_bps),
                    fmt_bps(disk.write_bps)
                )
            } else {
                String::new()
            };
            Line::from(vec![
                Span::raw(format!("{:<10} ", disk.mount)),
                Span::styled("█".repeat(filled), Style::default().fg(color)),
                Span::styled("░".repeat(empty), Style::default().fg(Color::DarkGray)),
                Span::raw(format!(
                    " {:3.0}%  {} / {}{}",
                    pct,
                    fmt_bytes(disk.used_bytes),
                    fmt_bytes(disk.total_bytes),
                    io_str
                )),
            ])
        })
        .collect();

    f.render_widget(Paragraph::new(Text::from(lines)), inner);
}

pub fn render_thermal(f: &mut Frame, app: &App, area: Rect) {
    let block = Block::default().title(" Thermal ").borders(Borders::ALL);
    let inner = block.inner(area);
    f.render_widget(block, area);

    let lines: Vec<Line> = if app.thermal.sensors.is_empty() {
        vec![Line::from("No thermal sensors found")]
    } else {
        app.thermal
            .sensors
            .iter()
            .map(|s| {
                let color = temp_color(s.temp_c);
                Line::from(vec![
                    Span::styled(
                        format!("{:5.1}°C", s.temp_c),
                        Style::default().fg(color),
                    ),
                    Span::raw(format!("  {}", s.name)),
                ])
            })
            .collect()
    };

    f.render_widget(Paragraph::new(Text::from(lines)), inner);
}

pub fn render_thermal_full(f: &mut Frame, app: &App, area: Rect) {
    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Percentage(60), Constraint::Percentage(40)])
        .split(area);
    render_thermal(f, app, rows[0]);
    render_health(f, app, rows[1]);
}

pub fn render_health(f: &mut Frame, app: &App, area: Rect) {
    let block = Block::default()
        .title(" Health & Battery ")
        .borders(Borders::ALL);
    let inner = block.inner(area);
    f.render_widget(block, area);

    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(2), Constraint::Min(0)])
        .split(inner);

    let health_pct = app.health.clamp(0.0, 100.0) as u16;
    let label = if app.health > 70.0 {
        "GOOD"
    } else if app.health > 40.0 {
        "FAIR"
    } else {
        "POOR"
    };
    let gauge = Gauge::default()
        .block(Block::default().title(" System Health "))
        .gauge_style(Style::default().fg(health_color(app.health)))
        .percent(health_pct)
        .label(format!("{:.0}%  {}", app.health, label));
    f.render_widget(gauge, rows[0]);

    let battery_line = match (&app.power.battery_pct, &app.power.status) {
        (Some(pct), Some(status)) => {
            let icon = match status.as_str() {
                "Charging" => "↑",
                "Discharging" => "↓",
                "Full" => "=",
                _ => "?",
            };
            format!("Battery: {}%  {}", pct, icon)
        }
        (Some(pct), None) => format!("Battery: {}%", pct),
        _ => "Battery: N/A".to_string(),
    };

    f.render_widget(Paragraph::new(battery_line), rows[1]);
}
