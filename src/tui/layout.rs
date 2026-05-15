use ratatui::layout::{Constraint, Direction, Layout, Rect};

pub fn app_layout(area: Rect) -> (Rect, Rect, Rect, Rect) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(2),  // header + sep
            Constraint::Min(1),     // content
            Constraint::Length(1),  // ask bar
            Constraint::Length(1),  // nav
        ])
        .split(area);
    (chunks[0], chunks[1], chunks[2], chunks[3])
}

pub fn centered_rect(area: Rect, percent_x: u16, percent_y: u16) -> Rect {
    let py = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Fill(1), Constraint::Length((area.height * percent_y) / 100), Constraint::Fill(1)])
        .split(area);
    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Fill(1), Constraint::Length((area.width * percent_x) / 100), Constraint::Fill(1)])
        .split(py[1])[1]
}