mod widgets;

use ratatui::{
    layout::{Constraint, Direction, Layout},
    Frame,
};

use crate::app::{App, AppState};

pub fn draw(frame: &mut Frame, app: &App) {
    match app.state {
        AppState::Menu => widgets::render_launch(frame, app, frame.area()),
        AppState::Dashboard => draw_dashboard(frame, app),
    }
}

fn draw_dashboard(frame: &mut Frame, app: &App) {
    let area = frame.area();

    // Outer: main block + one-line footer outside the border
    let outer = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(0), Constraint::Length(1)])
        .split(area);

    widgets::render_footer(frame, outer[1]);

    let main_block = widgets::make_main_block();
    let inner = main_block.inner(outer[0]);
    frame.render_widget(main_block, outer[0]);

    // Inner rows: header | CPU+MEM/DISK | NET+TEMP | BAT/HEALTH
    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),  // header status line
            Constraint::Min(9),     // CPU (left 54%) | MEM+DISK (right 46%)
            Constraint::Min(4),     // NET (left 54%) | TEMP (right 46%)
            Constraint::Length(3),  // BAT + HEALTH
        ])
        .split(inner);

    widgets::render_header(frame, app, rows[0]);

    // Top section split
    let top_cols = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(54), Constraint::Percentage(46)])
        .split(rows[1]);

    widgets::render_cpu(frame, app, top_cols[0]);

    // Right column: MEM on top, DISK below
    let right_rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(4), Constraint::Min(0)])
        .split(top_cols[1]);

    widgets::render_memory(frame, app, right_rows[0]);
    widgets::render_disk(frame, app, right_rows[1]);

    // Mid section split
    let mid_cols = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(54), Constraint::Percentage(46)])
        .split(rows[2]);

    widgets::render_network(frame, app, mid_cols[0]);
    widgets::render_thermal(frame, app, mid_cols[1]);

    widgets::render_power_health(frame, app, rows[3]);
}
