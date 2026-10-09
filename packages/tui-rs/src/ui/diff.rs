//! P7 Diff view: a changed file's unified patch, coloured.

use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Paragraph, Wrap};

use super::{Theme, panel_focus};
use crate::app::App;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Header,
    Hunk,
    Add,
    Del,
    Context,
}

/// Classify each line of a unified patch (pure).
pub fn lines(patch: &str) -> Vec<(Kind, String)> {
    patch
        .lines()
        .filter(|l| !l.starts_with("Index:") && !l.starts_with("===") && !l.starts_with("\\ No newline"))
        .map(|l| {
            let kind = if l.starts_with("+++") || l.starts_with("---") || l.starts_with("diff ") {
                Kind::Header
            } else if l.starts_with("@@") {
                Kind::Hunk
            } else if l.starts_with('+') {
                Kind::Add
            } else if l.starts_with('-') {
                Kind::Del
            } else {
                Kind::Context
            };
            (kind, l.to_string())
        })
        .collect()
}

pub fn render(f: &mut Frame, area: Rect, app: &App, file: &str) {
    let diff = app.store.diffs.get(&app.root).and_then(|d| d.iter().find(|d| d.file.as_deref() == Some(file)).cloned());
    let body: Vec<Line> = match diff.as_ref().and_then(|d| d.patch.as_deref()) {
        Some(patch) if !patch.trim().is_empty() => lines(patch)
            .into_iter()
            .map(|(k, l)| {
                let style = match k {
                    Kind::Header => Theme::muted().add_modifier(Modifier::BOLD),
                    Kind::Hunk => Style::default().fg(Theme::c().blue),
                    Kind::Add => Style::default().fg(Theme::c().green),
                    Kind::Del => Style::default().fg(Theme::c().red),
                    Kind::Context => Theme::text(),
                };
                Line::from(Span::styled(l, style))
            })
            .collect(),
        _ => vec![Line::from(Span::styled("No patch for this file (binary, or the diff is not loaded yet).", Theme::muted()))],
    };
    let stats = diff.map(|d| format!(" +{} −{}", d.additions, d.deletions)).unwrap_or_default();
    let max_scroll = (body.len() as u16).saturating_sub(area.height.saturating_sub(2));
    f.render_widget(
        Paragraph::new(body)
            .block(panel_focus(&format!("DIFF {file}{stats} · ↑↓ scroll · esc back"), false))
            .wrap(Wrap { trim: false })
            .scroll((app.files_scroll.min(max_scroll), 0)),
        area,
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classifies_patch_lines() {
        let patch = "Index: a.rs\n===\n--- a.rs\n+++ a.rs\n@@ -1,2 +1,2 @@\n keep\n-old\n+new";
        let kinds: Vec<Kind> = lines(patch).into_iter().map(|(k, _)| k).collect();
        assert_eq!(kinds, [Kind::Header, Kind::Header, Kind::Hunk, Kind::Context, Kind::Del, Kind::Add]);
    }
}
