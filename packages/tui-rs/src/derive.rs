//! R5 Derive: pure views over the Store (port of the tested TS `derive.ts`).

use serde_json::Value;

use crate::store::Store;
use crate::types::*;

pub const MAX_CHILDREN: usize = 9;

#[derive(Debug, Clone, PartialEq)]
pub enum Wait {
    Idle,
    Done,
    Error(String),
    Interrupted,
    Permission(String),
    Question,
    Compacting,
    Retry { attempt: i64, next: i64 },
    Subagent { since: i64 },
    Tool { name: String, since: i64 },
    Model { since: i64 },
    Thinking,
    Streaming,
}

impl Wait {
    pub fn is_busy(&self) -> bool {
        !matches!(self, Wait::Idle | Wait::Done | Wait::Error(_) | Wait::Interrupted)
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Usage {
    pub input: f64,
    pub output: f64,
    pub cache_read: f64,
    pub cache_write: f64,
}

impl Usage {
    pub fn total(&self) -> f64 {
        self.input + self.output + self.cache_read + self.cache_write
    }
    pub fn add(self, o: Usage) -> Usage {
        Usage {
            input: self.input + o.input,
            output: self.output + o.output,
            cache_read: self.cache_read + o.cache_read,
            cache_write: self.cache_write + o.cache_write,
        }
    }
}

pub fn root(store: &Store, session: &str) -> String {
    let mut current = session.to_string();
    for _ in 0..64 {
        match store.sessions.get(&current).and_then(|s| s.parent_id.clone()) {
            Some(parent) if store.sessions.contains_key(&parent) => current = parent,
            _ => break,
        }
    }
    current
}

/// Child session ids referenced by `task` tool parts of a session (old children may be missing from lists).
pub fn task_sessions(store: &Store, session: &str) -> Vec<String> {
    store
        .messages(session)
        .iter()
        .flat_map(|m| store.parts(m.id()))
        .filter_map(|p| match &p.kind {
            PartKind::Tool { tool, state, .. } if tool == "task" => {
                state.metadata().and_then(|m| m.get("sessionId")).and_then(Value::as_str).map(str::to_string)
            }
            _ => None,
        })
        .collect()
}

/// Tab order: index 0 = root, 1..=9 = direct children by creation; busy children win when there are more.
pub fn agents(store: &Store, session: &str) -> Vec<String> {
    let root_id = root(store, session);
    let mut kids: Vec<&Session> =
        store.sessions.values().filter(|s| s.parent_id.as_deref() == Some(root_id.as_str())).collect();
    kids.sort_by(|a, b| (a.time.created, &a.id).cmp(&(b.time.created, &b.id)));
    if kids.len() > MAX_CHILDREN {
        let busy = |s: &&Session| wait_of(store, &s.id).is_busy();
        let mut active: Vec<&Session> = kids.iter().copied().filter(busy).collect();
        let rest: Vec<&Session> = kids.iter().copied().filter(|s| !busy(s)).collect();
        let room = MAX_CHILDREN.saturating_sub(active.len());
        active.extend(rest[rest.len().saturating_sub(room)..].iter().copied());
        active.truncate(MAX_CHILDREN);
        active.sort_by(|a, b| (a.time.created, &a.id).cmp(&(b.time.created, &b.id)));
        kids = active;
    }
    std::iter::once(root_id).chain(kids.into_iter().map(|s| s.id.clone())).collect()
}

pub fn agent_name(store: &Store, session: &str, index: usize) -> String {
    store
        .messages(session)
        .iter()
        .rev()
        .find_map(|m| m.assistant().map(|a| a.agent.clone()))
        .filter(|a| !a.is_empty())
        .or_else(|| store.sessions.get(session).and_then(|s| s.agent.clone()))
        .unwrap_or_else(|| if index == 0 { "bake".into() } else { "agent".into() })
}

pub fn model_name(store: &Store, session: &str) -> Option<String> {
    store.messages(session).iter().rev().find_map(|m| m.assistant().map(|a| a.model_id.clone()))
}

fn error_message(error: &Value) -> String {
    error
        .get("data")
        .and_then(|d| d.get("message"))
        .and_then(Value::as_str)
        .or_else(|| error.get("name").and_then(Value::as_str))
        .unwrap_or("error")
        .to_string()
}

pub fn wait_of(store: &Store, session: &str) -> Wait {
    if let Some(p) = store.permissions.get(session).and_then(|l| l.first()) {
        return Wait::Permission(p.permission.clone());
    }
    if store.questions.get(session).is_some_and(|l| !l.is_empty()) {
        return Wait::Question;
    }
    let info = store.sessions.get(session);
    if info.is_some_and(|s| s.time.compacting.is_some()) {
        return Wait::Compacting;
    }
    let messages = store.messages(session);
    let last = messages.last();
    match store.status.get(session) {
        Some(SessionStatus::Retry { attempt, next, .. }) => return Wait::Retry { attempt: *attempt, next: *next },
        None | Some(SessionStatus::Idle) => {
            if let Some(Message::Assistant(a)) = last {
                if let Some(err) = &a.error {
                    if err.get("name").and_then(Value::as_str) == Some("MessageAbortedError") {
                        return Wait::Interrupted;
                    }
                    return Wait::Error(error_message(err));
                }
            }
            return if info.is_some_and(|s| s.parent_id.is_some()) { Wait::Done } else { Wait::Idle };
        }
        Some(SessionStatus::Busy) => {}
    }
    let Some(Message::Assistant(a)) = last else {
        return Wait::Model { since: last.map(|m| m.created()).unwrap_or(0) };
    };
    let parts: Vec<&Part> = store
        .parts(&a.id)
        .iter()
        .filter(|p| !matches!(p.kind, PartKind::StepStart {} | PartKind::StepFinish {}))
        .collect();
    let running = parts.iter().rev().find_map(|p| match &p.kind {
        PartKind::Tool { tool, state, .. } if state.is_active() => Some((tool, state)),
        _ => None,
    });
    if let Some((tool, state)) = running {
        let since = match state {
            ToolState::Running { time, .. } => time.start,
            _ => a.time.created,
        };
        return if tool == "task" { Wait::Subagent { since } } else { Wait::Tool { name: tool.clone(), since } };
    }
    match parts.last().map(|p| &p.kind) {
        Some(PartKind::Reasoning { .. }) => Wait::Thinking,
        Some(PartKind::Text { .. }) => Wait::Streaming,
        _ => {
            let since = parts
                .iter()
                .rev()
                .find_map(|p| match &p.kind {
                    PartKind::Tool { state: ToolState::Completed { time, .. }, .. }
                    | PartKind::Tool { state: ToolState::Error { time, .. }, .. } => time.end,
                    _ => None,
                })
                .unwrap_or(a.time.created);
            Wait::Model { since }
        }
    }
}

pub fn usage(store: &Store, session: &str) -> Usage {
    let sum = store.messages(session).iter().filter_map(Message::assistant).fold(Usage::default(), |acc, a| {
        acc.add(Usage {
            input: a.tokens.input,
            output: a.tokens.output + a.tokens.reasoning,
            cache_read: a.tokens.cache.read,
            cache_write: a.tokens.cache.write,
        })
    });
    // Session totals cover messages beyond the loaded window; take whichever is larger.
    match store.sessions.get(session).and_then(|s| s.tokens.as_ref()) {
        Some(t) => Usage {
            input: sum.input.max(t.input),
            output: sum.output.max(t.output + t.reasoning),
            cache_read: sum.cache_read.max(t.cache.read),
            cache_write: sum.cache_write.max(t.cache.write),
        },
        None => sum,
    }
}

/// `input` already excludes cached tokens on the server side.
pub fn cache_hit(u: &Usage) -> Option<u32> {
    let prompt = u.input + u.cache_read + u.cache_write;
    (prompt > 0.0).then(|| ((u.cache_read / prompt) * 100.0).round() as u32)
}

#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Eta {
    pub done: usize,
    pub total: usize,
    pub eta_ms: Option<i64>,
}

/// Average time per completed todo × remaining todos, from the todowrite history.
pub fn eta(store: &Store, root_id: &str, now: i64) -> Eta {
    let todos = store.todos.get(root_id).map(Vec::as_slice).unwrap_or(&[]);
    let remaining = todos.iter().filter(|t| t.status == "pending" || t.status == "in_progress").count();
    let done = todos.iter().filter(|t| t.status == "completed").count();
    let mut snaps: Vec<(i64, Vec<Todo>)> = store
        .messages(root_id)
        .iter()
        .flat_map(|m| store.parts(m.id()))
        .filter_map(|p| match &p.kind {
            PartKind::Tool { tool, state: ToolState::Completed { input, time, .. }, .. } if tool == "todowrite" => {
                let list: Vec<Todo> = serde_json::from_value(input.get("todos")?.clone()).ok()?;
                Some((time.end.unwrap_or(time.start), list))
            }
            _ => None,
        })
        .collect();
    snaps.sort_by_key(|(t, _)| *t);
    let base = Eta { done, total: done + remaining, eta_ms: None };
    let Some(start) = snaps.first().map(|(t, _)| *t) else { return base };
    let mut completed: Vec<(String, i64)> = Vec::new();
    for (time, list) in &snaps {
        for t in list.iter().filter(|t| t.status == "completed") {
            if !completed.iter().any(|(c, _)| c == &t.content) {
                completed.push((t.content.clone(), *time));
            }
        }
    }
    let last = completed.iter().map(|(_, t)| *t).max().unwrap_or(start);
    if completed.is_empty() || remaining == 0 || last <= start {
        return base;
    }
    let avg = (last - start) as f64 / completed.len() as f64;
    Eta { eta_ms: Some(((avg * remaining as f64) as i64 - (now - last)).max(0)), ..base }
}

pub fn bar(value: f64, max: f64, width: usize) -> String {
    let filled = if max > 0.0 { ((value / max) * width as f64).round().min(width as f64) as usize } else { 0 };
    "▓".repeat(filled) + &"░".repeat(width - filled)
}

pub fn tokens(n: f64) -> String {
    if n >= 1_000_000.0 {
        format!("{:.1}M", n / 1_000_000.0)
    } else if n >= 1000.0 {
        format!("{}k", (n / 1000.0).round())
    } else {
        format!("{n}")
    }
}

pub fn seconds(ms: i64) -> String {
    let s = (ms.max(0) as f64 / 1000.0).round() as i64;
    if s < 60 {
        return format!("{s}s");
    }
    let m = s / 60;
    if m < 60 {
        return if s % 60 > 0 { format!("{m}m{}s", s % 60) } else { format!("{m}m") };
    }
    format!("{}h{}m", m / 60, m % 60)
}

/// Long label (sidebar, status) using the AIOven words.
pub fn label(w: &Wait, now: i64) -> String {
    match w {
        Wait::Idle => "idle · waiting for you".into(),
        Wait::Done => "baked".into(),
        Wait::Error(m) => format!("burnt: {m}"),
        Wait::Interrupted => "pulled out".into(),
        Wait::Permission(p) => format!("needs permission: {p}"),
        Wait::Question => "waiting for your answer".into(),
        Wait::Compacting => "compacting context".into(),
        Wait::Retry { attempt, next } => format!("retry #{attempt} in {}", seconds(next - now)),
        Wait::Subagent { since } => format!("waiting on subagent {}", seconds(now - since)),
        Wait::Tool { name, since } => format!("tool:{name} {}", seconds(now - since)),
        Wait::Model { since } => format!("preheating {}", seconds(now - since)),
        Wait::Thinking => "thinking".into(),
        Wait::Streaming => "writing".into(),
    }
}

/// Short status for tab titles.
pub fn short(w: &Wait, now: i64) -> String {
    match w {
        Wait::Tool { name, since } => format!("⏳{name} {}", seconds(now - since)),
        Wait::Subagent { .. } => "⇢".into(),
        Wait::Model { .. } => "⏳".into(),
        Wait::Thinking | Wait::Streaming => "●".into(),
        Wait::Permission(_) => "? permission".into(),
        Wait::Question => "? question".into(),
        Wait::Retry { attempt, .. } => format!("↻{attempt}"),
        Wait::Compacting => "⇣".into(),
        Wait::Done => "✓".into(),
        Wait::Error(_) => "✗".into(),
        Wait::Interrupted => "■".into(),
        Wait::Idle => String::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::store::apply;

    fn store(events: &[&str]) -> Store {
        let mut s = Store::default();
        for e in events {
            apply(&mut s, Event::parse(e).expect("event"));
        }
        s
    }

    const ROOT: &str = r#"{"type":"session.updated","properties":{"sessionID":"r","info":{"id":"r","time":{"created":1,"updated":1}}}}"#;
    const KID: &str = r#"{"type":"session.updated","properties":{"sessionID":"k","info":{"id":"k","parentID":"r","time":{"created":2,"updated":2}}}}"#;
    const BUSY: &str = r#"{"type":"session.status","properties":{"sessionID":"r","status":{"type":"busy"}}}"#;
    const ASSISTANT: &str = r#"{"type":"message.updated","properties":{"sessionID":"r","info":{"id":"a","sessionID":"r","role":"assistant","time":{"created":5},"agent":"bake","modelID":"m","providerID":"p","tokens":{"input":100,"output":10,"reasoning":0,"cache":{"read":300,"write":0}}}}}"#;

    #[test]
    fn tab_order_and_root() {
        let s = store(&[ROOT, KID]);
        assert_eq!(root(&s, "k"), "r");
        assert_eq!(agents(&s, "k"), ["r", "k"]);
    }

    #[test]
    fn waits() {
        let s = store(&[ROOT, KID]);
        assert_eq!(wait_of(&s, "r"), Wait::Idle);
        assert_eq!(wait_of(&s, "k"), Wait::Done);
        let s = store(&[ROOT, BUSY, ASSISTANT,
            r#"{"type":"message.part.updated","properties":{"sessionID":"r","part":{"id":"p","messageID":"a","sessionID":"r","type":"tool","tool":"bash","callID":"c","state":{"status":"running","input":{},"time":{"start":7000}}}}}"#]);
        assert_eq!(wait_of(&s, "r"), Wait::Tool { name: "bash".into(), since: 7000 });
        assert_eq!(label(&wait_of(&s, "r"), 19_000), "tool:bash 12s");
        let s = store(&[ROOT, BUSY, ASSISTANT]);
        assert_eq!(label(&wait_of(&s, "r"), 3005), "preheating 3s");
    }

    #[test]
    fn usage_and_cache() {
        let s = store(&[ROOT, ASSISTANT]);
        let u = usage(&s, "r");
        assert_eq!(u.total(), 410.0);
        assert_eq!(cache_hit(&u), Some(75));
    }

    #[test]
    fn eta_from_todowrite_history() {
        let snap = |id: &str, end: i64, todos: &str| format!(
            r#"{{"type":"message.part.updated","properties":{{"sessionID":"r","part":{{"id":"{id}","messageID":"a","sessionID":"r","type":"tool","tool":"todowrite","callID":"c","state":{{"status":"completed","input":{{"todos":{todos}}},"title":"","metadata":{{}},"time":{{"start":{end},"end":{end}}}}}}}}}}}"#);
        let s1 = snap("p1", 0, r#"[{"content":"x","status":"in_progress"},{"content":"y","status":"pending"},{"content":"z","status":"pending"}]"#);
        let s2 = snap("p2", 60_000, r#"[{"content":"x","status":"completed"},{"content":"y","status":"in_progress"},{"content":"z","status":"pending"}]"#);
        let todos = r#"{"type":"todo.updated","properties":{"sessionID":"r","todos":[{"content":"x","status":"completed"},{"content":"y","status":"in_progress"},{"content":"z","status":"pending"}]}}"#;
        let s = store(&[ROOT, ASSISTANT, &s1, &s2, todos]);
        assert_eq!(eta(&s, "r", 60_000), Eta { done: 1, total: 3, eta_ms: Some(120_000) });
    }

    #[test]
    fn formatting() {
        assert_eq!(bar(50.0, 100.0, 8), "▓▓▓▓░░░░");
        assert_eq!(tokens(3_200_000.0), "3.2M");
        assert_eq!(tokens(62_400.0), "62k");
        assert_eq!(seconds(125_000), "2m5s");
        assert_eq!(label(&Wait::Done, 0), "baked");
        assert_eq!(label(&Wait::Interrupted, 0), "pulled out");
    }
}
