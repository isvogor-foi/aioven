//! R9 Markdown: pulldown-cmark → ratatui Text (headings, emphasis, lists, quotes, code, tables).

use pulldown_cmark::{CodeBlockKind, Event, HeadingLevel, Options, Parser, Tag, TagEnd};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span, Text};

#[derive(Clone, Copy)]
pub struct Palette {
    pub text: Color,
    pub muted: Color,
    pub accent: Color,
    pub code: Color,
}

pub fn render(markdown: &str, p: Palette) -> Text<'static> {
    let mut lines: Vec<Line<'static>> = Vec::new();
    let mut current: Vec<Span<'static>> = Vec::new();
    let mut styles: Vec<Style> = vec![Style::default().fg(p.text)];
    let mut list_depth: Vec<Option<u64>> = Vec::new();
    let mut in_code = false;
    let mut quote = 0usize;
    let mut row: Vec<String> = Vec::new();
    let mut cell = String::new();
    let mut in_cell = false;

    let flush = |lines: &mut Vec<Line<'static>>, current: &mut Vec<Span<'static>>, quote: usize| {
        if current.is_empty() {
            return;
        }
        let mut spans = Vec::new();
        if quote > 0 {
            spans.push(Span::styled("│ ".repeat(quote), Style::default().fg(p.muted)));
        }
        spans.append(current);
        lines.push(Line::from(spans));
    };

    for event in Parser::new_ext(markdown, Options::ENABLE_TABLES | Options::ENABLE_STRIKETHROUGH) {
        let style = *styles.last().unwrap_or(&Style::default());
        match event {
            Event::Start(Tag::Heading { level, .. }) => {
                flush(&mut lines, &mut current, quote);
                let mut s = Style::default().fg(p.accent).add_modifier(Modifier::BOLD);
                if level == HeadingLevel::H1 {
                    s = s.add_modifier(Modifier::UNDERLINED);
                }
                styles.push(s);
            }
            Event::End(TagEnd::Heading(_)) => {
                styles.pop();
                flush(&mut lines, &mut current, quote);
            }
            Event::Start(Tag::Emphasis) => styles.push(style.add_modifier(Modifier::ITALIC)),
            Event::Start(Tag::Strong) => styles.push(style.add_modifier(Modifier::BOLD)),
            Event::Start(Tag::Strikethrough) => styles.push(style.add_modifier(Modifier::CROSSED_OUT)),
            Event::Start(Tag::Link { .. }) => styles.push(style.fg(p.accent).add_modifier(Modifier::UNDERLINED)),
            Event::End(TagEnd::Emphasis | TagEnd::Strong | TagEnd::Strikethrough | TagEnd::Link) => {
                styles.pop();
            }
            Event::Start(Tag::BlockQuote(_)) => {
                flush(&mut lines, &mut current, quote);
                quote += 1;
            }
            Event::End(TagEnd::BlockQuote(_)) => {
                flush(&mut lines, &mut current, quote);
                quote = quote.saturating_sub(1);
            }
            Event::Start(Tag::List(start)) => {
                flush(&mut lines, &mut current, quote);
                list_depth.push(start);
            }
            Event::End(TagEnd::List(_)) => {
                flush(&mut lines, &mut current, quote);
                list_depth.pop();
            }
            Event::Start(Tag::Item) => {
                flush(&mut lines, &mut current, quote);
                let indent = "  ".repeat(list_depth.len().saturating_sub(1));
                let marker = match list_depth.last_mut() {
                    Some(Some(n)) => {
                        let m = format!("{n}. ");
                        *n += 1;
                        m
                    }
                    _ => "• ".to_string(),
                };
                current.push(Span::styled(format!("{indent}{marker}"), Style::default().fg(p.muted)));
            }
            Event::End(TagEnd::Item) => flush(&mut lines, &mut current, quote),
            Event::Start(Tag::CodeBlock(kind)) => {
                flush(&mut lines, &mut current, quote);
                in_code = true;
                if let CodeBlockKind::Fenced(lang) = kind {
                    if !lang.is_empty() {
                        lines.push(Line::from(Span::styled(format!("  ⟨{lang}⟩"), Style::default().fg(p.muted))));
                    }
                }
            }
            Event::End(TagEnd::CodeBlock) => in_code = false,
            Event::Start(Tag::Table(_)) => flush(&mut lines, &mut current, quote),
            Event::Start(Tag::TableHead | Tag::TableRow) => row.clear(),
            Event::Start(Tag::TableCell) => {
                in_cell = true;
                cell.clear();
            }
            Event::End(TagEnd::TableCell) => {
                in_cell = false;
                row.push(cell.trim().to_string());
            }
            Event::End(TagEnd::TableHead) => {
                lines.push(Line::from(Span::styled(row.join(" │ "), Style::default().fg(p.text).add_modifier(Modifier::BOLD))));
                lines.push(Line::from(Span::styled("─".repeat(row.join(" │ ").chars().count()), Style::default().fg(p.muted))));
            }
            Event::End(TagEnd::TableRow) => lines.push(Line::from(Span::styled(row.join(" │ "), Style::default().fg(p.text)))),
            Event::Text(text) => {
                if in_cell {
                    cell.push_str(&text);
                } else if in_code {
                    for l in text.lines() {
                        lines.push(Line::from(Span::styled(format!("  {l}"), Style::default().fg(p.code))));
                    }
                } else {
                    current.push(Span::styled(text.into_string(), style));
                }
            }
            Event::Code(code) => {
                if in_cell {
                    cell.push_str(&code);
                } else {
                    current.push(Span::styled(code.into_string(), Style::default().fg(p.code)));
                }
            }
            Event::SoftBreak => current.push(Span::raw(" ")),
            Event::HardBreak => flush(&mut lines, &mut current, quote),
            Event::End(TagEnd::Paragraph) => {
                flush(&mut lines, &mut current, quote);
                if list_depth.is_empty() {
                    lines.push(Line::default());
                }
            }
            Event::Rule => lines.push(Line::from(Span::styled("────────", Style::default().fg(p.muted)))),
            _ => {}
        }
    }
    flush(&mut lines, &mut current, quote);
    while lines.last().is_some_and(|l| l.spans.is_empty()) {
        lines.pop();
    }
    Text::from(lines)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn plain(t: &Text) -> Vec<String> {
        t.lines.iter().map(|l| l.spans.iter().map(|s| s.content.as_ref()).collect()).collect()
    }

    #[test]
    fn renders_common_blocks() {
        let p = Palette { text: Color::White, muted: Color::Gray, accent: Color::Cyan, code: Color::Yellow };
        let out = plain(&render("# Title\n\nSome *text* and `code`.\n\n- a\n- b\n\n1. one\n\n```rs\nfn x() {}\n```\n\n| A | B |\n|---|---|\n| 1 | 2 |\n", p));
        assert_eq!(out[0], "Title");
        assert!(out.contains(&"Some text and code.".to_string()));
        assert!(out.contains(&"• a".to_string()) && out.contains(&"1. one".to_string()));
        assert!(out.contains(&"  fn x() {}".to_string()));
        assert!(out.contains(&"A │ B".to_string()) && out.contains(&"1 │ 2".to_string()));
    }
}
