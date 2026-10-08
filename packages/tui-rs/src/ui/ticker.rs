//! T10 Tickers: spinner frames and thick bars (pure).

const FRAMES: [&str; 10] = ["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];

pub fn spinner(now_ms: i64) -> &'static str {
    FRAMES[((now_ms / 100).rem_euclid(FRAMES.len() as i64)) as usize]
}

pub fn bar_thick(value: f64, max: f64, width: usize) -> String {
    let filled = if max > 0.0 { ((value / max) * width as f64).round().min(width as f64) as usize } else { 0 };
    "█".repeat(filled) + &"▒".repeat(width - filled)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn frames_and_bars() {
        assert_eq!(spinner(0), "⠋");
        assert_eq!(spinner(100), "⠙");
        assert_eq!(spinner(1000), "⠋");
        assert_eq!(bar_thick(1.0, 4.0, 4), "█▒▒▒");
        assert_eq!(bar_thick(9.0, 4.0, 4), "████");
    }
}
