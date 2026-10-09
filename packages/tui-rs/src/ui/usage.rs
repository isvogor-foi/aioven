//! T22 Usage page: yearly heat map of tokens per day, monthly bars, totals since installation.

use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;

use super::{Theme, panel};
use crate::app::App;
use crate::derive::tokens;
use crate::usage::{days_from_civil, levels, month_totals, summary, year_grid};

const SHADES: [Color; 5] = [
    Color::Rgb(0x15, 0x2c, 0x3e),
    Color::Rgb(0x0e, 0x4a, 0x5a),
    Color::Rgb(0x16, 0x7c, 0x99),
    Color::Rgb(0x22, 0xb0, 0xd0),
    Color::Rgb(0x22, 0xd3, 0xee),
];
const MONTHS: [&str; 12] = ["Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec"];
const WEEKDAYS: [&str; 7] = ["Mon", "", "Wed", "", "Fri", "", "Sun"];

pub fn render(f: &mut Frame, area: Rect, app: &App) {
    let Some(u) = &app.usage else {
        f.render_widget(Paragraph::new(Span::styled("loading usage…", Theme::muted())).block(panel("USAGE")), area);
        return;
    };
    let year = app.usage_year;
    let s = summary(&u.days);
    let mut lines: Vec<Line> = vec![
        Line::from(vec![
            Span::styled(format!("Since {} ", u.first.clone().unwrap_or_else(|| "—".into())), Theme::muted()),
            Span::styled(format!("Σ {} tokens", tokens(s.tokens)), Theme::text().add_modifier(Modifier::BOLD)),
            Span::styled(format!(" · cache reads {} · {} messages · {} active days", tokens(s.cache_read), tokens(s.messages), s.active_days), Theme::muted()),
            Span::styled(if s.cost > 0.0 { format!(" · ${:.2}", s.cost) } else { String::new() }, Theme::muted()),
        ]),
        Line::from(Span::styled(
            s.busiest.as_ref().map(|(d, t)| format!("Busiest day {d}: {} tokens", tokens(*t))).unwrap_or_default(),
            Theme::muted(),
        )),
        Line::default(),
        Line::from(vec![
            Span::styled("← ", Style::default().fg(Theme::AQUA)),
            Span::styled(format!("{year}"), Theme::title()),
            Span::styled(" →", Style::default().fg(Theme::AQUA)),
            Span::styled("   tokens per day", Theme::muted()),
        ]),
    ];

    // heat map: 7 weekday rows × weeks; 2 columns per cell when it fits
    let grid = year_grid(&u.days, year);
    let values: Vec<f64> = grid.iter().flat_map(|w| w.iter().filter_map(|c| *c)).collect();
    let level = levels(&values);
    let room = area.width.saturating_sub(8) as usize;
    let cell = if grid.len() * 2 <= room { 2 } else { 1 };
    let first_monday = {
        let start = days_from_civil(year, 1, 1);
        start - crate::usage::weekday(start)
    };
    // month labels above the week where each month starts
    let mut header = vec![' '; grid.len() * cell];
    for (m, name) in MONTHS.iter().enumerate() {
        let w = ((days_from_civil(year, m as i64 + 1, 1) - first_monday) / 7) as usize * cell;
        for (i, ch) in name.chars().enumerate() {
            if let Some(slot) = header.get_mut(w + i) {
                *slot = ch;
            }
        }
    }
    lines.push(Line::from(vec![Span::raw("     "), Span::styled(header.into_iter().collect::<String>(), Theme::muted())]));
    for (d, label) in WEEKDAYS.iter().enumerate() {
        let mut spans = vec![Span::styled(format!("{label:<4} "), Theme::muted())];
        for week in &grid {
            match week[d] {
                None => spans.push(Span::raw(" ".repeat(cell))),
                Some(v) => spans.push(Span::styled(if cell == 2 { "■ " } else { "■" }, Style::default().fg(SHADES[level(v) as usize]))),
            }
        }
        lines.push(Line::from(spans));
    }
    let mut legend = vec![Span::styled("     less ", Theme::muted())];
    for c in SHADES {
        legend.push(Span::styled("■ ", Style::default().fg(c)));
    }
    legend.push(Span::styled("more", Theme::muted()));
    lines.push(Line::from(legend));
    lines.push(Line::default());

    // months
    let months = month_totals(&u.days, year);
    let max = months.iter().cloned().fold(0.0, f64::max);
    let bar_w = (area.width as usize).saturating_sub(20).min(48);
    for (m, total) in months.iter().enumerate() {
        let filled = if max > 0.0 { ((total / max) * bar_w as f64).round() as usize } else { 0 };
        lines.push(Line::from(vec![
            Span::styled(format!("{}  ", MONTHS[m]), Theme::muted()),
            Span::styled("█".repeat(filled), Style::default().fg(Theme::BLUE)),
            Span::styled("▒".repeat(bar_w - filled), Style::default().fg(Theme::DIM)),
            Span::styled(format!(" {}", if *total > 0.0 { tokens(*total) } else { "·".into() }), Theme::text()),
        ]));
    }
    f.render_widget(Paragraph::new(lines).block(panel("USAGE · ←/→ year · esc back")), area);
}
