use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span, Text},
    widgets::{Block, Borders, Gauge, Paragraph, Sparkline},
    Frame,
};

use crate::app::{App, MENU_ITEMS};
use crate::collect::system;
use crate::splash;

// ── Brand colors ─────────────────────────────────────────────────────────────

const CYAN: Color = Color::Rgb(63, 220, 220);
const CYAN_DIM: Color = Color::Rgb(31, 138, 138);
const MAGENTA: Color = Color::Rgb(224, 108, 224);
const GREEN: Color = Color::Rgb(95, 211, 138);
const YELLOW: Color = Color::Rgb(230, 196, 107);
const RED: Color = Color::Rgb(224, 104, 95);
const BLUE: Color = Color::Rgb(108, 182, 224);
const TEXT: Color = Color::Rgb(197, 208, 211);
const TEXT_DIM: Color = Color::Rgb(106, 119, 125);
const TEXT_FAINT: Color = Color::Rgb(63, 74, 79);

// ── Color helpers ─────────────────────────────────────────────────────────────

/// <40% → blue, 40-69% → yellow, >=70% → red
fn gauge_color(pct: f64) -> Color {
    if pct < 40.0 {
        BLUE
    } else if pct < 70.0 {
        YELLOW
    } else {
        RED
    }
}

fn health_color(score: f64) -> Color {
    if score > 70.0 {
        GREEN
    } else if score > 40.0 {
        YELLOW
    } else {
        RED
    }
}

fn bat_color(pct: f64) -> Color {
    if pct > 40.0 {
        GREEN
    } else if pct > 20.0 {
        YELLOW
    } else {
        RED
    }
}

fn swap_color(pct: f64) -> Color {
    if pct < 40.0 {
        GREEN
    } else if pct < 70.0 {
        YELLOW
    } else {
        RED
    }
}

// ── Format helpers ─────────────────────────────────────────────────────────────

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

fn bar_line(filled: usize, total: usize, fill_color: Color) -> Vec<Span<'static>> {
    let empty = total.saturating_sub(filled);
    vec![
        Span::styled("█".repeat(filled), Style::default().fg(fill_color)),
        Span::styled("░".repeat(empty), Style::default().fg(TEXT_FAINT)),
    ]
}

// ── Main block ────────────────────────────────────────────────────────────────

pub fn make_main_block() -> Block<'static> {
    Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(CYAN))
        .title_top(Line::from(vec![
            Span::styled("owl", Style::default().fg(CYAN).add_modifier(Modifier::BOLD)),
            Span::styled(" · lean eyes on Linux ", Style::default().fg(CYAN_DIM)),
        ]))
}

// ── Launch / menu screen ──────────────────────────────────────────────────────

pub fn render_launch(f: &mut Frame, app: &App, area: Rect) {
    let art = splash::ART;
    let art_height = art.lines().count() as u16;

    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(art_height),
            Constraint::Length(1),
            Constraint::Min(1),
            Constraint::Length(1),
        ])
        .split(area);

    f.render_widget(
        Paragraph::new(art).style(Style::default().fg(CYAN)),
        rows[0],
    );

    // Tagline with cyan-dim dots
    f.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled("owl", Style::default().fg(CYAN).add_modifier(Modifier::BOLD)),
            Span::styled(" · ", Style::default().fg(CYAN_DIM)),
            Span::styled("lean eyes on Linux", Style::default().fg(TEXT_DIM)),
            Span::styled(" · ", Style::default().fg(CYAN_DIM)),
            Span::styled("system monitor", Style::default().fg(TEXT_FAINT)),
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
                    Span::styled("▶ ", Style::default().fg(CYAN).add_modifier(Modifier::BOLD)),
                    Span::styled(
                        format!("{}. {:<10}", i + 1, label),
                        Style::default().fg(CYAN).add_modifier(Modifier::BOLD),
                    ),
                    Span::styled(*desc, Style::default().fg(TEXT)),
                ])
            } else {
                Line::from(vec![
                    Span::raw("  "),
                    Span::styled(
                        format!("{}. {:<10}", i + 1, label),
                        Style::default().fg(TEXT_DIM),
                    ),
                    Span::styled(*desc, Style::default().fg(TEXT_FAINT)),
                ])
            }
        })
        .collect();

    f.render_widget(Paragraph::new(Text::from(items)), rows[2]);

    f.render_widget(
        Paragraph::new(Line::from(vec![
            keybind("↑↓"),
            Span::styled(" navigate", Style::default().fg(TEXT_FAINT)),
            dim_sep(),
            keybind("Enter"),
            Span::styled(" open", Style::default().fg(TEXT_FAINT)),
            dim_sep(),
            keybind("q"),
            Span::styled(" quit", Style::default().fg(TEXT_FAINT)),
        ])),
        rows[3],
    );
}

// ── Footer ────────────────────────────────────────────────────────────────────

pub fn render_footer(f: &mut Frame, area: Rect) {
    f.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled("  ", Style::default()),
            keybind("q"),
            Span::styled(" quit", Style::default().fg(TEXT_FAINT)),
            dim_sep(),
            keybind("↑↓"),
            Span::styled(" select", Style::default().fg(TEXT_FAINT)),
            dim_sep(),
            keybind("tab"),
            Span::styled(" cycle panel", Style::default().fg(TEXT_FAINT)),
            dim_sep(),
            keybind("p"),
            Span::styled(" pause", Style::default().fg(TEXT_FAINT)),
        ])),
        area,
    );
}

fn keybind(s: &'static str) -> Span<'static> {
    Span::styled(s, Style::default().fg(CYAN))
}

fn dim_sep() -> Span<'static> {
    Span::styled("   ", Style::default().fg(TEXT_FAINT))
}

// ── Header ────────────────────────────────────────────────────────────────────

pub fn render_header(f: &mut Frame, app: &App, area: Rect) {
    // Split: left info | right clock (8 chars)
    let cols = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Min(0), Constraint::Length(8)])
        .split(area);

    let uptime_str = system::fmt_uptime(app.uptime_secs);

    let left = Line::from(vec![
        Span::styled("▟▙ ", Style::default().fg(CYAN)),
        Span::styled("owl", Style::default().fg(CYAN).add_modifier(Modifier::BOLD)),
        Span::styled("  v0.1", Style::default().fg(TEXT_DIM)),
        Span::styled("   ", Style::default()),
        Span::styled(app.hostname.clone(), Style::default().fg(TEXT)),
        Span::styled("   up ", Style::default().fg(TEXT_DIM)),
        Span::styled(uptime_str, Style::default().fg(TEXT)),
        Span::styled("   load ", Style::default().fg(TEXT_DIM)),
        Span::styled(
            format!("{:.2} {:.2} {:.2}", app.load_1m, app.load_5m, app.load_15m),
            Style::default().fg(TEXT),
        ),
    ]);

    let clock = system::local_time_str();
    let right = Paragraph::new(Line::from(Span::styled(
        clock,
        Style::default().fg(TEXT_DIM),
    )))
    .alignment(Alignment::Right);

    f.render_widget(Paragraph::new(left), cols[0]);
    f.render_widget(right, cols[1]);
}

// ── CPU ───────────────────────────────────────────────────────────────────────

pub fn render_cpu(f: &mut Frame, app: &App, area: Rect) {
    let model_short: String = app
        .cpu_model
        .split_whitespace()
        .take(4)
        .collect::<Vec<_>>()
        .join(" ");
    let model_short = if model_short.len() > 22 {
        model_short[..22].to_string()
    } else {
        model_short
    };

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(CYAN_DIM))
        .title_top(Line::from(vec![
            Span::styled(" CPU  ", Style::default().fg(TEXT_DIM)),
            Span::styled(model_short, Style::default().fg(TEXT)),
        ]))
        .title_top(
            Line::from(Span::styled(
                format!(" {:.0}% ", app.cpu.total_pct),
                Style::default()
                    .fg(gauge_color(app.cpu.total_pct))
                    .add_modifier(Modifier::BOLD),
            ))
            .right_aligned(),
        );

    let inner = block.inner(area);
    f.render_widget(block, area);

    if app.cpu.per_core_pct.is_empty() {
        f.render_widget(
            Paragraph::new(Span::styled("waiting...", Style::default().fg(TEXT_FAINT))),
            inner,
        );
        return;
    }

    // Split inner: core rows (Min) + sparkline label (1) + sparkline bars (2)
    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(1), Constraint::Length(1), Constraint::Length(2)])
        .split(inner);

    // Per-core paragraph: 2 columns, up to 8 cores
    let cores = &app.cpu.per_core_pct;
    let half = (cores.len().min(8) + 1) / 2;
    const BAR: usize = 12;

    let core_lines: Vec<Line> = (0..half)
        .map(|i| {
            let mut spans = Vec::new();
            // left core
            let pct_l = cores[i];
            let filled_l = (pct_l.clamp(0.0, 100.0) / 100.0 * BAR as f64) as usize;
            spans.push(Span::styled(
                format!("c{:<2} ", i),
                Style::default().fg(TEXT_DIM),
            ));
            spans.extend(bar_line(filled_l, BAR, gauge_color(pct_l)));
            spans.push(Span::styled(
                format!(" {:3.0}%", pct_l),
                Style::default().fg(gauge_color(pct_l)),
            ));
            // right core
            let j = i + half;
            if j < cores.len().min(8) {
                let pct_r = cores[j];
                let filled_r = (pct_r.clamp(0.0, 100.0) / 100.0 * BAR as f64) as usize;
                spans.push(Span::styled(
                    format!("  c{:<2} ", j),
                    Style::default().fg(TEXT_DIM),
                ));
                spans.extend(bar_line(filled_r, BAR, gauge_color(pct_r)));
                spans.push(Span::styled(
                    format!(" {:3.0}%", pct_r),
                    Style::default().fg(gauge_color(pct_r)),
                ));
            }
            Line::from(spans)
        })
        .collect();

    f.render_widget(Paragraph::new(Text::from(core_lines)), rows[0]);

    // Load sparkline label
    f.render_widget(
        Paragraph::new(Span::styled("  load 60s", Style::default().fg(TEXT_DIM))),
        rows[1],
    );

    // Load sparkline
    let load_data: Vec<u64> = app.load_history.iter().copied().collect();
    f.render_widget(
        Sparkline::default()
            .style(Style::default().fg(CYAN))
            .data(&load_data),
        rows[2],
    );
}

// ── Memory ────────────────────────────────────────────────────────────────────

pub fn render_memory(f: &mut Frame, app: &App, area: Rect) {
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(CYAN_DIM))
        .title_top(Span::styled(" MEM ", Style::default().fg(TEXT_DIM)));

    let inner = block.inner(area);
    f.render_widget(block, area);

    const BAR: usize = 12;

    let mem_pct = if app.mem.total_kb > 0 {
        (app.mem.used_kb as f64 / app.mem.total_kb as f64 * 100.0).clamp(0.0, 100.0)
    } else {
        0.0
    };
    let swap_pct = if app.mem.swap_total_kb > 0 {
        (app.mem.swap_used_kb as f64 / app.mem.swap_total_kb as f64 * 100.0).clamp(0.0, 100.0)
    } else {
        0.0
    };

    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(1), Constraint::Length(1)])
        .split(inner);

    let ram_filled = (mem_pct / 100.0 * BAR as f64) as usize;
    let ram_color = gauge_color(mem_pct);
    let mut ram_spans = vec![Span::styled(" ram ", Style::default().fg(TEXT_DIM))];
    ram_spans.extend(bar_line(ram_filled, BAR, ram_color));
    ram_spans.push(Span::styled(
        format!(" {}/{}", fmt_bytes(app.mem.used_kb * 1024), fmt_bytes(app.mem.total_kb * 1024)),
        Style::default().fg(TEXT_DIM),
    ));
    f.render_widget(Paragraph::new(Line::from(ram_spans)), rows[0]);

    let swp_filled = (swap_pct / 100.0 * BAR as f64) as usize;
    let swp_color = swap_color(swap_pct);
    let mut swp_spans = vec![Span::styled(" swp ", Style::default().fg(TEXT_DIM))];
    swp_spans.extend(bar_line(swp_filled, BAR, swp_color));
    swp_spans.push(Span::styled(
        format!(" {}/{}", fmt_bytes(app.mem.swap_used_kb * 1024), fmt_bytes(app.mem.swap_total_kb * 1024)),
        Style::default().fg(TEXT_DIM),
    ));
    f.render_widget(Paragraph::new(Line::from(swp_spans)), rows[1]);
}

// ── Disk ──────────────────────────────────────────────────────────────────────

pub fn render_disk(f: &mut Frame, app: &App, area: Rect) {
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(CYAN_DIM))
        .title_top(Span::styled(
            " DISK ",
            Style::default().fg(TEXT_DIM),
        ));

    let inner = block.inner(area);
    f.render_widget(block, area);

    if app.disks.is_empty() {
        f.render_widget(
            Paragraph::new(Span::styled("no disks", Style::default().fg(TEXT_FAINT))),
            inner,
        );
        return;
    }

    const BAR: usize = 15;

    let lines: Vec<Line> = app
        .disks
        .iter()
        .map(|disk| {
            let pct = if disk.total_bytes > 0 {
                (disk.used_bytes as f64 / disk.total_bytes as f64 * 100.0).clamp(0.0, 100.0)
            } else {
                0.0
            };
            let filled = (pct / 100.0 * BAR as f64) as usize;
            let color = gauge_color(pct);

            // Truncate mount to 6 chars
            let mount = if disk.mount.len() > 6 {
                format!("{:.6}", &disk.mount)
            } else {
                format!("{:<6}", disk.mount)
            };

            let mut spans = vec![
                Span::styled(format!(" {}", mount), Style::default().fg(TEXT_DIM)),
                Span::raw(" "),
            ];
            spans.extend(bar_line(filled, BAR, color));
            spans.push(Span::styled(
                format!(" {:3.0}%", pct),
                Style::default().fg(color),
            ));
            Line::from(spans)
        })
        .collect();

    f.render_widget(Paragraph::new(Text::from(lines)), inner);
}

// ── Network ───────────────────────────────────────────────────────────────────

pub fn render_network(f: &mut Frame, app: &App, area: Rect) {
    let iface = if app.iface_name.is_empty() {
        "—"
    } else {
        &app.iface_name
    };

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(CYAN_DIM))
        .title_top(Line::from(vec![
            Span::styled(" NET  ", Style::default().fg(TEXT_DIM)),
            Span::styled(iface.to_string(), Style::default().fg(TEXT)),
        ]));

    let inner = block.inner(area);
    f.render_widget(block, area);

    // Split: rx label + sparkline | tx label + sparkline
    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Ratio(1, 2),
            Constraint::Ratio(1, 2),
        ])
        .split(inner);

    let rx_data: Vec<u64> = app.net_rx_history.iter().copied().collect();
    let tx_data: Vec<u64> = app.net_tx_history.iter().copied().collect();

    let rx_sparkline = Sparkline::default()
        .block(Block::default().title(Line::from(vec![
            Span::styled(" rx ↓ ", Style::default().fg(CYAN)),
            Span::styled(fmt_bps(app.net.rx_bps), Style::default().fg(TEXT)),
        ])))
        .style(Style::default().fg(CYAN))
        .data(&rx_data);
    f.render_widget(rx_sparkline, rows[0]);

    let tx_sparkline = Sparkline::default()
        .block(Block::default().title(Line::from(vec![
            Span::styled(" tx ↑ ", Style::default().fg(MAGENTA)),
            Span::styled(fmt_bps(app.net.tx_bps), Style::default().fg(TEXT)),
        ])))
        .style(Style::default().fg(MAGENTA))
        .data(&tx_data);
    f.render_widget(tx_sparkline, rows[1]);
}

// ── Thermal ───────────────────────────────────────────────────────────────────

pub fn render_thermal(f: &mut Frame, app: &App, area: Rect) {
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(CYAN_DIM))
        .title_top(Span::styled(
            " TEMP ",
            Style::default().fg(TEXT_DIM),
        ));

    let inner = block.inner(area);
    f.render_widget(block, area);

    if app.thermal.sensors.is_empty() {
        f.render_widget(
            Paragraph::new(Span::styled("no sensors", Style::default().fg(TEXT_FAINT))),
            inner,
        );
        return;
    }

    // Show up to 2 sensors, each as a 16-cell gauge
    let sensors: Vec<_> = app.thermal.sensors.iter().take(2).collect();
    let constraints: Vec<Constraint> = sensors.iter().map(|_| Constraint::Ratio(1, sensors.len() as u32)).collect();
    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints(constraints)
        .split(inner);

    let labels = ["cpu", "ssd"];
    for (idx, sensor) in sensors.iter().enumerate() {
        let pct = sensor.temp_c.clamp(0.0, 100.0);
        let color = gauge_color(pct);
        let label = labels.get(idx).copied().unwrap_or("tmp");

        let gauge = Gauge::default()
            .block(Block::default().title(Span::styled(
                format!(" {} {:.0}°C ", label, sensor.temp_c),
                Style::default().fg(color),
            )))
            .gauge_style(Style::default().fg(color).bg(Color::Reset))
            .use_unicode(true)
            .percent(pct as u16)
            .label("");
        f.render_widget(gauge, rows[idx]);
    }
}

// ── Battery + Health ──────────────────────────────────────────────────────────

pub fn render_power_health(f: &mut Frame, app: &App, area: Rect) {
    let cols = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(55), Constraint::Percentage(45)])
        .split(area);

    // Battery gauge
    let bat_pct = app
        .power
        .battery_pct
        .map(|p| p as f64)
        .unwrap_or(0.0);

    let bat_label = match &app.power.status {
        Some(s) => match s.as_str() {
            "Charging" => format!("{}% ↑", bat_pct as u8),
            "Discharging" => format!("{}% ↓", bat_pct as u8),
            "Full" => format!("{}% ✓", bat_pct as u8),
            _ => format!("{}%", bat_pct as u8),
        },
        None => {
            if app.power.battery_pct.is_none() {
                "N/A".to_string()
            } else {
                format!("{}%", bat_pct as u8)
            }
        }
    };

    let bat_block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(CYAN_DIM))
        .title_top(Span::styled(
            " BAT ",
            Style::default().fg(TEXT_DIM),
        ));

    if app.power.battery_pct.is_some() {
        let gauge = Gauge::default()
            .block(bat_block)
            .gauge_style(
                Style::default()
                    .fg(bat_color(bat_pct))
                    .bg(Color::Reset),
            )
            .use_unicode(true)
            .percent(bat_pct.clamp(0.0, 100.0) as u16)
            .label(Span::styled(bat_label, Style::default().fg(Color::Black).add_modifier(Modifier::BOLD)));
        f.render_widget(gauge, cols[0]);
    } else {
        f.render_widget(
            Paragraph::new(Span::styled(
                " BAT  N/A",
                Style::default().fg(TEXT_FAINT),
            ))
            .block(bat_block),
            cols[0],
        );
    }

    // Health score
    let health_block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(CYAN_DIM))
        .title_top(Span::styled(
            " HEALTH ",
            Style::default().fg(TEXT_DIM),
        ));

    let score = app.health.clamp(0.0, 100.0);
    let color = health_color(score);
    let label_word = if score > 70.0 {
        "good"
    } else if score > 40.0 {
        "warn"
    } else {
        "bad"
    };

    let health_inner = health_block.inner(cols[1]);
    f.render_widget(health_block, cols[1]);

    f.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled("● ", Style::default().fg(color)),
            Span::styled(
                format!("{:.0}", score),
                Style::default().fg(color).add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                format!("  {}", label_word),
                Style::default().fg(TEXT_DIM),
            ),
        ]))
        .alignment(Alignment::Center),
        health_inner,
    );
}

// ── Help ──────────────────────────────────────────────────────────────────────

pub fn render_help(f: &mut Frame, _app: &App, area: Rect) {
    const BINDS: &[(&str, &str)] = &[
        ("q",      "quit"),
        ("Esc",    "back to menu"),
        ("p",      "pause / resume data refresh"),
        ("↑ / k",  "move selection up"),
        ("↓ / j",  "move selection down"),
        ("Enter",  "open selected view"),
        ("1",      "open Overview"),
        ("2",      "open Help"),
    ];

    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(2), Constraint::Min(0)])
        .split(area);

    f.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled("Keybindings", Style::default().fg(CYAN).add_modifier(Modifier::BOLD)),
        ])),
        rows[0],
    );

    let lines: Vec<Line> = BINDS
        .iter()
        .map(|(key, desc)| {
            Line::from(vec![
                Span::styled(format!("  {:<12}", key), Style::default().fg(CYAN)),
                Span::styled(*desc, Style::default().fg(TEXT_DIM)),
            ])
        })
        .collect();

    f.render_widget(Paragraph::new(Text::from(lines)), rows[1]);
}
