//! Blueprint tab: recipe components with status and their changed files; skill tabs.

use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Paragraph, Wrap};

use super::{Theme, panel};
use crate::app::App;
use crate::plan_model::{self, Status};

/// (done, total) components of the recipe; (0, 0) without a recipe.
pub fn progress(app: &App) -> (usize, usize) {
    let Some(md) = app.recipe.as_deref() else { return (0, 0) };
    let components = plan_model::parse_plan(md);
    let todos = app.store.todos.get(&app.root).cloned().unwrap_or_default();
    let done = components.iter().filter(|c| plan_model::status(c, &todos) == Status::Completed).count();
    (done, components.len())
}

pub fn render(f: &mut Frame, area: Rect, app: &App) {
    let diffs = app.store.diffs.get(&app.root).cloned().unwrap_or_default();
    let todos = app.store.todos.get(&app.root).cloned().unwrap_or_default();
    let planned = app.recipe.as_deref().map(plan_model::parse_plan).unwrap_or_default();
    let (components, source) = if planned.is_empty() {
        (plan_model::infer(&diffs), "from changed files".to_string())
    } else {
        (planned.clone(), "from recipe".to_string())
    };
    let (by, other) = plan_model::assign(&components, &diffs);
    let interfaces = app.recipe.as_deref().map(plan_model::parse_interfaces).unwrap_or_default();
    let links = app.recipe.as_deref().map(plan_model::parse_communication).unwrap_or_default();
    let mut lines = vec![Line::from(Span::styled(source, Theme::muted())), Line::default()];
    for (i, c) in components.iter().enumerate() {
        if planned.is_empty() && by[i].is_empty() {
            continue;
        }
        let st = if planned.is_empty() { Status::InProgress } else { plan_model::status(c, &todos) };
        let (icon, color) = match st {
            Status::Completed => ("✓", Theme::GREEN),
            Status::InProgress => ("⏳", Theme::AQUA),
            Status::Pending => ("○", Theme::MUTED),
        };
        lines.push(Line::from(vec![
            Span::styled(format!("{icon} "), Style::default().fg(color)),
            Span::styled(c.name.clone(), Theme::text().add_modifier(Modifier::BOLD)),
            Span::styled(if c.new { "  new" } else { "  reuse" }, Theme::muted()),
        ]));
        for (_, sig) in interfaces.iter().filter(|(n, _)| *n == c.name) {
            lines.push(Line::from(vec![
                Span::styled("    ◇ ", Style::default().fg(Theme::BLUE)),
                Span::styled(sig.clone(), Style::default().fg(Theme::CODE)),
            ]));
        }
        if by[i].is_empty() {
            lines.push(Line::from(Span::styled(format!("    {}", if c.paths.is_empty() { "no files yet".into() } else { c.paths.join(", ") }), Style::default().fg(Theme::DIM))));
        }
        for d in &by[i] {
            lines.push(Line::from(vec![
                Span::styled(format!("    {}", d.file.clone().unwrap_or_default()), Theme::text()),
                Span::styled(format!(" +{}", d.additions), Style::default().fg(Theme::GREEN)),
                Span::styled(format!(" −{}", d.deletions), Style::default().fg(Theme::RED)),
            ]));
        }
    }
    if !links.is_empty() {
        lines.push(Line::default());
        lines.push(Line::from(Span::styled("COMMUNICATION", Theme::muted().add_modifier(Modifier::BOLD))));
        for (from, to, what) in &links {
            lines.push(Line::from(vec![
                Span::styled(format!("    {from} "), Theme::text()),
                Span::styled("→ ", Style::default().fg(Theme::AQUA)),
                Span::styled(format!("{to}  "), Theme::text()),
                Span::styled(what.clone(), Theme::muted()),
            ]));
        }
    }
    if !other.is_empty() {
        lines.push(Line::default());
        lines.push(Line::from(Span::styled("OTHER CHANGED FILES", Theme::muted().add_modifier(Modifier::BOLD))));
        for d in other {
            lines.push(Line::from(vec![
                Span::styled(format!("    {}", d.file.clone().unwrap_or_default()), Theme::text()),
                Span::styled(format!(" +{}", d.additions), Style::default().fg(Theme::GREEN)),
                Span::styled(format!(" −{}", d.deletions), Style::default().fg(Theme::RED)),
            ]));
        }
    }
    if components.is_empty() && diffs.is_empty() {
        lines.push(Line::from(Span::styled("No recipe and no changed files yet.", Theme::muted())));
    }
    let (done, total) = progress(app);
    let title = if total > 0 { format!("BLUEPRINT {done}/{total}") } else { "BLUEPRINT".into() };
    let focused = app.focus == crate::keys::Focus::Files;
    let max_scroll = (lines.len() as u16).saturating_sub(area.height.saturating_sub(2));
    f.render_widget(
        Paragraph::new(lines)
            .block(super::panel_focus(&title, focused))
            .wrap(Wrap { trim: false })
            .scroll((app.files_scroll.min(max_scroll), 0)),
        area,
    );
}

pub fn render_skill(f: &mut Frame, area: Rect, app: &App, name: &str) {
    let skill = app.store.skills.iter().find(|s| s.name == name);
    let mut lines = vec![
        Line::from(Span::styled(name.to_string(), Theme::title())),
        Line::default(),
        Line::from(Span::styled(skill.and_then(|s| s.description.clone()).unwrap_or_else(|| "(no description)".into()), Theme::text())),
        Line::default(),
        Line::from(Span::styled("Used in this session by:", Theme::muted())),
    ];
    for (i, id) in app.tabs().iter().enumerate() {
        let used = app.store.messages(id).iter().any(|m| {
            app.store.parts(m.id()).iter().any(|p| matches!(&p.kind, crate::types::PartKind::Tool { tool, state, .. } if tool == "skill" && state.input().get("name").and_then(|v| v.as_str()) == Some(name)))
        });
        if used {
            lines.push(Line::from(Span::styled(format!("  {i} {}", crate::derive::agent_name(&app.store, id, i)), Theme::text())));
        }
    }
    lines.push(Line::default());
    lines.push(Line::from(Span::styled("esc back · alt+s next skill", Theme::muted())));
    f.render_widget(Paragraph::new(lines).block(panel("SKILL")).wrap(Wrap { trim: false }), area);
}
