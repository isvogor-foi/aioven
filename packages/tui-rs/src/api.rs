//! R2 Api: typed HTTP calls to the AIOven server (every call is scoped by ?directory=).

use std::collections::HashMap;

use anyhow::{Context, Result};
use serde::de::DeserializeOwned;
use serde_json::{Value, json};
use url::Url;

use crate::types::*;

#[derive(Clone)]
pub struct Api {
    http: reqwest::Client,
    base: Url,
    directory: String,
}

impl Api {
    pub fn new(base: Url, directory: String) -> Api {
        Api { http: reqwest::Client::new(), base, directory }
    }

    pub fn url(&self, path: &str) -> Url {
        let mut url = self.base.join(path).expect("valid api path");
        url.query_pairs_mut().append_pair("directory", &self.directory);
        url
    }

    async fn get<T: DeserializeOwned>(&self, path: &str) -> Result<T> {
        let res = self.http.get(self.url(path)).send().await.with_context(|| format!("GET {path}"))?;
        let res = res.error_for_status().with_context(|| format!("GET {path}"))?;
        Ok(res.json().await.with_context(|| format!("decode {path}"))?)
    }

    async fn post(&self, path: &str, body: Value) -> Result<Value> {
        let res = self.http.post(self.url(path)).json(&body).send().await.with_context(|| format!("POST {path}"))?;
        let res = res.error_for_status().with_context(|| format!("POST {path}"))?;
        let text = res.text().await?;
        Ok(if text.trim().is_empty() { Value::Null } else { serde_json::from_str(&text).unwrap_or(Value::Null) })
    }

    pub async fn agents(&self) -> Result<Vec<Agent>> {
        self.get("/agent").await
    }
    pub async fn skills(&self) -> Result<Vec<Skill>> {
        self.get("/skill").await
    }
    pub async fn config(&self) -> Result<Value> {
        self.get("/config").await
    }
    pub async fn session_create(&self) -> Result<Session> {
        Ok(serde_json::from_value(self.post("/session", json!({})).await?)?)
    }
    pub async fn session(&self, id: &str) -> Result<Session> {
        self.get(&format!("/session/{id}")).await
    }
    pub async fn children(&self, id: &str) -> Result<Vec<Session>> {
        self.get(&format!("/session/{id}/children")).await
    }
    pub async fn messages(&self, id: &str, limit: u32) -> Result<Vec<MessageWithParts>> {
        self.get(&format!("/session/{id}/message?limit={limit}")).await
    }
    pub async fn todos(&self, id: &str) -> Result<Vec<Todo>> {
        self.get(&format!("/session/{id}/todo")).await
    }
    pub async fn diff(&self, id: &str) -> Result<Vec<FileDiff>> {
        self.get(&format!("/session/{id}/diff")).await
    }
    pub async fn status(&self) -> Result<HashMap<String, SessionStatus>> {
        self.get("/session/status").await
    }
    pub async fn prompt(&self, id: &str, text: &str, agent: &str, model: Option<(String, String)>) -> Result<()> {
        let mut body = json!({ "agent": agent, "parts": [{ "type": "text", "text": text }] });
        if let Some((provider, model)) = model {
            body["model"] = json!({ "providerID": provider, "modelID": model });
        }
        self.post(&format!("/session/{id}/prompt_async"), body).await.map(|_| ())
    }
    /// Move running foreground subagents of a session to the background.
    pub async fn background(&self, id: &str) -> Result<()> {
        self.post(&format!("/experimental/session/{id}/background"), json!({})).await.map(|_| ())
    }
    pub async fn commands(&self) -> Result<Vec<Command>> {
        self.get("/command").await
    }
    pub async fn command(&self, id: &str, name: &str, arguments: &str, agent: &str) -> Result<()> {
        let body = json!({ "command": name, "arguments": arguments, "agent": agent });
        self.post(&format!("/session/{id}/command"), body).await.map(|_| ())
    }
    /// Models of connected providers as (provider id, model id, label).
    pub async fn models(&self) -> Result<Vec<(String, String, String)>> {
        let list: ProviderList = self.get("/provider").await?;
        Ok(list
            .all
            .into_iter()
            .filter(|p| list.connected.contains(&p.id))
            .flat_map(|p| {
                let pname = p.name.clone().unwrap_or_else(|| p.id.clone());
                p.models.into_iter().map(move |(id, m)| {
                    let label = format!("{} · {}", m.name.unwrap_or_else(|| id.clone()), pname);
                    (p.id.clone(), id, label)
                })
            })
            .collect())
    }
    pub async fn abort(&self, id: &str) -> Result<()> {
        self.post(&format!("/session/{id}/abort"), json!({})).await.map(|_| ())
    }
    pub async fn permission_reply(&self, request: &str, reply: Reply) -> Result<()> {
        self.post(&format!("/permission/{request}/reply"), json!({ "reply": reply })).await.map(|_| ())
    }
    pub async fn question_reply(&self, request: &str, answers: Vec<Vec<String>>) -> Result<()> {
        self.post(&format!("/question/{request}/reply"), json!({ "answers": answers })).await.map(|_| ())
    }
    pub async fn question_reject(&self, request: &str) -> Result<()> {
        self.post(&format!("/question/{request}/reject"), json!({})).await.map(|_| ())
    }
    /// Text content of a project file (e.g. the recipe), or None when missing/binary.
    pub async fn read_file(&self, path: &str) -> Result<Option<String>> {
        let mut url = self.url("/file/content");
        url.query_pairs_mut().append_pair("path", path);
        let res = self.http.get(url).send().await?;
        if !res.status().is_success() {
            return Ok(None);
        }
        let v: Value = res.json().await?;
        Ok((v.get("type").and_then(Value::as_str) == Some("text"))
            .then(|| v.get("content").and_then(Value::as_str).unwrap_or("").to_string()))
    }
}
