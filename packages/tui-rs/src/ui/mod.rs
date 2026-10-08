//! R6 Views: pure render functions over the App state.

mod blueprint;
mod chat;
mod dialogs;
mod files;
mod popups;
mod sidebar;
mod status;
mod top_bar;
pub mod ticker;

use ratatui::Frame;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::widgets::{Block, BorderType, Borders};

use crate::app::{App, View};
use crate::brand;

/// AIOven palette: blue/aqua accents on the terminal's own background.
pub struct Theme;
impl Theme {
    pub const TEXT: Color = Color::Rgb(0xe6, 0xf4, 0xfa);
    pub const MUTED: Color = Color::Rgb(0x7f, 0x9b, 0xb0);
    pub const DIM: Color = Color::Rgb(0x3d, 0x6a, 0x85);
    pub const AQUA: Color = Color::Rgb(brand::AQUA.0, brand::AQUA.1, brand::AQUA.2);
    pub const BLUE: Color = Color::Rgb(brand::BLUE.0, brand::BLUE.1, brand::BLUE.2);
    pub const PANEL: Color = Color::Rgb(0x10, 0x23, 0x32);
    pub const SELECTED: Color = Color::Rgb(0x1b, 0x36, 0x4a);
    pub const GREEN: Color = Color::Rgb(0x7f, 0xd8, 0x8f);
    pub const YELLOW: Color = Color::Rgb(0xe5, 0xc0, 0x7b);
    pub const RED: Color = Color::Rgb(0xe0, 0x6c, 0x75);
    pub const CODE: Color = Color::Rgb(0x67, 0xe8, 0xf9);

    pub fn text() -> Style {
        Style::default().fg(Self::TEXT)
    }
    pub fn muted() -> Style {
        Style::default().fg(Self::MUTED)
    }
    pub fn title() -> Style {
        Style::default().fg(Self::AQUA).add_modifier(Modifier::BOLD)
    }
}

/// Thick-bordered panel with a small uppercase title.
pub fn panel(title: &str) -> Block<'static> {
    Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Thick)
        .border_style(Style::default().fg(Theme::DIM))
        .title(ratatui::text::Span::styled(format!(" {title} "), Theme::title()))
}

/// Panel whose border lights up when it has keyboard focus.
pub fn panel_focus(title: &str, focused: bool) -> Block<'static> {
    let title = if focused { format!("{title} · focused · ↑↓ scroll · esc back") } else { title.to_string() };
    panel(&title).border_style(Style::default().fg(if focused { Theme::AQUA } else { Theme::DIM }))
}

pub fn render(f: &mut Frame, app: &mut App) {
    let input_height = (app.input.lines().len() as u16).clamp(1, 6) + 2;
    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(1), Constraint::Min(6), Constraint::Length(input_height), Constraint::Length(1)])
        .split(f.area());
    top_bar::render(f, rows[0], app);

    let body = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Min(30), Constraint::Length(32)])
        .split(rows[1]);
    match &app.view {
        View::Chat => {
            let middle = Layout::default()
                .direction(Direction::Vertical)
                .constraints([Constraint::Percentage(40), Constraint::Percentage(60)])
                .split(body[0]);
            app.files_area = middle[0];
            app.chat_area = middle[1];
            files::render(f, middle[0], app);
            chat::render(f, middle[1], app);
        }
        View::Blueprint => blueprint::render(f, body[0], app),
        View::Skill(name) => blueprint::render_skill(f, body[0], app, name),
    }
    sidebar::render(f, body[1], app);
    render_input(f, rows[2], app);
    status::render(f, rows[3], app);
    popups::render_completion(f, rows[2], app);
    popups::render_menu(f, f.area(), app);
    dialogs::render(f, f.area(), app);
}

fn render_input(f: &mut Frame, area: Rect, app: &mut App) {
    let title = format!("{} ▸", app.agent);
    app.input.set_block(panel(&title).border_style(Style::default().fg(Theme::BLUE)));
    app.input.set_style(Theme::text());
    f.render_widget(&app.input, area);
}

pub fn centered(area: Rect, width: u16, height: u16) -> Rect {
    let w = width.min(area.width.saturating_sub(2));
    let h = height.min(area.height.saturating_sub(2));
    Rect { x: area.x + (area.width - w) / 2, y: area.y + (area.height - h) / 2, width: w, height: h }
}

pub fn fit(text: &str, max: usize) -> String {
    if text.chars().count() <= max {
        return text.to_string();
    }
    if max <= 1 {
        return String::new();
    }
    let mut s: String = text.chars().take(max - 1).collect();
    s.push('…');
    s
}
