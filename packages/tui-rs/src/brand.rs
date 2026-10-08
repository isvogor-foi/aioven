//! R10 Brand: Rust copy of `packages/tui/src/brand.ts` (glyphs + forced blue colours).

use ratatui::style::Color;

pub const NAME: &str = "AIOven";
pub const COMMAND: &str = "aioven";

pub const LEFT: [&str; 4] = ["     ▀", "▀▀▀█ █", "█^^█ █", "▀▀▀▀ ▀"];
pub const RIGHT: [&str; 4] = ["                   ", "█▀▀█ █  █ █▀▀█ █▀▀▄", "█__█ ▀▄▄▀ █^^^ █__█", "▀▀▀▀  ▀▀  ▀▀▀▀ ▀~~▀"];

pub const AQUA: (u8, u8, u8) = (0x22, 0xd3, 0xee);
pub const BLUE: (u8, u8, u8) = (0x3b, 0x82, 0xf6);
pub const AQUA_SHADOW: (u8, u8, u8) = (0x0e, 0x4a, 0x5a);
pub const BLUE_SHADOW: (u8, u8, u8) = (0x1e, 0x3a, 0x8a);

pub fn color(c: (u8, u8, u8)) -> Color {
    Color::Rgb(c.0, c.1, c.2)
}

fn draw(line: &str, fg: (u8, u8, u8), shadow: (u8, u8, u8)) -> String {
    let f = format!("\x1b[38;2;{};{};{}m", fg.0, fg.1, fg.2);
    let sf = format!("\x1b[38;2;{};{};{}m", shadow.0, shadow.1, shadow.2);
    let sb = format!("\x1b[48;2;{};{};{}m", shadow.0, shadow.1, shadow.2);
    line.chars()
        .map(|c| match c {
            '_' => format!("{sb} \x1b[0m"),
            '^' => format!("{f}{sb}▀\x1b[0m"),
            '~' => format!("{sf}▀\x1b[0m"),
            ' ' => " ".to_string(),
            c => format!("{f}{c}\x1b[0m"),
        })
        .collect()
}

/// Exit-screen logo, truecolor blue.
pub fn ansi_logo(pad: &str) -> Vec<String> {
    LEFT.iter()
        .zip(RIGHT.iter())
        .map(|(l, r)| format!("{pad}{} {}", draw(l, AQUA, AQUA_SHADOW), draw(r, BLUE, BLUE_SHADOW)))
        .collect()
}

#[cfg(test)]
mod tests {
    #[test]
    fn matches_typescript_brand() {
        let ts = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/../tui/src/brand.ts")).unwrap();
        for line in super::LEFT.iter().chain(super::RIGHT.iter()) {
            assert!(ts.contains(&format!("\"{line}\"")), "glyph line missing in brand.ts: {line}");
        }
        assert!(ts.contains("0x22, 0xd3, 0xee") && ts.contains("0x3b, 0x82, 0xf6"));
    }
}
