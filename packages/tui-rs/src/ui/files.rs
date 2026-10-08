//! Files pane: changed files of the session tree with +/− and their recipe component.

use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;

use super::{Theme, fit, panel_focus};
use crate::app::App;
use crate::plan_model;

pub fn render(f: &mut Frame, area: Rect, app: &App) {
    let diffs = app.store.diffs.get(&app.root).cloned().unwrap_or_default();
    let components = app.recipe.as_deref().map(plan_model::parse_plan).unwrap_or_default();
    let (by, _) = plan_model::assign(&components, &diffs);
    let owner = |file: &str| -> Option<&str> {
        by.iter().enumerate().find(|(_, list)| list.iter().any(|d| d.file.as_deref() == Some(file))).map(|(i, _)| components[i].name.as_str())
    };
    let title = format!("FILES CHANGED {}", if diffs.is_empty() { String::new() } else { diffs.len().to_string() });
    let inner_w = area.width.saturating_sub(4) as usize;
    let lines: Vec<Line> = if diffs.is_empty() {
        vec![Line::from(Span::styled("No changes yet.", Theme::muted()))]
    } else {
        diffs
            .iter()
            .filter_map(|d| d.file.as_deref().map(|file| (d, file)))
            .map(|(d, file)| {
                let icon = match d.status.as_deref() {
                    Some("added") => Span::styled("+ ", Style::default().fg(Theme::GREEN)),
                    Some("deleted") => Span::styled("− ", Style::default().fg(Theme::RED)),
                    _ => Span::styled("✎ ", Style::default().fg(Theme::AQUA)),
                };
                let counts = format!(" +{} −{}", d.additions, d.deletions);
                let comp = owner(file).map(|c| format!("  {c}")).unwrap_or_default();
                let room = inner_w.saturating_sub(2 + counts.chars().count() + comp.chars().count());
                let name = fit(file, room);
                let pad = inner_w.saturating_sub(2 + name.chars().count() + counts.chars().count() + comp.chars().count());
                Line::from(vec![
                    icon,
                    Span::styled(name, Theme::text()),
                    Span::styled(format!(" +{}", d.additions), Style::default().fg(Theme::GREEN)),
                    Span::styled(format!(" −{}", d.deletions), Style::default().fg(Theme::RED)),
                    Span::raw(" ".repeat(pad)),
                    Span::styled(comp, Style::default().fg(Theme::BLUE)),
                ])
            })
            .collect()
    };
    let max_scroll = (lines.len() as u16).saturating_sub(area.height.saturating_sub(2));
    let focused = app.focus == crate::keys::Focus::Files;
    f.render_widget(Paragraph::new(lines).block(panel_focus(&title, focused)).scroll((app.files_scroll.min(max_scroll), 0)), area);
}
