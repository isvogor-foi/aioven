//! R5 PlanModel: components of the approved recipe and the files they change
//! (port of the tested TS `plan-model.ts`, U7 bullet format).

use crate::types::{FileDiff, Todo};

#[derive(Debug, Clone, PartialEq)]
pub struct Component {
    pub name: String,
    pub new: bool,
    pub paths: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Status {
    Pending,
    InProgress,
    Completed,
}

/// `- **Name** (new|reuse) — paths: a.ts, b/ — responsibility`, inside a `### Components` section.
pub fn parse_plan(markdown: &str) -> Vec<Component> {
    let lines: Vec<&str> = markdown.lines().collect();
    let heading = |l: &str| l.starts_with('#') && l.trim_start_matches('#').starts_with(' ');
    let level = |l: &str| l.chars().take_while(|c| *c == '#').count();
    let Some(start) = lines.iter().position(|l| heading(l) && (2..=4).contains(&level(l)) && l.to_lowercase().contains("components")) else {
        return vec![];
    };
    let lvl = level(lines[start]);
    lines[start + 1..]
        .iter()
        .take_while(|l| !(heading(l) && level(l) <= lvl))
        .filter_map(|l| parse_bullet(l))
        .collect()
}

fn parse_bullet(line: &str) -> Option<Component> {
    let rest = line.trim_start().strip_prefix("- ").or_else(|| line.trim_start().strip_prefix("* "))?;
    let rest = rest.strip_prefix("**")?;
    let (name, rest) = rest.split_once("**")?;
    let rest = rest.trim_start().strip_prefix('(')?;
    let (kind, rest) = rest.split_once(')')?;
    let new = match kind.to_lowercase().as_str() {
        "new" => true,
        "reuse" => false,
        _ => return None,
    };
    let rest = rest.trim_start().trim_start_matches(['—', '–', '-']).trim_start();
    let lower = rest.to_lowercase();
    let after = if lower.starts_with("paths:") { &rest[6..] } else if lower.starts_with("path:") { &rest[5..] } else { return None };
    // paths run until the next " — " (or " - ") separator
    let paths_text = [" — ", " – ", " - "].iter().filter_map(|sep| after.find(sep)).min().map_or(after, |i| &after[..i]);
    let paths = paths_text
        .split(',')
        .map(|p| p.trim().trim_matches('`').to_string())
        .filter(|p| !p.is_empty())
        .collect();
    Some(Component { name: name.trim().to_string(), new, paths })
}

fn covers(path: &str, file: &str) -> bool {
    let dir = path.trim_end_matches('/');
    file == dir || file.starts_with(&format!("{dir}/"))
}

/// Files grouped by the first component whose path equals or contains them; the rest is "other".
pub fn assign<'a>(components: &[Component], files: &'a [FileDiff]) -> (Vec<Vec<&'a FileDiff>>, Vec<&'a FileDiff>) {
    let mut by = vec![Vec::new(); components.len()];
    let mut other = Vec::new();
    for f in files {
        let Some(name) = f.file.as_deref() else { continue };
        match components.iter().position(|c| c.paths.iter().any(|p| covers(p, name))) {
            Some(i) => by[i].push(f),
            None => other.push(f),
        }
    }
    (by, other)
}

pub fn status(component: &Component, todos: &[Todo]) -> Status {
    let name = component.name.to_lowercase();
    let mine: Vec<&Todo> = todos.iter().filter(|t| t.content.to_lowercase().contains(&name)).collect();
    if mine.iter().any(|t| t.status == "in_progress") {
        return Status::InProgress;
    }
    if !mine.is_empty() && mine.iter().all(|t| t.status == "completed" || t.status == "cancelled") {
        return Status::Completed;
    }
    Status::Pending
}

/// No recipe: one component per package (packages/<name>) or top-level folder.
pub fn infer(files: &[FileDiff]) -> Vec<Component> {
    let mut out: Vec<Component> = Vec::new();
    for f in files.iter().filter_map(|f| f.file.as_deref()) {
        let parts: Vec<&str> = f.split('/').collect();
        let prefix = if parts[0] == "packages" && parts.len() > 2 {
            format!("packages/{}", parts[1])
        } else if parts.len() > 1 {
            parts[0].to_string()
        } else {
            ".".into()
        };
        if out.iter().any(|c| c.paths.first().map(String::as_str) == Some(prefix.as_str()) || (prefix == "." && c.paths.is_empty())) {
            continue;
        }
        out.push(if prefix == "." {
            Component { name: "project root".into(), new: false, paths: vec![] }
        } else {
            Component { name: prefix.clone(), new: false, paths: vec![prefix] }
        });
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    const PLAN: &str = "# Retry\n\n## Architecture\n### Components\n- **RetryPolicy** (new) — paths: src/retry.ts — backoff\n- **FetchClient** (reuse) — paths: `src/client.ts`, src/http/ — calls retry\n* **ClientTests** (new) - paths: test/client.test.ts - tests\n- not a component\n\n### Interfaces\n- **Ignored** (new) — paths: x.ts — no\n";

    fn diff(f: &str) -> FileDiff {
        FileDiff { file: Some(f.into()), additions: 1, ..Default::default() }
    }

    #[test]
    fn parses_components_section_only() {
        let c = parse_plan(PLAN);
        assert_eq!(c.iter().map(|c| c.name.as_str()).collect::<Vec<_>>(), ["RetryPolicy", "FetchClient", "ClientTests"]);
        assert_eq!(c[1].paths, ["src/client.ts", "src/http/"]);
        assert!(c[0].new && !c[1].new);
        assert!(parse_plan("# x\n- **X** (new) — paths: a").is_empty());
    }

    #[test]
    fn assigns_files() {
        let c = parse_plan(PLAN);
        let files = [diff("src/retry.ts"), diff("src/http/agent.ts"), diff("src/httpx.ts"), diff("bun.lock")];
        let (by, other) = assign(&c, &files);
        assert_eq!(by[0].len(), 1);
        assert_eq!(by[1][0].file.as_deref(), Some("src/http/agent.ts"));
        assert!(by[2].is_empty());
        assert_eq!(other.len(), 2);
    }

    #[test]
    fn status_and_infer() {
        let c = Component { name: "RetryPolicy".into(), new: true, paths: vec![] };
        let t = |c: &str, s: &str| Todo { content: c.into(), status: s.into() };
        assert_eq!(status(&c, &[]), Status::Pending);
        assert_eq!(status(&c, &[t("build retrypolicy", "in_progress")]), Status::InProgress);
        assert_eq!(status(&c, &[t("RetryPolicy", "completed")]), Status::Completed);
        let names: Vec<String> = infer(&[diff("packages/tui/a.ts"), diff("packages/tui/b.ts"), diff("src/x.ts"), diff("README.md")]).into_iter().map(|c| c.name).collect();
        assert_eq!(names, ["packages/tui", "src", "project root"]);
    }
}
