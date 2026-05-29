mod widgets;

use ratatui::{
    layout::{Constraint, Direction, Layout},
    Frame,
};

use crate::app::{App, AppState, View};

pub fn draw(frame: &mut Frame, app: &App) {
    match app.state {
        AppState::Menu => widgets::render_launch(frame, app, frame.area()),
        AppState::Dashboard => draw_dashboard(frame, app),
    }
}

fn draw_dashboard(frame: &mut Frame, app: &App) {
    let area = frame.area();

    // Reserve one line at the bottom for the nav hint
    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(0), Constraint::Length(1)])
        .split(area);

    match app.view {
        View::Overview => draw_overview(frame, app, rows[0]),
        View::Cpu      => widgets::render_cpu(frame, app, rows[0]),
        View::Memory   => widgets::render_memory(frame, app, rows[0]),
        View::Network  => widgets::render_network(frame, app, rows[0]),
        View::Disk     => widgets::render_disk(frame, app, rows[0]),
        View::Thermal  => widgets::render_thermal_full(frame, app, rows[0]),
    }

    widgets::render_dashboard_footer(frame, rows[1]);
}

fn draw_overview(frame: &mut Frame, app: &App, area: ratatui::layout::Rect) {
    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Min(4),
            Constraint::Min(6),
            Constraint::Min(4),
            Constraint::Min(5),
        ])
        .split(area);

    widgets::render_cpu(frame, app, rows[0]);

    let mid = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(40), Constraint::Percentage(60)])
        .split(rows[1]);

    widgets::render_memory(frame, app, mid[0]);
    widgets::render_network(frame, app, mid[1]);

    widgets::render_disk(frame, app, rows[2]);

    let bottom = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(rows[3]);

    widgets::render_thermal(frame, app, bottom[0]);
    widgets::render_health(frame, app, bottom[1]);
}
