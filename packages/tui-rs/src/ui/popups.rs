//! T13 completion popup (above the input) and T14 Ctrl+P menu popup.

use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Clear, Paragraph};

use super::{Theme, centered, fit, panel};
use crate::app::App;

fn row(selected: bool, left: String, right: String, width: usize) -> Line<'static> {
    let style = if selected { Style::default().fg(Theme::c().text).bg(Theme::c().selected).add_modifier(Modifier::BOLD) } else { Theme::text() };
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
    let files = app.file_completions();
    if !files.is_empty() {
        let height = files.len() as u16 + 2;
        let width = input.width.min(72);
        let area = Rect { x: input.x, y: input.y.saturating_sub(height), width, height };
        let inner = width.saturating_sub(2) as usize;
        let lines: Vec<Line> =
            files.iter().enumerate().map(|(i, p)| row(i == app.popup_sel, format!("@ {p}"), String::new(), inner)).collect();
        f.render_widget(Clear, area);
        f.render_widget(
            Paragraph::new(lines).block(panel("FILES · tab/⏎ insert · esc close").style(Style::default().bg(Theme::c().panel))),
            area,
        );
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
        Paragraph::new(lines).block(panel("SKILLS & COMMANDS · tab complete · ⏎ run · esc close").style(Style::default().bg(Theme::c().panel))),
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
        Span::styled("› ", Style::default().fg(Theme::c().aqua)),
        Span::styled(app.menu_query.clone(), Theme::text()),
        Span::styled("▏", Style::default().fg(Theme::c().aqua)),
    ])];
    lines.push(Line::from(Span::styled("─".repeat(inner), Style::default().fg(Theme::c().dim))));
    for (i, (label, _)) in items.iter().enumerate().skip(start).take(shown) {
        lines.push(row(i == app.popup_sel, label.clone(), String::new(), inner));
    }
    if items.is_empty() {
        lines.push(Line::from(Span::styled("  nothing matches", Theme::muted())));
    }
    f.render_widget(Clear, rect);
    f.render_widget(
        Paragraph::new(lines).block(panel("MENU · type to filter · ⏎ apply · esc close").border_style(Style::default().fg(Theme::c().blue)).style(Style::default().bg(Theme::c().panel))),
        rect,
    );
}

pub fn render_connect(f: &mut Frame, area: Rect, app: &App) {
    use crate::connect::{Step, filter};
    let Some(step) = &app.connect else { return };
    let rect = centered(area, 72, 18);
    let inner = rect.width.saturating_sub(2) as usize;
    let mut lines: Vec<Line> = Vec::new();
    let title = match step {
        Step::Pick { query, sel } => {
            lines.push(Line::from(vec![Span::styled("› ", Style::default().fg(Theme::c().aqua)), Span::styled(query.clone(), Theme::text()), Span::styled("▏", Style::default().fg(Theme::c().aqua))]));
            lines.push(Line::from(Span::styled("─".repeat(inner), Style::default().fg(Theme::c().dim))));
            let list = filter(&app.connect_providers, query);
            if app.connect_providers.is_empty() {
                lines.push(Line::from(Span::styled("  loading providers…", Theme::muted())));
            }
            let start = sel.saturating_sub(11);
            for (i, p) in list.iter().enumerate().skip(start).take(12) {
                lines.push(row(i == *sel, p.name.clone(), if p.connected { "✓ connected".into() } else { p.id.clone() }, inner));
            }
            "CONNECT PROVIDER · type to filter · ⏎ choose · esc close"
        }
        Step::Method { provider, methods, sel } => {
            lines.push(Line::from(Span::styled(format!("How do you want to log in to {}?", provider.name), Theme::text())));
            lines.push(Line::default());
            for (i, m) in methods.iter().enumerate() {
                lines.push(row(i == *sel, m.label.clone(), if m.oauth { "browser".into() } else { "api key".into() }, inner));
            }
            "CONNECT · ↑↓ ⏎"
        }
        Step::Key { provider, key } => {
            lines.push(Line::from(Span::styled(format!("Paste your {} API key:", provider.name), Theme::text())));
            lines.push(Line::default());
            lines.push(Line::from(vec![Span::styled("  ", Theme::text()), Span::styled("•".repeat(key.chars().count().min(inner - 4)), Theme::text()), Span::styled("▏", Style::default().fg(Theme::c().aqua))]));
            lines.push(Line::default());
            lines.push(Line::from(Span::styled("⏎ save · esc cancel", Theme::muted())));
            "CONNECT · API KEY"
        }
        Step::Oauth { provider, url, auto, instructions, code, .. } => {
            lines.push(Line::from(Span::styled(format!("Log in to {} in your browser:", provider.name), Theme::text())));
            lines.push(Line::from(Span::styled(url.clone(), Style::default().fg(Theme::c().code))));
            if !instructions.is_empty() {
                lines.push(Line::default());
                lines.push(Line::from(Span::styled(instructions.clone(), Style::default().fg(Theme::c().yellow))));
            }
            lines.push(Line::default());
            if *auto {
                lines.push(Line::from(vec![Span::styled(crate::ui::ticker::spinner(app.now), Style::default().fg(Theme::c().aqua)), Span::styled(" waiting for the browser login to finish… (esc cancel)", Theme::muted())]));
            } else {
                lines.push(Line::from(vec![Span::styled("code: ", Theme::muted()), Span::styled(code.clone(), Theme::text()), Span::styled("▏", Style::default().fg(Theme::c().aqua))]));
                lines.push(Line::from(Span::styled("paste the code · ⏎ finish · esc cancel", Theme::muted())));
            }
            "CONNECT · BROWSER LOGIN"
        }
        Step::Busy(text) => {
            lines.push(Line::from(vec![Span::styled(format!("{} ", crate::ui::ticker::spinner(app.now)), Style::default().fg(Theme::c().aqua)), Span::styled(text.clone(), Theme::text())]));
            "CONNECT"
        }
    };
    f.render_widget(Clear, rect);
    f.render_widget(
        Paragraph::new(lines).wrap(ratatui::widgets::Wrap { trim: false }).block(panel(title).border_style(Style::default().fg(Theme::c().blue)).style(Style::default().bg(Theme::c().panel))),
        rect,
    );
}

/// T23 sessions popup: "+ New session" then past sessions (title · age · tokens).
pub fn render_sessions(f: &mut Frame, area: Rect, app: &App) {
    let Some((query, sel)) = &app.sessions_popup else { return };
    let list = app.sessions_visible();
    let rect = centered(area, 80, 20);
    let inner = rect.width.saturating_sub(2) as usize;
    let shown = 15usize;
    let mut lines = vec![
        Line::from(vec![Span::styled("› ", Style::default().fg(Theme::c().aqua)), Span::styled(query.clone(), Theme::text()), Span::styled("▏", Style::default().fg(Theme::c().aqua))]),
        Line::from(Span::styled("─".repeat(inner), Style::default().fg(Theme::c().dim))),
    ];
    let start = sel.saturating_sub(shown - 1);
    let age = |ms: i64| {
        let d = (app.now - ms).max(0);
        if d >= 48 * 3_600_000 { format!("{}d", d / 86_400_000) } else { crate::derive::seconds(d) }
    };
    let rows: Vec<(String, String)> = std::iter::once(("+ New session".to_string(), String::new()))
        .chain(list.iter().map(|s| {
            let tok = s.tokens.as_ref().map(|t| t.input + t.output + t.reasoning).unwrap_or(0.0);
            (if s.title.is_empty() { s.id.clone() } else { s.title.clone() }, format!("{} ago · {}", age(s.time.updated), crate::derive::tokens(tok)))
        }))
        .collect();
    for (i, (left, right)) in rows.iter().enumerate().skip(start).take(shown) {
        lines.push(row(i == *sel, left.clone(), right.clone(), inner));
    }
    if app.session_list.is_empty() {
        lines.push(Line::from(Span::styled("  loading sessions…", Theme::muted())));
    }
    f.render_widget(Clear, rect);
    f.render_widget(
        Paragraph::new(lines).block(panel("SESSIONS · type to filter · ⏎ open · esc close").border_style(Style::default().fg(Theme::c().blue)).style(Style::default().bg(Theme::c().panel))),
        rect,
    );
}

/// P5 rename popup: one line of text.
pub fn render_rename(f: &mut Frame, area: Rect, app: &App) {
    let Some(text) = &app.rename else { return };
    let rect = centered(area, 70, 3);
    f.render_widget(Clear, rect);
    f.render_widget(
        Paragraph::new(Line::from(vec![Span::styled(text.clone(), Theme::text()), Span::styled("▏", Style::default().fg(Theme::c().aqua))]))
            .block(panel("RENAME SESSION · ⏎ save · esc cancel").border_style(Style::default().fg(Theme::c().blue)).style(Style::default().bg(Theme::c().panel))),
        rect,
    );
}

/// T29 Agents & models: agent list, then the choices for one agent (recommended size first).
pub fn render_agents(f: &mut Frame, area: Rect, app: &App) {
    let Some(p) = &app.agents_popup else { return };
    let rows = app.agents_popup_rows();
    let shown = 16usize;
    let rect = centered(area, 84, (rows.len().min(shown) as u16) + 4);
    let inner = rect.width.saturating_sub(2) as usize;
    let agent = p.agent.and_then(|i| app.agent_models.get(i));
    let mut lines = vec![match agent {
        Some(a) => Line::from(vec![
            Span::styled("› ", Style::default().fg(Theme::c().aqua)),
            Span::styled(p.query.clone(), Theme::text()),
            Span::styled("▏", Style::default().fg(Theme::c().aqua)),
            Span::styled(format!("   recommended size for {}: {}", a.name, a.recommended), Theme::muted()),
        ]),
        None => Line::from(Span::styled("choose an agent; sizes: S small · M medium · L large", Theme::muted())),
    }];
    lines.push(Line::from(Span::styled("─".repeat(inner), Style::default().fg(Theme::c().dim))));
    let start = p.sel.saturating_sub(shown - 1);
    for (i, (label, _)) in rows.iter().enumerate().skip(start).take(shown) {
        lines.push(row(i == p.sel, label.clone(), String::new(), inner));
    }
    if rows.is_empty() {
        lines.push(Line::from(Span::styled("  loading agents…", Theme::muted())));
    }
    let title = match agent {
        Some(a) => format!("{} MODEL · type to filter · ⏎ use · esc back", a.name.to_uppercase()),
        None => "AGENTS & MODELS · ⏎ choose · esc close".into(),
    };
    f.render_widget(Clear, rect);
    f.render_widget(
        Paragraph::new(lines).block(panel(&title).border_style(Style::default().fg(Theme::c().blue)).style(Style::default().bg(Theme::c().panel))),
        rect,
    );
}
