//! R5 compact(): one-line tool summaries (port of the tested TS `compact.ts`).

use serde_json::Value;

use crate::types::ToolState;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Line {
    pub icon: &'static str,
    pub text: String,
}

fn s<'a>(v: &'a Value, k: &str) -> &'a str {
    v.get(k).and_then(Value::as_str).unwrap_or("")
}
fn n(v: &Value, k: &str) -> i64 {
    v.get(k).and_then(Value::as_i64).unwrap_or(0)
}

pub fn compact(tool: &str, state: &ToolState, relative: impl Fn(&str) -> String) -> Line {
    let input = state.input();
    let meta = state.metadata().cloned().unwrap_or(Value::Null);
    let icon = |done: &'static str| match state {
        ToolState::Error { .. } => "✗",
        ToolState::Pending { .. } | ToolState::Running { .. } => "⏳",
        ToolState::Completed { .. } => done,
    };
    let file = {
        let raw = if !s(input, "filePath").is_empty() { s(input, "filePath") } else { s(input, "path") };
        if raw.is_empty() { String::new() } else { relative(raw) }
    };
    let secs = match state {
        ToolState::Completed { time, .. } => format!(" ({}s)", ((time.end.unwrap_or(time.start) - time.start).max(0) as f64 / 1000.0).round()),
        _ => String::new(),
    };
    match tool {
        "edit" | "write" => {
            let d = meta.get("filediff").cloned().unwrap_or(Value::Null);
            let counts = if d.get("additions").is_some() { format!(" +{} −{}", n(&d, "additions"), n(&d, "deletions")) } else { String::new() };
            Line { icon: icon("✎"), text: format!("{} {file}{counts}", if tool == "write" { "wrote" } else { "edited" }) }
        }
        "apply_patch" => {
            let files = meta.get("files").and_then(Value::as_array).cloned().unwrap_or_default();
            let text = if files.is_empty() {
                "patch".to_string()
            } else {
                files.iter().map(|f| format!("{} +{} −{}", s(f, "relativePath"), n(f, "additions"), n(f, "deletions"))).collect::<Vec<_>>().join(", ")
            };
            Line { icon: icon("✎"), text: format!("edited {text}") }
        }
        "bash" => Line { icon: icon("⏵"), text: format!("{}{secs}", s(input, "command").lines().next().unwrap_or("")) },
        "read" => Line { icon: icon("·"), text: format!("read {file}") },
        "grep" | "glob" | "ast_grep" => Line { icon: icon("·"), text: format!("{tool} {}", s(input, "pattern")) },
        "task" => Line { icon: icon("⇢"), text: format!("{}: {}", if s(input, "subagent_type").is_empty() { "agent" } else { s(input, "subagent_type") }, s(input, "description")) },
        "todowrite" => Line { icon: icon("☰"), text: "updated recipe".into() },
        "skill" => Line { icon: icon("✦"), text: format!("skill {}", s(input, "name")) },
        _ => Line { icon: icon("·"), text: tool.to_string() },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::ToolTime;
    use serde_json::json;

    fn done(input: Value, metadata: Value) -> ToolState {
        ToolState::Completed { input, title: String::new(), metadata, time: ToolTime { start: 0, end: Some(12_000) } }
    }

    #[test]
    fn one_line_summaries() {
        let rel = |f: &str| f.trim_start_matches("/r/").to_string();
        assert_eq!(compact("edit", &done(json!({"filePath":"/r/src/a.ts"}), json!({"filediff":{"additions":4,"deletions":1}})), rel).text, "edited src/a.ts +4 −1");
        assert_eq!(compact("bash", &done(json!({"command":"bun test\nx"}), json!({})), rel), Line { icon: "⏵", text: "bun test (12s)".into() });
        assert_eq!(compact("task", &done(json!({"subagent_type":"pantry","description":"find calls"}), json!({})), rel).text, "pantry: find calls");
    }

    #[test]
    fn icons_follow_state() {
        let running = ToolState::Running { input: json!({"command":"ls"}), title: None, metadata: json!({}), time: ToolTime::default() };
        assert_eq!(compact("bash", &running, |f| f.into()).icon, "⏳");
        let err = ToolState::Error { input: json!({}), error: "x".into(), time: ToolTime::default() };
        assert_eq!(compact("read", &err, |f| f.into()).icon, "✗");
    }
}
