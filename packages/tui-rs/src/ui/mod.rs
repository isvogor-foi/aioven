//! R6 Views: pure render functions over the App state.

mod blueprint;
mod chat;
mod diff;
mod dialogs;
mod popups;
mod sidebar;
mod status;
mod top_bar;
mod usage;
pub mod ticker;
#[cfg(test)]
mod tests;

use ratatui::Frame;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::widgets::{Block, BorderType, Borders};

use crate::app::{App, View};
use crate::brand;

/// P15: colour palette (named; chosen with Ctrl+P → Theme, saved as `aioven.theme`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Palette {
    pub text: Color,
    pub muted: Color,
    pub dim: Color,
    pub aqua: Color,
    pub blue: Color,
    pub panel: Color,
    pub selected: Color,
    pub green: Color,
    pub yellow: Color,
    pub red: Color,
    pub code: Color,
}

pub const THEMES: [&str; 4] = ["blue", "midnight", "mono", "light"];

const fn rgb(c: (u8, u8, u8)) -> Color {
    Color::Rgb(c.0, c.1, c.2)
}

/// Palette by name (unknown names fall back to the AIOven blue).
pub fn palette(name: &str) -> Palette {
    let blue = Palette {
        text: rgb((0xe6, 0xf4, 0xfa)),
        muted: rgb((0x7f, 0x9b, 0xb0)),
        dim: rgb((0x3d, 0x6a, 0x85)),
        aqua: rgb(brand::AQUA),
        blue: rgb(brand::BLUE),
        panel: rgb((0x10, 0x23, 0x32)),
        selected: rgb((0x1b, 0x36, 0x4a)),
        green: rgb((0x7f, 0xd8, 0x8f)),
        yellow: rgb((0xe5, 0xc0, 0x7b)),
        red: rgb((0xe0, 0x6c, 0x75)),
        code: rgb((0x67, 0xe8, 0xf9)),
    };
    match name {
        "midnight" => Palette {
            muted: rgb((0x8b, 0x8f, 0xc0)),
            dim: rgb((0x4a, 0x4e, 0x86)),
            aqua: rgb((0xa5, 0xb4, 0xfc)),
            blue: rgb((0x81, 0x8c, 0xf8)),
            panel: rgb((0x16, 0x17, 0x33)),
            selected: rgb((0x27, 0x29, 0x55)),
            code: rgb((0xc4, 0xb5, 0xfd)),
            ..blue
        },
        "mono" => Palette {
            text: rgb((0xee, 0xee, 0xee)),
            muted: rgb((0x9a, 0x9a, 0x9a)),
            dim: rgb((0x5c, 0x5c, 0x5c)),
            aqua: rgb((0xff, 0xff, 0xff)),
            blue: rgb((0xc8, 0xc8, 0xc8)),
            panel: rgb((0x1c, 0x1c, 0x1c)),
            selected: rgb((0x33, 0x33, 0x33)),
            code: rgb((0xd0, 0xd0, 0xd0)),
            ..blue
        },
        "light" => Palette {
            text: rgb((0x10, 0x23, 0x32)),
            muted: rgb((0x4b, 0x63, 0x75)),
            dim: rgb((0x9a, 0xb3, 0xc4)),
            aqua: rgb((0x08, 0x7e, 0xa4)),
            blue: rgb((0x1d, 0x5f, 0xd1)),
            panel: rgb((0xee, 0xf5, 0xfa)),
            selected: rgb((0xd2, 0xe6, 0xf3)),
            green: rgb((0x1f, 0x8a, 0x3b)),
            yellow: rgb((0x9a, 0x67, 0x00)),
            red: rgb((0xc0, 0x2b, 0x37)),
            code: rgb((0x0b, 0x6b, 0x8a)),
        },
        _ => blue,
    }
}

static ACTIVE: std::sync::RwLock<Option<Palette>> = std::sync::RwLock::new(None);

pub struct Theme;
impl Theme {
    /// Active palette.
    pub fn c() -> Palette {
        ACTIVE.read().ok().and_then(|p| *p).unwrap_or_else(|| palette("blue"))
    }
    pub fn set(name: &str) {
        if let Ok(mut p) = ACTIVE.write() {
            *p = Some(palette(name));
        }
    }

    pub fn text() -> Style {
        Style::default().fg(Self::c().text)
    }
    pub fn muted() -> Style {
        Style::default().fg(Self::c().muted)
    }
    pub fn title() -> Style {
        Style::default().fg(Self::c().aqua).add_modifier(Modifier::BOLD)
    }
}

/// Thick-bordered panel with a small uppercase title.
pub fn panel(title: &str) -> Block<'static> {
    Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Thick)
        .border_style(Style::default().fg(Theme::c().dim))
        .title(ratatui::text::Span::styled(format!(" {title} "), Theme::title()))
}

/// Panel whose border lights up when it has keyboard focus.
pub fn panel_focus(title: &str, focused: bool) -> Block<'static> {
    let title = if focused { format!("{title} · focused · ↑↓ scroll · esc back") } else { title.to_string() };
    panel(&title).border_style(Style::default().fg(if focused { Theme::c().aqua } else { Theme::c().dim }))
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
                .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
                .split(body[0]);
            // T16d: the blueprint (components, interfaces, files) is the main view; chat below
            app.files_area = middle[0];
            app.chat_area = middle[1];
            blueprint::render(f, middle[0], app);
            chat::render(f, middle[1], app);
        }
        View::Blueprint => blueprint::render(f, body[0], app),
        View::Skill(name) => blueprint::render_skill(f, body[0], app, name),
        View::Usage => usage::render(f, body[0], app),
        View::Diff(file) => diff::render(f, body[0], app, file),
    }
    sidebar::render(f, body[1], app);
    render_input(f, rows[2], app);
    status::render(f, rows[3], app);
    popups::render_completion(f, rows[2], app);
    popups::render_menu(f, f.area(), app);
    popups::render_connect(f, f.area(), app);
    popups::render_sessions(f, f.area(), app);
    popups::render_rename(f, f.area(), app);
    dialogs::render(f, f.area(), app);
}

fn render_input(f: &mut Frame, area: Rect, app: &mut App) {
    let title = format!("{} ▸", app.agent);
    app.input.set_block(panel(&title).border_style(Style::default().fg(Theme::c().blue)));
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
