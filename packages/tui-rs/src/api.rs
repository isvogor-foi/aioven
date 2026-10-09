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

    /// P3: project files matching `query` (fuzzy, server side).
    pub async fn find_files(&self, query: &str) -> Result<Vec<String>> {
        let mut url = self.url("/find/file");
        url.query_pairs_mut().append_pair("query", query).append_pair("limit", "12");
        let res = self.http.get(url).send().await.context("GET /find/file")?.error_for_status()?;
        Ok(res.json().await?)
    }
    pub fn directory(&self) -> &str {
        &self.directory
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
    /// Root sessions of the project, newest first.
    pub async fn latest_session(&self) -> Result<Option<Session>> {
        let mut list: Vec<Session> = self.get("/session?roots=true").await?;
        list.retain(|s| s.parent_id.is_none());
        list.sort_by_key(|s| std::cmp::Reverse(s.time.updated));
        Ok(list.into_iter().next())
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
    /// Whole-session diff (AIOven endpoint; upstream /session/{id}/diff only diffs one turn).
    pub async fn diff(&self, id: &str) -> Result<Vec<FileDiff>> {
        self.get(&format!("/experimental/aioven/session/{id}/diff")).await
    }
    pub async fn status(&self) -> Result<HashMap<String, SessionStatus>> {
        self.get("/session/status").await
    }
    /// `files`: P3 attachments (file parts); `variant`: P6 reasoning variant of the model.
    pub async fn prompt(
        &self,
        id: &str,
        text: &str,
        agent: &str,
        model: Option<(String, String)>,
        variant: Option<String>,
        files: Vec<Value>,
    ) -> Result<()> {
        let mut parts = vec![json!({ "type": "text", "text": text })];
        parts.extend(files);
        let mut body = json!({ "agent": agent, "parts": parts });
        if let Some((provider, model)) = model {
            body["model"] = json!({ "providerID": provider, "modelID": model });
        }
        if let Some(variant) = variant {
            body["variant"] = json!(variant);
        }
        self.post(&format!("/session/{id}/prompt_async"), body).await.map(|_| ())
    }
    /// P4: run a shell command in the session (output becomes part of the transcript).
    pub async fn shell(&self, id: &str, agent: &str, command: &str, model: Option<(String, String)>) -> Result<()> {
        let mut body = json!({ "agent": agent, "command": command });
        if let Some((provider, model)) = model {
            body["model"] = json!({ "providerID": provider, "modelID": model });
        }
        self.post(&format!("/session/{id}/shell"), body).await.map(|_| ())
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
    /// T28: AIOven agents with size and resolved model.
    pub async fn agent_models(&self) -> Result<Vec<AgentModel>> {
        self.get("/experimental/aioven/agents").await
    }
    /// P6: reasoning variants per (provider, model) of connected providers.
    pub async fn variants(&self) -> Result<HashMap<(String, String), Vec<String>>> {
        let list: ProviderList = self.get("/provider").await?;
        Ok(list
            .all
            .into_iter()
            .filter(|p| list.connected.contains(&p.id))
            .flat_map(|p| {
                let pid = p.id.clone();
                p.models.into_iter().map(move |(id, m)| ((pid.clone(), id), m.variants.into_keys().collect()))
            })
            .collect())
    }
    /// P5 session actions
    pub async fn rename(&self, id: &str, title: &str) -> Result<()> {
        let res = self.http.patch(self.url(&format!("/session/{id}"))).json(&json!({ "title": title })).send().await?;
        res.error_for_status().context("rename session")?;
        Ok(())
    }
    pub async fn delete(&self, id: &str) -> Result<()> {
        let res = self.http.delete(self.url(&format!("/session/{id}"))).send().await?;
        res.error_for_status().context("delete session")?;
        Ok(())
    }
    pub async fn revert(&self, id: &str, message_id: &str) -> Result<()> {
        self.post(&format!("/session/{id}/revert"), json!({ "messageID": message_id })).await.map(|_| ())
    }
    pub async fn unrevert(&self, id: &str) -> Result<()> {
        self.post(&format!("/session/{id}/unrevert"), json!({})).await.map(|_| ())
    }
    /// Permission requests pending before the client connected.
    pub async fn permissions(&self) -> Result<Vec<PermissionRequest>> {
        self.get("/permission").await
    }
    /// Question requests pending before the client connected.
    pub async fn questions(&self) -> Result<Vec<QuestionRequest>> {
        self.get("/question").await
    }
    // ---- T19 connect -------------------------------------------------------------------------------
    pub async fn provider_choices(&self) -> Result<Vec<crate::connect::ProviderChoice>> {
        let list: ProviderList = self.get("/provider").await?;
        Ok(list
            .all
            .into_iter()
            .map(|p| crate::connect::ProviderChoice {
                connected: list.connected.contains(&p.id),
                name: p.name.clone().unwrap_or_else(|| p.id.clone()),
                id: p.id,
            })
            .collect())
    }
    pub async fn auth_methods(&self) -> Result<HashMap<String, Vec<crate::connect::Method>>> {
        let raw: HashMap<String, Vec<Value>> = self.get("/provider/auth").await?;
        Ok(raw
            .into_iter()
            .map(|(id, list)| {
                let methods = list
                    .iter()
                    .map(|m| crate::connect::Method {
                        oauth: m.get("type").and_then(Value::as_str) == Some("oauth"),
                        label: m.get("label").and_then(Value::as_str).unwrap_or("").to_string(),
                    })
                    .collect();
                (id, methods)
            })
            .collect())
    }
    pub async fn set_api_key(&self, provider: &str, key: &str) -> Result<()> {
        let res = self.http.put(self.url(&format!("/auth/{provider}"))).json(&json!({ "type": "api", "key": key })).send().await?;
        res.error_for_status()?;
        Ok(())
    }
    /// → (url, auto, instructions)
    pub async fn oauth_authorize(&self, provider: &str, method: usize) -> Result<(String, bool, String)> {
        let v = self.post(&format!("/provider/{provider}/oauth/authorize"), json!({ "method": method })).await?;
        let s = |k: &str| v.get(k).and_then(Value::as_str).unwrap_or("").to_string();
        Ok((s("url"), s("method") == "auto", s("instructions")))
    }
    pub async fn oauth_callback(&self, provider: &str, method: usize, code: Option<String>) -> Result<()> {
        let mut body = json!({ "method": method });
        if let Some(code) = code {
            body["code"] = json!(code);
        }
        self.post(&format!("/provider/{provider}/oauth/callback"), body).await.map(|_| ())
    }
    /// T22: token usage per day since installation.
    pub async fn usage(&self) -> Result<crate::usage::Usage> {
        self.get("/experimental/aioven/usage").await
    }
    /// T23: root sessions of the project, newest first.
    pub async fn sessions(&self) -> Result<Vec<Session>> {
        let mut list: Vec<Session> = self.get("/session?roots=true&limit=200").await?;
        list.retain(|s| s.parent_id.is_none());
        list.sort_by_key(|s| std::cmp::Reverse(s.time.updated));
        Ok(list)
    }
    /// T24/T25: deep-merge a patch into the user's global config.
    pub async fn patch_global(&self, patch: Value) -> Result<()> {
        let res = self.http.patch(self.url("/global/config")).json(&patch).send().await?;
        res.error_for_status()?;
        Ok(())
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
