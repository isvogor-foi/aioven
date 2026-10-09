//! Status line: agent, tier, model, notices and key hints.

use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;

use super::Theme;
use crate::app::App;
use crate::{budget, derive};

pub fn render(f: &mut Frame, area: Rect, app: &App) {
    let tier = budget::tier(&app.store.config, &app.agent).map(|t| t[..1].to_uppercase()).unwrap_or_default();
    let model = derive::model_name(&app.store, &app.root).unwrap_or_default();
    let model = app.model.as_ref().map(|(_, m)| m.clone()).unwrap_or(model);
    let left = format!(" {} {tier} {model}", app.agent);
    // most important first; drop from the end until the line fits
    let all = ["⇧⏎ send", "ctrl+p menu", "ctrl+0-9 tabs", "ctrl+g blueprint", "/ skills", "ctrl+↑ focus"];
    let room_for_hints = (area.width as usize).saturating_sub(left.chars().count() + 4);
    let mut hints = String::new();
    for h in all {
        let next = if hints.is_empty() { format!(" {h} ") } else { format!("{hints}· {h} ") };
        if next.chars().count() > room_for_hints {
            break;
        }
        hints = next;
    }
    let hints = hints.as_str();
    let retry = match app.wait(&app.viewing) {
        w @ derive::Wait::Retry { .. } => Some(derive::label(&w, app.now)),
        _ => None,
    };
    let notice = app.notice.clone().or(retry).unwrap_or_default();
    let room = (area.width as usize).saturating_sub(left.chars().count() + hints.chars().count() + 3);
    let notice: String = notice.chars().take(room).collect();
    let pad = (area.width as usize).saturating_sub(left.chars().count() + notice.chars().count() + hints.chars().count() + 3);
    f.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled(left, Style::default().fg(Theme::c().aqua)),
            Span::raw("   "),
            Span::styled(notice, Style::default().fg(Theme::c().red)),
            Span::raw(" ".repeat(pad)),
            Span::styled(hints, Theme::muted()),
        ])),
        area,
    );
}
