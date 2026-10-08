//! T13 completion popup (above the input) and T14 Ctrl+P menu popup.

use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Clear, Paragraph};

use super::{Theme, centered, fit, panel};
use crate::app::App;

fn row(selected: bool, left: String, right: String, width: usize) -> Line<'static> {
    let style = if selected { Style::default().fg(Theme::TEXT).bg(Theme::SELECTED).add_modifier(Modifier::BOLD) } else { Theme::text() };
    let left = fit(&left, width.saturating_sub(2));
    let room = width.saturating_sub(left.chars().count() + 3);
    Line::from(vec![
        Span::styled(if selected { "▸ " } else { "  " }, style),
        Span::styled(left, style),
        Span::styled(format!(" {}", fit(&right, room)), Theme::muted()),
    ])
}

pub fn render_completion(f: &mut Frame, input: Rect, app: &App) {
    if app.menu_open {
        return;
    }
    let list = app.completions();
    if list.is_empty() {
        return;
    }
    let height = list.len() as u16 + 2;
    let width = input.width.min(72);
    let area = Rect { x: input.x, y: input.y.saturating_sub(height), width, height };
    let inner = width.saturating_sub(2) as usize;
    let lines: Vec<Line> = list
        .iter()
        .enumerate()
        .map(|(i, e)| row(i == app.popup_sel, format!("{}{}", if e.skill { "✦ " } else { "/ " }, e.name), e.description.clone(), inner))
        .collect();
    f.render_widget(Clear, area);
    f.render_widget(
        Paragraph::new(lines).block(panel("SKILLS & COMMANDS · tab complete · ⏎ run · esc close").style(Style::default().bg(Theme::PANEL))),
        area,
    );
}

pub fn render_menu(f: &mut Frame, area: Rect, app: &App) {
    if !app.menu_open {
        return;
    }
    let items = app.menu_visible();
    let shown = items.len().min(14);
    let rect = centered(area, 70, shown as u16 + 4);
    let inner = rect.width.saturating_sub(2) as usize;
    let start = app.popup_sel.saturating_sub(shown.saturating_sub(1));
    let mut lines = vec![Line::from(vec![
        Span::styled("› ", Style::default().fg(Theme::AQUA)),
        Span::styled(app.menu_query.clone(), Theme::text()),
        Span::styled("▏", Style::default().fg(Theme::AQUA)),
    ])];
    lines.push(Line::from(Span::styled("─".repeat(inner), Style::default().fg(Theme::DIM))));
    for (i, (label, _)) in items.iter().enumerate().skip(start).take(shown) {
        lines.push(row(i == app.popup_sel, label.clone(), String::new(), inner));
    }
    if items.is_empty() {
        lines.push(Line::from(Span::styled("  nothing matches", Theme::muted())));
    }
    f.render_widget(Clear, rect);
    f.render_widget(
        Paragraph::new(lines).block(panel("MENU · type to filter · ⏎ apply · esc close").border_style(Style::default().fg(Theme::BLUE)).style(Style::default().bg(Theme::PANEL))),
        rect,
    );
}
