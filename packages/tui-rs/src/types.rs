//! Wire types for the subset of the AIOven server API this client uses.
//! Field names follow the server JSON (camelCase); unknown fields are ignored.

use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, Default, Deserialize, Serialize, PartialEq)]
pub struct Tokens {
    #[serde(default)]
    pub input: f64,
    #[serde(default)]
    pub output: f64,
    #[serde(default)]
    pub reasoning: f64,
    #[serde(default)]
    pub cache: Cache,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize, PartialEq)]
pub struct Cache {
    #[serde(default)]
    pub read: f64,
    #[serde(default)]
    pub write: f64,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize, PartialEq)]
pub struct SessionTime {
    #[serde(default)]
    pub created: i64,
    #[serde(default)]
    pub updated: i64,
    #[serde(default)]
    pub compacting: Option<i64>,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Session {
    pub id: String,
    #[serde(default, rename = "parentID")]
    pub parent_id: Option<String>,
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub slug: String,
    #[serde(default)]
    pub directory: String,
    #[serde(default)]
    pub agent: Option<String>,
    #[serde(default)]
    pub tokens: Option<Tokens>,
    #[serde(default)]
    pub time: SessionTime,
    /// P5: set after "undo": messages from this one on are hidden until the next prompt
    #[serde(default)]
    pub revert: Option<SessionRevert>,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize, PartialEq)]
pub struct SessionRevert {
    #[serde(rename = "messageID")]
    pub message_id: String,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize, PartialEq)]
pub struct MessageTime {
    #[serde(default)]
    pub created: i64,
    #[serde(default)]
    pub completed: Option<i64>,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
#[serde(tag = "role", rename_all = "lowercase")]
pub enum Message {
    User(UserMessage),
    Assistant(AssistantMessage),
}

#[derive(Debug, Clone, Default, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct UserMessage {
    pub id: String,
    #[serde(rename = "sessionID")]
    pub session_id: String,
    #[serde(default)]
    pub time: MessageTime,
    #[serde(default)]
    pub agent: String,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize, PartialEq)]
pub struct AssistantMessage {
    pub id: String,
    #[serde(rename = "sessionID")]
    pub session_id: String,
    #[serde(default)]
    pub time: MessageTime,
    #[serde(default)]
    pub error: Option<Value>,
    #[serde(default)]
    pub agent: String,
    #[serde(default, rename = "modelID")]
    pub model_id: String,
    #[serde(default, rename = "providerID")]
    pub provider_id: String,
    #[serde(default)]
    pub tokens: Tokens,
    #[serde(default)]
    pub finish: Option<String>,
}

impl Message {
    pub fn id(&self) -> &str {
        match self {
            Message::User(m) => &m.id,
            Message::Assistant(m) => &m.id,
        }
    }
    pub fn session_id(&self) -> &str {
        match self {
            Message::User(m) => &m.session_id,
            Message::Assistant(m) => &m.session_id,
        }
    }
    pub fn created(&self) -> i64 {
        match self {
            Message::User(m) => m.time.created,
            Message::Assistant(m) => m.time.created,
        }
    }
    pub fn assistant(&self) -> Option<&AssistantMessage> {
        match self {
            Message::Assistant(m) => Some(m),
            Message::User(_) => None,
        }
    }
}

#[derive(Debug, Clone, Default, Deserialize, Serialize, PartialEq)]
pub struct ToolTime {
    #[serde(default)]
    pub start: i64,
    #[serde(default)]
    pub end: Option<i64>,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
#[serde(tag = "status", rename_all = "lowercase")]
pub enum ToolState {
    Pending {
        #[serde(default)]
        input: Value,
    },
    Running {
        #[serde(default)]
        input: Value,
        #[serde(default)]
        title: Option<String>,
        #[serde(default)]
        metadata: Value,
        #[serde(default)]
        time: ToolTime,
    },
    Completed {
        #[serde(default)]
        input: Value,
        #[serde(default)]
        title: String,
        #[serde(default)]
        metadata: Value,
        #[serde(default)]
        time: ToolTime,
    },
    Error {
        #[serde(default)]
        input: Value,
        #[serde(default)]
        error: String,
        #[serde(default)]
        time: ToolTime,
    },
}

impl ToolState {
    pub fn input(&self) -> &Value {
        match self {
            ToolState::Pending { input }
            | ToolState::Running { input, .. }
            | ToolState::Completed { input, .. }
            | ToolState::Error { input, .. } => input,
        }
    }
    pub fn metadata(&self) -> Option<&Value> {
        match self {
            ToolState::Running { metadata, .. } | ToolState::Completed { metadata, .. } => Some(metadata),
            _ => None,
        }
    }
    pub fn is_active(&self) -> bool {
        matches!(self, ToolState::Pending { .. } | ToolState::Running { .. })
    }
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
#[serde(tag = "type", rename_all = "kebab-case")]
pub enum PartKind {
    Text {
        #[serde(default)]
        text: String,
    },
    Reasoning {
        #[serde(default)]
        text: String,
    },
    Tool {
        tool: String,
        #[serde(default, rename = "callID")]
        call_id: String,
        state: ToolState,
    },
    StepStart {},
    StepFinish {},
    /// files changed by the step (P7: triggers a diff refresh)
    Patch {},
    #[serde(other)]
    Other,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
pub struct Part {
    pub id: String,
    #[serde(rename = "messageID")]
    pub message_id: String,
    #[serde(rename = "sessionID")]
    pub session_id: String,
    #[serde(flatten)]
    pub kind: PartKind,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
pub struct MessageWithParts {
    pub info: Message,
    #[serde(default)]
    pub parts: Vec<Part>,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize, PartialEq)]
pub struct Todo {
    #[serde(default)]
    pub content: String,
    #[serde(default)]
    pub status: String,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize, PartialEq)]
pub struct FileDiff {
    #[serde(default)]
    pub file: Option<String>,
    #[serde(default)]
    pub additions: i64,
    #[serde(default)]
    pub deletions: i64,
    #[serde(default)]
    pub status: Option<String>,
    /// P7: unified diff of the file
    #[serde(default)]
    pub patch: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum SessionStatus {
    Idle,
    Busy,
    Retry {
        #[serde(default)]
        attempt: i64,
        #[serde(default)]
        message: String,
        #[serde(default)]
        next: i64,
    },
}

#[derive(Debug, Clone, Default, Deserialize, Serialize, PartialEq)]
pub struct PermissionRequest {
    pub id: String,
    #[serde(rename = "sessionID")]
    pub session_id: String,
    #[serde(default)]
    pub permission: String,
    #[serde(default)]
    pub patterns: Vec<String>,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize, PartialEq)]
pub struct QuestionOption {
    #[serde(default)]
    pub label: String,
    #[serde(default)]
    pub description: String,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize, PartialEq)]
pub struct QuestionInfo {
    #[serde(default)]
    pub question: String,
    #[serde(default)]
    pub header: String,
    #[serde(default)]
    pub options: Vec<QuestionOption>,
    #[serde(default)]
    pub multiple: Option<bool>,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize, PartialEq)]
pub struct QuestionRequest {
    pub id: String,
    #[serde(rename = "sessionID")]
    pub session_id: String,
    #[serde(default)]
    pub questions: Vec<QuestionInfo>,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize, PartialEq)]
pub struct Agent {
    pub name: String,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub mode: String,
    #[serde(default)]
    pub hidden: Option<bool>,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize, PartialEq)]
pub struct Skill {
    pub name: String,
    #[serde(default)]
    pub description: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Reply {
    Once,
    Always,
    Reject,
}

/// Server-sent event: `data: {"id","type","properties"}`.
#[derive(Debug, Clone, PartialEq)]
pub enum Event {
    SessionUpdated(Session),
    SessionDeleted(String),
    SessionStatus { session_id: String, status: SessionStatus },
    MessageUpdated(Message),
    PartUpdated(Part),
    PartDelta { message_id: String, part_id: String, field: String, delta: String },
    PartRemoved { message_id: String, part_id: String },
    TodoUpdated { session_id: String, todos: Vec<Todo> },
    SessionDiff { session_id: String, diff: Vec<FileDiff> },
    PermissionAsked(PermissionRequest),
    PermissionReplied { session_id: String, request_id: String },
    QuestionAsked(QuestionRequest),
    QuestionDone { session_id: String, request_id: String },
}

impl Event {
    /// Parse one SSE data payload; unknown or malformed events give `None`.
    pub fn parse(data: &str) -> Option<Event> {
        let v: Value = serde_json::from_str(data).ok()?;
        let p = v.get("properties")?;
        let s = |k: &str| p.get(k).and_then(Value::as_str).map(str::to_string);
        let de = |k: &str| p.get(k).cloned();
        Some(match v.get("type")?.as_str()? {
            "session.updated" | "session.created" => Event::SessionUpdated(serde_json::from_value(de("info")?).ok()?),
            "session.deleted" => Event::SessionDeleted(s("sessionID")?),
            "session.status" => Event::SessionStatus {
                session_id: s("sessionID")?,
                status: serde_json::from_value(de("status")?).ok()?,
            },
            "message.updated" => Event::MessageUpdated(serde_json::from_value(de("info")?).ok()?),
            "message.part.updated" => Event::PartUpdated(serde_json::from_value(de("part")?).ok()?),
            "message.part.delta" => Event::PartDelta {
                message_id: s("messageID")?,
                part_id: s("partID")?,
                field: s("field")?,
                delta: s("delta")?,
            },
            "message.part.removed" => Event::PartRemoved { message_id: s("messageID")?, part_id: s("partID")? },
            "todo.updated" => Event::TodoUpdated {
                session_id: s("sessionID")?,
                todos: serde_json::from_value(de("todos")?).ok()?,
            },
            "session.diff" => Event::SessionDiff {
                session_id: s("sessionID")?,
                diff: serde_json::from_value(de("diff")?).ok()?,
            },
            "permission.asked" => Event::PermissionAsked(serde_json::from_value(p.clone()).ok()?),
            "permission.replied" => Event::PermissionReplied { session_id: s("sessionID")?, request_id: s("requestID")? },
            "question.asked" => Event::QuestionAsked(serde_json::from_value(p.clone()).ok()?),
            "question.replied" | "question.rejected" => {
                Event::QuestionDone { session_id: s("sessionID")?, request_id: s("requestID")? }
            }
            _ => return None,
        })
    }
}

/// GET /command — slash commands; skills have `source: "skill"`.
#[derive(Debug, Clone, Default, Deserialize, Serialize, PartialEq)]
pub struct Command {
    pub name: String,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub source: Option<String>,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize, PartialEq)]
pub struct ModelInfo {
    #[serde(default)]
    pub id: Option<String>,
    #[serde(default)]
    pub name: Option<String>,
    /// P6: reasoning variants (e.g. low/medium/high) by name
    #[serde(default)]
    pub variants: std::collections::BTreeMap<String, Value>,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize, PartialEq)]
pub struct Provider {
    pub id: String,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub models: std::collections::BTreeMap<String, ModelInfo>,
}

/// GET /provider
#[derive(Debug, Clone, Default, Deserialize, Serialize, PartialEq)]
pub struct ProviderList {
    #[serde(default)]
    pub all: Vec<Provider>,
    #[serde(default)]
    pub connected: Vec<String>,
}
