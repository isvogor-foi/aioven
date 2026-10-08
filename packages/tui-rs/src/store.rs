//! R4 Store: client state, updated only by the pure reducer `apply`.

use std::collections::HashMap;

use serde_json::Value;

use crate::types::*;

#[derive(Debug, Default, Clone)]
pub struct Store {
    pub sessions: HashMap<String, Session>,
    /// Messages per session, kept sorted by (created, id).
    pub messages: HashMap<String, Vec<Message>>,
    /// Parts per message id, in arrival order.
    pub parts: HashMap<String, Vec<Part>>,
    pub status: HashMap<String, SessionStatus>,
    pub todos: HashMap<String, Vec<Todo>>,
    pub diffs: HashMap<String, Vec<FileDiff>>,
    pub permissions: HashMap<String, Vec<PermissionRequest>>,
    pub questions: HashMap<String, Vec<QuestionRequest>>,
    pub agents: Vec<Agent>,
    pub skills: Vec<Skill>,
    pub config: Value,
}

impl Store {
    pub fn messages(&self, session: &str) -> &[Message] {
        self.messages.get(session).map(Vec::as_slice).unwrap_or(&[])
    }
    pub fn parts(&self, message: &str) -> &[Part] {
        self.parts.get(message).map(Vec::as_slice).unwrap_or(&[])
    }

    pub fn upsert_message(&mut self, message: Message) {
        let list = self.messages.entry(message.session_id().to_string()).or_default();
        match list.iter().position(|m| m.id() == message.id()) {
            Some(i) => list[i] = message,
            None => {
                list.push(message);
                list.sort_by(|a, b| (a.created(), a.id()).cmp(&(b.created(), b.id())));
            }
        }
    }

    pub fn upsert_part(&mut self, part: Part) {
        let list = self.parts.entry(part.message_id.clone()).or_default();
        match list.iter().position(|p| p.id == part.id) {
            Some(i) => list[i] = part,
            None => list.push(part),
        }
    }

    /// Load a session's history (replaces what is known for that session).
    pub fn load_history(&mut self, session: &str, history: Vec<MessageWithParts>) {
        self.messages.insert(session.to_string(), Vec::new());
        for item in history {
            let id = item.info.id().to_string();
            self.upsert_message(item.info);
            self.parts.insert(id, item.parts);
        }
    }
}

pub fn apply(store: &mut Store, event: Event) {
    match event {
        Event::SessionUpdated(session) => {
            store.sessions.insert(session.id.clone(), session);
        }
        Event::SessionDeleted(id) => {
            store.sessions.remove(&id);
        }
        Event::SessionStatus { session_id, status } => {
            store.status.insert(session_id, status);
        }
        Event::MessageUpdated(message) => store.upsert_message(message),
        Event::PartUpdated(part) => store.upsert_part(part),
        Event::PartDelta { message_id, part_id, field, delta } => {
            if field != "text" {
                return;
            }
            if let Some(part) = store.parts.get_mut(&message_id).and_then(|l| l.iter_mut().find(|p| p.id == part_id)) {
                match &mut part.kind {
                    PartKind::Text { text } | PartKind::Reasoning { text } => text.push_str(&delta),
                    _ => {}
                }
            }
        }
        Event::PartRemoved { message_id, part_id } => {
            if let Some(list) = store.parts.get_mut(&message_id) {
                list.retain(|p| p.id != part_id);
            }
        }
        Event::TodoUpdated { session_id, todos } => {
            store.todos.insert(session_id, todos);
        }
        Event::SessionDiff { session_id, diff } => {
            store.diffs.insert(session_id, diff);
        }
        Event::PermissionAsked(request) => {
            let list = store.permissions.entry(request.session_id.clone()).or_default();
            list.retain(|r| r.id != request.id);
            list.push(request);
        }
        Event::PermissionReplied { session_id, request_id } => {
            if let Some(list) = store.permissions.get_mut(&session_id) {
                list.retain(|r| r.id != request_id);
            }
        }
        Event::QuestionAsked(request) => {
            let list = store.questions.entry(request.session_id.clone()).or_default();
            list.retain(|r| r.id != request.id);
            list.push(request);
        }
        Event::QuestionDone { session_id, request_id } => {
            if let Some(list) = store.questions.get_mut(&session_id) {
                list.retain(|r| r.id != request_id);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ev(json: &str) -> Event {
        Event::parse(json).expect("event parses")
    }

    #[test]
    fn messages_parts_and_deltas() {
        let mut s = Store::default();
        apply(&mut s, ev(r#"{"type":"message.updated","properties":{"sessionID":"s","info":{"id":"m2","sessionID":"s","role":"assistant","time":{"created":2},"agent":"bake","modelID":"x","providerID":"p","tokens":{"input":1,"output":2,"reasoning":0,"cache":{"read":0,"write":0}}}}}"#));
        apply(&mut s, ev(r#"{"type":"message.updated","properties":{"sessionID":"s","info":{"id":"m1","sessionID":"s","role":"user","time":{"created":1},"agent":"bake"}}}"#));
        assert_eq!(s.messages("s").iter().map(|m| m.id()).collect::<Vec<_>>(), ["m1", "m2"]);

        apply(&mut s, ev(r#"{"type":"message.part.updated","properties":{"sessionID":"s","part":{"id":"p1","messageID":"m2","sessionID":"s","type":"text","text":"he"}}}"#));
        apply(&mut s, ev(r#"{"type":"message.part.delta","properties":{"sessionID":"s","messageID":"m2","partID":"p1","field":"text","delta":"llo"}}"#));
        assert_eq!(s.parts("m2")[0].kind, PartKind::Text { text: "hello".into() });

        apply(&mut s, ev(r#"{"type":"message.part.removed","properties":{"sessionID":"s","messageID":"m2","partID":"p1"}}"#));
        assert!(s.parts("m2").is_empty());
    }

    #[test]
    fn permissions_come_and_go() {
        let mut s = Store::default();
        apply(&mut s, ev(r#"{"type":"permission.asked","properties":{"id":"r1","sessionID":"s","permission":"bash","patterns":["ls"],"metadata":{},"always":[]}}"#));
        assert_eq!(s.permissions["s"].len(), 1);
        apply(&mut s, ev(r#"{"type":"permission.replied","properties":{"sessionID":"s","requestID":"r1","reply":"once"}}"#));
        assert!(s.permissions["s"].is_empty());
    }

    #[test]
    fn session_parent_id_is_read() {
        let mut s = Store::default();
        apply(&mut s, ev(r#"{"type":"session.updated","properties":{"sessionID":"k","info":{"id":"k","parentID":"r","time":{"created":1,"updated":1}}}}"#));
        assert_eq!(s.sessions["k"].parent_id.as_deref(), Some("r"));
    }

    #[test]
    fn tool_parts_and_unknown_events() {
        let mut s = Store::default();
        apply(&mut s, ev(r#"{"type":"message.part.updated","properties":{"sessionID":"s","part":{"id":"p","messageID":"m","sessionID":"s","type":"tool","tool":"bash","callID":"c","state":{"status":"running","input":{"command":"ls"},"time":{"start":5}}}}}"#));
        assert!(matches!(&s.parts("m")[0].kind, PartKind::Tool { tool, state, .. } if tool == "bash" && state.is_active()));
        assert!(Event::parse(r#"{"type":"server.connected","properties":{}}"#).is_none());
    }
}
