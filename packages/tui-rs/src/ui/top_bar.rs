//! Top bar: agent tabs · skill tabs · blueprint tab. Fits the width by dropping detail.

use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;

use super::{Theme, fit};
use crate::app::{App, View};
use crate::derive::{self, Wait};

pub struct Tab {
    pub index: usize,
    pub name: String,
    pub status: String,
    pub tokens: String,
}

/// Most detailed level that fits: full → no tokens → short names → index + status.
pub fn fit_tabs(tabs: &[Tab], width: usize) -> Vec<Tab> {
    let size = |t: &Tab| {
        [t.index.to_string(), t.name.clone(), t.status.clone(), t.tokens.clone()]
            .iter()
            .filter(|s| !s.is_empty())
            .map(|s| s.chars().count() + 1)
            .sum::<usize>()
            + 2
    };
    for level in 0..4 {
        let fitted: Vec<Tab> = tabs
            .iter()
            .map(|t| Tab {
                index: t.index,
                name: match level {
                    2 => fit(&t.name, 8),
                    3 => String::new(),
                    _ => t.name.clone(),
                },
                status: t.status.clone(),
                tokens: if level == 0 { t.tokens.clone() } else { String::new() },
            })
            .collect();
        if fitted.iter().map(size).sum::<usize>() <= width || level == 3 {
            return fitted;
        }
    }
    unreachable!()
}

fn color(w: &Wait) -> ratatui::style::Color {
    match w {
        Wait::Permission(_) | Wait::Question => Theme::YELLOW,
        Wait::Error(_) | Wait::Retry { .. } => Theme::RED,
        Wait::Done => Theme::GREEN,
        w if w.is_busy() => Theme::AQUA,
        _ => Theme::MUTED,
    }
}

pub fn render(f: &mut Frame, area: Rect, app: &App) {
    let ids = app.tabs();
    let tabs: Vec<Tab> = ids
        .iter()
        .enumerate()
        .map(|(i, id)| Tab {
            index: i,
            name: derive::agent_name(&app.store, id, i),
            status: {
                let w = app.wait(id);
                let s = derive::short(&w, app.now);
                if w.is_busy() && !matches!(w, Wait::Permission(_) | Wait::Question) {
                    format!("{}{}", crate::ui::ticker::spinner(app.now), s.trim_start_matches(['⏳', '●']))
                } else {
                    s
                }
            },
            tokens: {
                let t = derive::usage(&app.store, id).total();
                if t > 0.0 { derive::tokens(t) } else { String::new() }
            },
        })
        .collect();

    let skills = app.used_skills();
    let skills_text = if skills.is_empty() { String::new() } else { format!(" ┆ skills: {} ", skills.join(" · ")) };
    let (done, total) = crate::ui::blueprint::progress(app);
    let blueprint = if total > 0 { format!(" G blueprint {done}/{total} ") } else { " G blueprint ".to_string() };
    let usage_tab = " U usage ";
    let room = (area.width as usize).saturating_sub(blueprint.chars().count() + skills_text.chars().count() + 11);
    let fitted = fit_tabs(&tabs, room);

    let mut spans: Vec<Span> = Vec::new();
    for t in &fitted {
        let id = &ids[t.index];
        let active = app.view == View::Chat && *id == app.viewing;
        let base = if active { Style::default().bg(Theme::SELECTED) } else { Style::default() };
        spans.push(Span::styled(format!(" {}", t.index), base.fg(Theme::MUTED)));
        if !t.name.is_empty() {
            let s = if active { base.fg(Theme::TEXT).add_modifier(Modifier::BOLD) } else { base.fg(Theme::MUTED) };
            spans.push(Span::styled(format!(" {}", t.name), s));
        }
        if !t.status.is_empty() {
            spans.push(Span::styled(format!(" {}", t.status), base.fg(color(&app.wait(id)))));
        }
        if !t.tokens.is_empty() {
            spans.push(Span::styled(format!(" {}", t.tokens), base.fg(Theme::DIM)));
        }
        spans.push(Span::styled(" ", base));
        spans.push(Span::styled("│", Style::default().fg(Theme::DIM)));
    }
    let used: usize = spans.iter().map(|s| s.content.chars().count()).sum();
    if !skills_text.is_empty() {
        let active = matches!(app.view, View::Skill(_));
        spans.push(Span::styled(skills_text.clone(), Style::default().fg(if active { Theme::AQUA } else { Theme::MUTED })));
    }
    let pad = (area.width as usize).saturating_sub(used + skills_text.chars().count() + blueprint.chars().count() + usage_tab.len() - 1);
    spans.push(Span::raw(" ".repeat(pad)));
    let usage_active = app.view == View::Usage;
    spans.push(Span::styled(
        usage_tab,
        if usage_active { Style::default().bg(Theme::SELECTED).fg(Theme::TEXT).add_modifier(Modifier::BOLD) } else { Style::default().fg(Theme::BLUE) },
    ));
    let bp_active = app.view == View::Blueprint;
    spans.push(Span::styled(
        blueprint,
        if bp_active { Style::default().bg(Theme::SELECTED).fg(Theme::TEXT).add_modifier(Modifier::BOLD) } else { Style::default().fg(Theme::BLUE) },
    ));
    f.render_widget(Paragraph::new(Line::from(spans)), area);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tab(i: usize, name: &str, status: &str, tokens: &str) -> Tab {
        Tab { index: i, name: name.into(), status: status.into(), tokens: tokens.into() }
    }

    #[test]
    fn drops_detail_to_fit() {
        let tabs = [tab(0, "realestate-orchestrator", "⏳", "3.2M"), tab(1, "pantry", "✓", "77k")];
        assert_eq!(fit_tabs(&tabs, 200)[0].tokens, "3.2M");
        assert!(fit_tabs(&tabs, 46).iter().all(|t| t.tokens.is_empty()));
        assert_eq!(fit_tabs(&tabs, 30)[0].name, "realest…");
        assert!(fit_tabs(&tabs, 4).iter().all(|t| t.name.is_empty()));
    }
}
