//! T22 Usage: pure calendar maths for the usage page (heat map per day, month totals, summary).

use serde::Deserialize;

#[derive(Debug, Clone, Default, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Day {
    pub day: String,
    #[serde(default)]
    pub input: f64,
    #[serde(default)]
    pub output: f64,
    #[serde(default)]
    pub reasoning: f64,
    #[serde(default)]
    pub cache_read: f64,
    #[serde(default)]
    pub cache_write: f64,
    #[serde(default)]
    pub cost: f64,
    #[serde(default)]
    pub messages: f64,
}

impl Day {
    /// Tokens the model processed that day (fresh input + output + reasoning; cache reads counted separately).
    pub fn tokens(&self) -> f64 {
        self.input + self.output + self.reasoning
    }
}

#[derive(Debug, Clone, Default, Deserialize, PartialEq)]
pub struct Usage {
    #[serde(default)]
    pub first: Option<String>,
    #[serde(default)]
    pub days: Vec<Day>,
}

/// Days since 1970-01-01 for a civil date (Howard Hinnant's algorithm).
pub fn days_from_civil(y: i64, m: i64, d: i64) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let mp = (m + 9) % 12;
    let doy = (153 * mp + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

pub fn parse(day: &str) -> Option<(i64, i64, i64)> {
    let mut it = day.split('-').map(|p| p.parse::<i64>().ok());
    Some((it.next()??, it.next()??, it.next()??))
}

/// 0 = Monday … 6 = Sunday
pub fn weekday(days: i64) -> i64 {
    (days + 3).rem_euclid(7)
}

/// Heat level 0–4 by quartiles of the non-zero values in the set.
pub fn levels(values: &[f64]) -> impl Fn(f64) -> u8 + '_ {
    let mut nz: Vec<f64> = values.iter().copied().filter(|v| *v > 0.0).collect();
    nz.sort_by(|a, b| a.partial_cmp(b).unwrap());
    move |v: f64| {
        if v <= 0.0 || nz.is_empty() {
            return 0;
        }
        let q = |p: f64| nz[((nz.len() - 1) as f64 * p).round() as usize];
        if v <= q(0.25) {
            1
        } else if v <= q(0.5) {
            2
        } else if v <= q(0.75) {
            3
        } else {
            4
        }
    }
}

/// Year heat map: `weeks[w][d]` = Some(tokens) for days of `year`, Monday-first columns.
pub fn year_grid(days: &[Day], year: i64) -> Vec<[Option<f64>; 7]> {
    let start = days_from_civil(year, 1, 1);
    let end = days_from_civil(year + 1, 1, 1);
    let first_monday = start - weekday(start);
    let weeks = ((end - first_monday) + 6) / 7;
    let mut grid = vec![[None; 7]; weeks as usize];
    for d in start..end {
        let w = ((d - first_monday) / 7) as usize;
        grid[w][weekday(d) as usize] = Some(0.0);
    }
    for day in days {
        let Some((y, m, dd)) = parse(&day.day) else { continue };
        if y != year {
            continue;
        }
        let d = days_from_civil(y, m, dd);
        let w = ((d - first_monday) / 7) as usize;
        if let Some(cell) = grid.get_mut(w).and_then(|c| c.get_mut(weekday(d) as usize)) {
            *cell = Some(cell.unwrap_or(0.0) + day.tokens());
        }
    }
    grid
}

/// Tokens per month (index 0 = January) for `year`.
pub fn month_totals(days: &[Day], year: i64) -> [f64; 12] {
    let mut out = [0.0; 12];
    for d in days {
        if let Some((y, m, _)) = parse(&d.day) {
            if y == year && (1..=12).contains(&m) {
                out[(m - 1) as usize] += d.tokens();
            }
        }
    }
    out
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Summary {
    pub tokens: f64,
    pub cache_read: f64,
    pub cost: f64,
    pub messages: f64,
    pub active_days: usize,
    pub busiest: Option<(String, f64)>,
}

pub fn summary(days: &[Day]) -> Summary {
    let busiest = days.iter().max_by(|a, b| a.tokens().partial_cmp(&b.tokens()).unwrap()).map(|d| (d.day.clone(), d.tokens()));
    Summary {
        tokens: days.iter().map(Day::tokens).sum(),
        cache_read: days.iter().map(|d| d.cache_read).sum(),
        cost: days.iter().map(|d| d.cost).sum(),
        messages: days.iter().map(|d| d.messages).sum(),
        active_days: days.iter().filter(|d| d.tokens() > 0.0).count(),
        busiest,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn day(d: &str, input: f64) -> Day {
        Day { day: d.into(), input, ..Default::default() }
    }

    #[test]
    fn calendar_basics() {
        assert_eq!(days_from_civil(1970, 1, 1), 0);
        assert_eq!(weekday(days_from_civil(2026, 10, 9)), 4); // Friday
        assert_eq!(parse("2026-04-28"), Some((2026, 4, 28)));
    }

    #[test]
    fn grid_places_days_by_week_and_weekday() {
        let g = year_grid(&[day("2026-01-01", 10.0), day("2026-01-05", 5.0), day("2025-12-31", 99.0)], 2026);
        assert_eq!(g[0][3], Some(10.0)); // Thu 1 Jan in week 0
        assert_eq!(g[0][0], None); // Mon 29 Dec 2025 is outside 2026
        assert_eq!(g[1][0], Some(5.0)); // Mon 5 Jan
        assert!(g.len() >= 53);
    }

    #[test]
    fn levels_months_summary() {
        let lv = levels(&[0.0, 1.0, 2.0, 3.0, 4.0]);
        assert_eq!((lv(0.0), lv(1.0), lv(4.0)), (0, 1, 4));
        let days = [day("2026-04-28", 100.0), day("2026-08-20", 300.0), day("2025-08-01", 7.0)];
        let m = month_totals(&days, 2026);
        assert_eq!((m[3], m[7], m[0]), (100.0, 300.0, 0.0));
        let s = summary(&days);
        assert_eq!(s.tokens, 407.0);
        assert_eq!(s.busiest, Some(("2026-08-20".into(), 300.0)));
        assert_eq!(s.active_days, 3);
    }
}
