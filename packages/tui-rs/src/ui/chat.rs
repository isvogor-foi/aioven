//! Chat pane: conversation of the selected tab — markdown text and one-line tool calls.

use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span, Text};
use ratatui::widgets::{Paragraph, Wrap};
use serde_json::Value;

use super::{Theme, panel_focus};
use crate::app::App;
use crate::compact::compact;
use crate::derive;
use crate::markdown::{self, Palette};
use crate::types::*;

fn palette() -> Palette {
    Palette { text: Theme::TEXT, muted: Theme::MUTED, accent: Theme::AQUA, code: Theme::CODE }
}

fn relative(dir: &str) -> impl Fn(&str) -> String + '_ {
    move |f: &str| f.strip_prefix(dir).map(|r| r.trim_start_matches('/').to_string()).unwrap_or_else(|| f.to_string())
}

pub fn build(app: &App) -> Text<'static> {
    let mut lines: Vec<Line<'static>> = Vec::new();
    let dir = app.store.sessions.get(&app.root).map(|s| s.directory.clone()).unwrap_or_default();
    let messages = app.store.messages(&app.viewing);
    for (index, message) in messages.iter().enumerate() {
        // one footer per turn: only after the last assistant message before the next user message
        let turn_end = !matches!(messages.get(index + 1), Some(Message::Assistant(_)));
        let parts = app.store.parts(message.id());
        match message {
            Message::User(_) => {
                let text: String = parts
                    .iter()
                    .filter_map(|p| match &p.kind {
                        PartKind::Text { text } => Some(text.as_str()),
                        _ => None,
                    })
                    .collect::<Vec<_>>()
                    .join("\n");
                if text.is_empty() {
                    continue;
                }
                lines.push(Line::default());
                for (i, l) in text.lines().enumerate() {
                    let head = if i == 0 { Span::styled("you ▸ ", Style::default().fg(Theme::BLUE).add_modifier(Modifier::BOLD)) } else { Span::raw("      ") };
                    lines.push(Line::from(vec![head, Span::styled(l.to_string(), Theme::text().add_modifier(Modifier::BOLD))]));
                }
                lines.push(Line::default());
            }
            Message::Assistant(a) => {
                for p in parts {
                    match &p.kind {
                        PartKind::Text { text } if !text.trim().is_empty() => {
                            lines.extend(markdown::render(text, palette()).lines);
                        }
                        PartKind::Reasoning { text } if !app.compact && !text.trim().is_empty() => {
                            for l in text.lines() {
                                lines.push(Line::from(Span::styled(format!("  {l}"), Style::default().fg(Theme::DIM).add_modifier(Modifier::ITALIC))));
                            }
                        }
                        PartKind::Tool { tool, state, .. } => {
                            let c = compact(tool, state, relative(&dir));
                            let color = match c.icon {
                                "✗" => Theme::RED,
                                "⏳" => Theme::AQUA,
                                _ => Theme::MUTED,
                            };
                            lines.push(Line::from(vec![
                                Span::styled(format!("  {} ", c.icon), Style::default().fg(color)),
                                Span::styled(c.text, Theme::muted()),
                            ]));
                            if !app.compact {
                                let detail = match state {
                                    ToolState::Completed { metadata, .. } => {
                                        metadata.get("output").and_then(Value::as_str).map(str::to_string).unwrap_or_default()
                                    }
                                    ToolState::Error { error, .. } => error.clone(),
                                    _ => String::new(),
                                };
                                for l in detail.lines().take(8) {
                                    lines.push(Line::from(Span::styled(format!("      {l}"), Style::default().fg(Theme::DIM))));
                                }
                            }
                        }
                        _ => {}
                    }
                }
                if let Some(err) = &a.error {
                    let w = if err.get("name").and_then(Value::as_str) == Some("MessageAbortedError") { "pulled out".to_string() } else { derive::label(&derive::Wait::Error(err.get("data").and_then(|d| d.get("message")).and_then(Value::as_str).unwrap_or("error").to_string()), 0) };
                    lines.push(Line::from(Span::styled(format!("  ■ {w}"), Style::default().fg(Theme::RED))));
                }
                if a.time.completed.is_some() && turn_end {
                    lines.push(Line::from(Span::styled(
                        format!("  ▣ {} · {}", a.agent, a.model_id),
                        Style::default().fg(Theme::DIM),
                    )));
                }
            }
        }
    }
    // T18: a stuck turn must say why (quota, auth, network…)
    if let derive::Wait::Retry { .. } = app.wait(&app.viewing) {
        lines.push(Line::default());
        lines.push(Line::from(Span::styled(
            format!("  ↻ {}", derive::label(&app.wait(&app.viewing), app.now)),
            Style::default().fg(Theme::RED).add_modifier(Modifier::BOLD),
        )));
        lines.push(Line::from(Span::styled(
            "    esc stop · ctrl+p → model or connect provider to switch",
            Theme::muted(),
        )));
    }
    Text::from(lines)
}

pub fn render(f: &mut Frame, area: Rect, app: &App) {
    let name = derive::agent_name(&app.store, &app.viewing, if app.viewing == app.root { 0 } else { 1 });
    let title = format!("CHAT · {name}{}", if app.compact { "" } else { " · full" });
    let text = build(app);
    let width = area.width.saturating_sub(2).max(1);
    // estimate wrapped height to keep the view pinned to the bottom
    let total: u16 = text
        .lines
        .iter()
        .map(|l| ((l.width() as u16).max(1) + width - 1) / width)
        .sum();
    let visible = area.height.saturating_sub(2);
    let bottom = total.saturating_sub(visible);
    let scroll = bottom.saturating_sub(app.chat_scroll);
    let focused = app.focus == crate::keys::Focus::Chat;
    let title = if app.chat_scroll > 0 && bottom > 0 { format!("{title} · ↑{}", app.chat_scroll.min(bottom)) } else { title };
    f.render_widget(Paragraph::new(text).block(panel_focus(&title, focused)).wrap(Wrap { trim: false }).scroll((scroll, 0)), area);
}
