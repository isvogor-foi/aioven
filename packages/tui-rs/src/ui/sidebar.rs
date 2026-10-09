//! Sidebar: tokens burned per agent, running tasks, recipe progress + ETA.

use ratatui::Frame;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;

use super::{Theme, fit, panel};
use crate::app::App;
use crate::budget;
use crate::derive::{self, Usage};

pub fn render(f: &mut Frame, area: Rect, app: &App) {
    let ids = app.tabs();
    let inner = area.width.saturating_sub(4) as usize;

    // TOKENS BURNED
    let mut tok: Vec<Line> = Vec::new();
    let mut total = Usage::default();
    for (i, id) in ids.iter().enumerate() {
        let name = derive::agent_name(&app.store, id, i);
        let u = derive::usage(&app.store, id);
        total = total.add(u);
        let b = budget::budget(&app.store.config, &name, i > 0);
        let used = u.total();
        let color = if used > b { Theme::c().red } else if used >= b * 0.8 { Theme::c().yellow } else { Theme::c().aqua };
        let label = fit(&name, 11);
        tok.push(Line::from(vec![
            Span::styled(format!("{label:<11} "), Theme::text()),
            Span::styled(format!("{:>5} ", derive::tokens(used)), Theme::muted()),
            Span::styled(crate::ui::ticker::bar_thick(used, b, inner.saturating_sub(18).clamp(4, 12)), Style::default().fg(color)),
        ]));
    }
    tok.push(Line::default());
    let cache = derive::cache_hit(&total).map(|c| format!(" · cache {c}%")).unwrap_or_default();
    tok.push(Line::from(Span::styled(format!("Σ {}{cache}", derive::tokens(total.total())), Style::default().fg(Theme::c().text).add_modifier(Modifier::BOLD))));
    tok.push(Line::from(Span::styled(
        format!("in {} · out {}", derive::tokens(total.input + total.cache_read + total.cache_write), derive::tokens(total.output)),
        Theme::muted(),
    )));

    // RUNNING
    let mut run: Vec<Line> = Vec::new();
    for (i, id) in ids.iter().enumerate() {
        let w = app.wait(id);
        if !w.is_busy() && !matches!(w, derive::Wait::Error(_)) {
            continue;
        }
        let name = derive::agent_name(&app.store, id, i);
        let color = match w {
            derive::Wait::Permission(_) | derive::Wait::Question => Theme::c().yellow,
            derive::Wait::Error(_) => Theme::c().red,
            _ => Theme::c().aqua,
        };
        let spin = if w.is_busy() { crate::ui::ticker::spinner(app.now) } else { "✗" };
        let live = app.live_output(id).map(|t| format!(" ↑{}", derive::tokens(t as f64))).unwrap_or_default();
        run.push(Line::from(vec![
            Span::styled(format!("{spin} "), Style::default().fg(color)),
            Span::styled(format!("{i} {} ", fit(&name, 10)), Theme::text()),
            Span::styled(fit(&format!("{}{live}", derive::label(&w, app.now)), inner.saturating_sub(15)), Style::default().fg(color)),
        ]));
    }
    if run.is_empty() {
        run.push(Line::from(Span::styled("nothing running", Theme::muted())));
    }
    let eta = derive::eta(&app.store, &app.root, app.now);
    if eta.total > 0 {
        run.push(Line::default());
        let mut l = vec![
            Span::styled("RECIPE ", Theme::title()),
            Span::styled(crate::ui::ticker::bar_thick(eta.done as f64, eta.total as f64, 6), Style::default().fg(Theme::c().blue)),
            Span::styled(format!(" {}/{}", eta.done, eta.total), Theme::text()),
        ];
        if let Some(ms) = eta.eta_ms {
            l.push(Span::styled(format!(" ~{}", derive::seconds(ms)), Theme::muted()));
        }
        run.push(Line::from(l));
    }

    // T28 AGENTS: every AIOven agent with its size and model, always shown
    let busy: Vec<String> = ids
        .iter()
        .enumerate()
        .filter(|(_, id)| app.wait(id).is_busy())
        .map(|(i, id)| derive::agent_name(&app.store, id, i))
        .collect();
    let agents: Vec<Line> = if app.agent_models.is_empty() {
        vec![Line::from(Span::styled("loading…", Theme::muted()))]
    } else {
        app.agent_models.iter().map(|a| agent_line(a, busy.contains(&a.name), inner)).collect()
    };

    let agents_h = agents.len() as u16 + 2;
    let tok_h = (tok.len() as u16 + 2).min(area.height.saturating_sub(agents_h) / 2 + 2);
    let parts = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(agents_h), Constraint::Length(tok_h), Constraint::Min(3)])
        .split(area);
    f.render_widget(Paragraph::new(agents).block(panel("AGENTS · ctrl+p models")), parts[0]);
    f.render_widget(Paragraph::new(tok).block(panel("TOKENS BURNED")), parts[1]);
    f.render_widget(Paragraph::new(run).block(panel("RUNNING")), parts[2]);
}

/// `● pantry  S gemini-flash`: dot lit while running; size letter (★ colour when it is the recommended size);
/// model dimmed when it is the server default.
pub fn agent_line(a: &crate::types::AgentModel, running: bool, width: usize) -> Line<'static> {
    let size = a.tier.chars().next().unwrap_or('?').to_ascii_uppercase();
    let model = a.model.as_deref().map(|m| m.rsplit('/').next().unwrap_or(m).to_string()).unwrap_or_else(|| "auto".into());
    let size_color = if a.tier == a.recommended { Theme::c().green } else { Theme::c().yellow };
    Line::from(vec![
        Span::styled(if running { "● " } else { "○ " }, Style::default().fg(if running { Theme::c().aqua } else { Theme::c().dim })),
        Span::styled(format!("{:<11} ", fit(&a.name, 11)), Theme::text()),
        Span::styled(format!("{size} "), Style::default().fg(size_color).add_modifier(Modifier::BOLD)),
        Span::styled(
            fit(&model, width.saturating_sub(16)),
            if a.source == "default" { Style::default().fg(Theme::c().dim) } else { Theme::muted() },
        ),
    ])
}
