//! Modal dialogs: stop confirmation, permission (y/a/n), question (↑↓ ⏎ esc).

use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Clear, Paragraph, Wrap};

use super::{Theme, centered, panel};
use crate::app::{App, Dialog};
use crate::derive;

fn show(f: &mut Frame, area: Rect, title: &str, lines: Vec<Line>, accent: ratatui::style::Color) {
    let h = lines.len() as u16 + 2;
    let rect = centered(area, 64, h);
    f.render_widget(Clear, rect);
    f.render_widget(
        Paragraph::new(lines)
            .wrap(Wrap { trim: false })
            .block(panel(title).border_style(Style::default().fg(accent)).style(Style::default().bg(Theme::PANEL))),
        rect,
    );
}

fn keys(k: &[(&str, &str)]) -> Line<'static> {
    let mut spans = Vec::new();
    for (key, what) in k {
        spans.push(Span::styled(format!(" {key} "), Style::default().fg(Theme::TEXT).bg(Theme::SELECTED).add_modifier(Modifier::BOLD)));
        spans.push(Span::styled(format!(" {what}   "), Theme::muted()));
    }
    Line::from(spans)
}

pub fn render(f: &mut Frame, area: Rect, app: &App) {
    if let Some(d) = &app.dialog {
        let text = match d {
            Dialog::StopOne(id) => {
                let name = derive::agent_name(&app.store, id, if *id == app.root { 0 } else { 1 });
                format!("Stop {name}?")
            }
            Dialog::StopAll(ids) => format!("Stop {} busy agent(s)?", ids.len()),
        };
        show(f, area, "STOP", vec![Line::from(Span::styled(text, Theme::text())), Line::default(), keys(&[("y", "stop"), ("n", "keep baking")])], Theme::RED);
        return;
    }
    if let Some(p) = app.pending_permission() {
        let who = derive::agent_name(&app.store, &p.session_id, if p.session_id == app.root { 0 } else { 1 });
        let mut lines = vec![Line::from(vec![
            Span::styled(format!("{who} "), Theme::title()),
            Span::styled(format!("wants permission: {}", p.permission), Theme::text()),
        ])];
        for pat in p.patterns.iter().take(4) {
            lines.push(Line::from(Span::styled(format!("  {pat}"), Style::default().fg(Theme::CODE))));
        }
        lines.push(Line::default());
        lines.push(keys(&[("y", "once"), ("a", "always"), ("n", "reject")]));
        show(f, area, "PERMISSION", lines, Theme::YELLOW);
        return;
    }
    if let Some(q) = app.pending_question() {
        let Some(info) = q.questions.first() else { return };
        let mut lines = vec![Line::from(Span::styled(info.question.clone(), Theme::text())), Line::default()];
        for (i, o) in info.options.iter().enumerate() {
            let sel = i == app.question_sel;
            let style = if sel { Style::default().fg(Theme::TEXT).bg(Theme::SELECTED).add_modifier(Modifier::BOLD) } else { Theme::muted() };
            lines.push(Line::from(vec![
                Span::styled(if sel { " ▸ " } else { "   " }, style),
                Span::styled(o.label.clone(), style),
                Span::styled(if o.description.is_empty() { String::new() } else { format!("  {}", o.description) }, Theme::muted()),
            ]));
        }
        lines.push(Line::default());
        lines.push(keys(&[("↑↓", "choose"), ("⏎", "answer"), ("esc", "dismiss")]));
        show(f, area, if info.header.is_empty() { "QUESTION" } else { &info.header }, lines, Theme::AQUA);
    }
}
