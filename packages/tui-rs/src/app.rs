//! R8 App: owns Store + UI state; `update(Action)` changes state and starts network work.
//! Network results come back as `NetMsg` so rendering never blocks.

use std::collections::HashSet;

use tokio::sync::mpsc::UnboundedSender;
use tui_textarea::TextArea;

use crate::api::Api;
use crate::derive::{self, Wait};
use crate::complete::{self, Entry};
use crate::keys::{Action, Focus, KeyContext, Modal, Pane, Popup};
use crate::connect::{self, Effect, Step};
use crate::menu::{self, Item};
use crate::store::{self, Store};
use crate::types::*;

#[derive(Debug, Clone, PartialEq)]
pub enum View {
    Chat,
    Blueprint,
    Skill(String),
    Usage,
    /// P7: patch of one changed file
    Diff(String),
}

#[derive(Debug, Clone, PartialEq)]
pub enum Dialog {
    StopOne(String),
    StopAll(Vec<String>),
    /// P5: delete the root session
    Delete(String),
}

pub enum NetMsg {
    History { session: String, items: Vec<MessageWithParts> },
    Session(Session),
    Todos { session: String, todos: Vec<Todo> },
    Diff { session: String, diff: Vec<FileDiff> },
    Recipe(Option<String>),
    Commands(Vec<Entry>),
    Models(Vec<(String, String, String)>),
    Notice(String),
    Pending { permissions: Vec<PermissionRequest>, questions: Vec<QuestionRequest> },
    ConnectData { providers: Vec<connect::ProviderChoice>, methods: std::collections::HashMap<String, Vec<connect::Method>> },
    ConnectStep(Step),
    Connected(String),
    Usage(crate::usage::Usage),
    Sessions(Vec<Session>),
    /// P3: file search results for an "@query"
    Files { query: String, list: Vec<String> },
    Variants(std::collections::HashMap<(String, String), Vec<String>>),
    AgentModels(Vec<crate::types::AgentModel>),
    /// P8: the pending root session was created
    Created(Session),
    Config(serde_json::Value),
    Error(String),
}

pub struct App {
    pub store: Store,
    pub api: Api,
    /// P8: "" = pending; the session is created on the first prompt/command/shell (see `with_root`)
    pub root: String,
    pub viewing: String,
    pub view: View,
    pub compact: bool,
    pub chat_scroll: u16,
    pub files_scroll: u16,
    pub dialog: Option<Dialog>,
    pub agent: String,
    pub quit_armed: bool,
    pub quit: bool,
    pub notice: Option<String>,
    pub question_sel: usize,
    pub input: TextArea<'static>,
    pub recipe: Option<String>,
    pub now: i64,
    pub focus: Focus,
    pub menu_open: bool,
    /// T23 sessions popup: (query, selection); list loaded once per open
    pub sessions_popup: Option<(String, usize)>,
    pub session_list: Vec<Session>,
    /// P3: (query, matches) of the last "@file" search
    pub file_matches: (String, Vec<String>),
    /// P6: reasoning variant sent with prompts (None = model default)
    pub variant: Option<String>,
    /// P4: set by Ctrl+E; the main loop suspends the TUI and opens $EDITOR with this text
    pub edit_request: Option<String>,
    /// P5: rename popup text
    pub rename: Option<String>,
    /// P7: index of the selected changed file in the blueprint (Files focus, [ / ])
    pub file_sel: usize,
    /// T28: AIOven agents with their size and model (right bar, settings)
    pub agent_models: Vec<crate::types::AgentModel>,
    /// T29 popup: None = closed; agent None = agent list, Some(i) = choices for agent i
    pub agents_popup: Option<AgentsPopup>,
    /// P6: reasoning variants per (provider, model)
    pub variants: std::collections::HashMap<(String, String), Vec<String>>,
    pub usage: Option<crate::usage::Usage>,
    pub usage_year: i64,
    pub connect: Option<Step>,
    pub connect_providers: Vec<connect::ProviderChoice>,
    connect_methods: std::collections::HashMap<String, Vec<connect::Method>>,
    pub menu_query: String,
    pub popup_sel: usize,
    complete_dismissed: Option<String>,
    pub commands: Vec<Entry>,
    pub models: Vec<(String, String, String)>,
    pub model: Option<(String, String)>,
    /// streamed characters per assistant message (live output ticker)
    pub out_chars: std::collections::HashMap<String, usize>,
    /// last rendered pane areas, for mouse-wheel routing
    pub chat_area: ratatui::layout::Rect,
    pub files_area: ratatui::layout::Rect,
    loaded: HashSet<String>,
    tx: UnboundedSender<NetMsg>,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct AgentsPopup {
    pub agent: Option<usize>,
    pub sel: usize,
    pub query: String,
}

pub fn now_ms() -> i64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_millis() as i64).unwrap_or(0)
}

impl App {
    pub fn new(api: Api, root: Option<Session>, tx: UnboundedSender<NetMsg>) -> App {
        let mut input = TextArea::default();
        input.set_cursor_line_style(ratatui::style::Style::default());
        input.set_placeholder_text("Ask anything…  (Enter = new line, Shift/Alt+Enter = send)");
        let id = root.as_ref().map(|s| s.id.clone()).unwrap_or_default();
        let mut store = Store::default();
        if let Some(root) = root {
            store.sessions.insert(id.clone(), root);
        }
        App {
            store,
            api,
            root: id.clone(),
            viewing: id,
            view: View::Chat,
            compact: true,
            chat_scroll: 0,
            files_scroll: 0,
            dialog: None,
            agent: "bake".into(),
            quit_armed: false,
            quit: false,
            notice: None,
            question_sel: 0,
            input,
            recipe: None,
            now: now_ms(),
            focus: Focus::Input,
            menu_open: false,
            sessions_popup: None,
            session_list: Vec::new(),
            file_matches: (String::new(), Vec::new()),
            variant: None,
            edit_request: None,
            rename: None,
            file_sel: 0,
            agent_models: Vec::new(),
            agents_popup: None,
            variants: Default::default(),
            usage: None,
            usage_year: 0,
            connect: None,
            connect_providers: Vec::new(),
            connect_methods: Default::default(),
            menu_query: String::new(),
            popup_sel: 0,
            complete_dismissed: None,
            commands: Vec::new(),
            models: Vec::new(),
            model: None,
            out_chars: Default::default(),
            chat_area: Default::default(),
            files_area: Default::default(),
            loaded: HashSet::new(),
            tx,
        }
    }

    /// Sessions shown as tabs: root + its children.
    pub fn tabs(&self) -> Vec<String> {
        derive::agents(&self.store, &self.root)
    }

    pub fn pending_permission(&self) -> Option<&PermissionRequest> {
        let tabs = self.tabs();
        tabs.iter().find_map(|id| self.store.permissions.get(id).and_then(|l| l.first()))
    }

    pub fn pending_question(&self) -> Option<&QuestionRequest> {
        let tabs = self.tabs();
        tabs.iter().find_map(|id| self.store.questions.get(id).and_then(|l| l.first()))
    }

    pub fn key_context(&self) -> KeyContext {
        let modal = if self.dialog.is_some() {
            Modal::Confirm
        } else if self.pending_permission().is_some() {
            Modal::Permission
        } else if self.pending_question().is_some() {
            Modal::Question
        } else {
            Modal::None
        };
        let popup = if self.agents_popup.is_some() {
            Popup::AgentModels
        } else if self.rename.is_some() {
            Popup::Rename
        } else if self.sessions_popup.is_some() {
            Popup::Sessions
        } else if self.connect.is_some() {
            Popup::Connect
        } else if self.menu_open {
            Popup::Menu
        } else if !self.completions().is_empty() || !self.file_completions().is_empty() {
            Popup::Complete
        } else {
            Popup::None
        };
        KeyContext { modal, in_chat: self.view == View::Chat, focus: self.focus, popup, usage: self.view == View::Usage, diff: matches!(self.view, View::Diff(_)) }
    }

    pub fn input_text(&self) -> String {
        self.input.lines().join("\n")
    }

    /// Completion candidates while the input starts with '/' or '\' (empty when dismissed or not triggered).
    pub fn completions(&self) -> Vec<&Entry> {
        let text = self.input_text();
        if self.focus != Focus::Input || self.complete_dismissed.as_deref() == Some(text.as_str()) {
            return vec![];
        }
        match complete::trigger(&text) {
            Some(q) => complete::matches(&self.commands, q).into_iter().take(8).collect(),
            None => vec![],
        }
    }

    /// P3: file candidates while an "@query" is typed at the end of the input.
    pub fn file_completions(&self) -> Vec<String> {
        let text = self.input_text();
        if self.focus != Focus::Input || self.complete_dismissed.as_deref() == Some(text.as_str()) {
            return vec![];
        }
        match complete::trigger_file(&text) {
            Some(q) if q == self.file_matches.0 => self.file_matches.1.clone(),
            _ => vec![],
        }
    }

    /// P3: search files for the "@query" being typed (results arrive as NetMsg::Files).
    fn search_files(&mut self) {
        let text = self.input_text();
        let Some(q) = complete::trigger_file(&text).map(str::to_string) else { return };
        if q == self.file_matches.0 && !self.file_matches.1.is_empty() {
            return;
        }
        let (api, tx) = (self.api.clone(), self.tx.clone());
        tokio::spawn(async move {
            if let Ok(list) = api.find_files(&q).await {
                let _ = tx.send(NetMsg::Files { query: q, list });
            }
        });
    }

    /// P3: file parts for every "@path" in the text that exists in the project.
    fn file_parts(&self, text: &str) -> Vec<serde_json::Value> {
        let root = std::path::Path::new(self.api.directory());
        complete::mentions(text)
            .into_iter()
            .filter_map(|m| {
                let abs = if std::path::Path::new(m).is_absolute() { std::path::PathBuf::from(m) } else { root.join(m) };
                let abs = abs.canonicalize().ok()?;
                let url = url::Url::from_file_path(&abs).ok()?;
                Some(serde_json::json!({ "type": "file", "mime": complete::mime(m), "url": url.as_str(), "filename": m }))
            })
            .collect()
    }

    /// aioven.tiers as (size, provider/model).
    pub fn tier_models(&self) -> Vec<(String, String)> {
        self.store
            .config
            .get("aioven")
            .and_then(|a| a.get("tiers"))
            .and_then(|t| t.as_object())
            .map(|o| o.iter().filter_map(|(k, v)| v.as_str().map(|m| (k.clone(), m.to_string()))).collect())
            .unwrap_or_default()
    }

    /// T29: rows of the open agents popup (agent list, or the filtered choices of one agent).
    pub fn agents_popup_rows(&self) -> Vec<(String, Option<crate::agent_models::Choice>)> {
        let Some(p) = &self.agents_popup else { return vec![] };
        match p.agent.and_then(|i| self.agent_models.get(i)) {
            None => self
                .agent_models
                .iter()
                .map(|a| {
                    let model = a.model.as_deref().map(|m| m.rsplit('/').next().unwrap_or(m)).unwrap_or("auto");
                    (format!("{:<12} recommended {:<6}  now {} · {model}", a.name, a.recommended, a.tier), None)
                })
                .collect(),
            Some(a) => {
                let q = p.query.to_lowercase();
                crate::agent_models::choices(a, &self.tier_models(), &self.models)
                    .into_iter()
                    .filter(|(l, _)| q.is_empty() || l.to_lowercase().contains(&q))
                    .map(|(l, c)| (l, Some(c)))
                    .collect()
            }
        }
    }

    pub fn menu_items(&self) -> Vec<(String, Item)> {
        let aioven = self.store.config.get("aioven");
        let terse = aioven.and_then(|a| a.get("terse")).and_then(|v| v.as_str()).unwrap_or("ultra").to_string();
        let tiers = self.tier_models();
        let variants = self.current_variants();
        let theme = self.theme_name();
        menu::items(&menu::Context {
            agent: &self.agent,
            compact: self.compact,
            models: &self.models,
            current_model: self.model.as_ref(),
            terse: &terse,
            tiers: &tiers,
            has_session: !self.root.is_empty(),
            reverted: self.store.sessions.get(&self.root).is_some_and(|s| s.revert.is_some()),
            variants: &variants,
            variant: self.variant.as_deref(),
            theme: &theme,
        })
    }

    /// P15: theme from the config (`aioven.theme`, default blue).
    pub fn theme_name(&self) -> String {
        self.store.config.get("aioven").and_then(|a| a.get("theme")).and_then(|v| v.as_str()).unwrap_or("blue").to_string()
    }

    /// P7: the changed file selected in the blueprint.
    pub fn selected_file(&self) -> Option<String> {
        self.store.diffs.get(&self.root)?.get(self.file_sel)?.file.clone()
    }

    /// P6: variants of the model prompts go to (chosen model, else the model of the last answer).
    pub fn current_variants(&self) -> Vec<String> {
        let model = self.model.clone().or_else(|| {
            self.store.messages(&self.root).iter().rev().find_map(|m| m.assistant().map(|a| (a.provider_id.clone(), a.model_id.clone())))
        });
        model.and_then(|m| self.variants.get(&m).cloned()).unwrap_or_default()
    }

    /// T23: sessions matching the popup query (title or id), newest first.
    pub fn sessions_visible(&self) -> Vec<&Session> {
        let q = self.sessions_popup.as_ref().map(|(q, _)| q.to_lowercase()).unwrap_or_default();
        self.session_list.iter().filter(|s| q.is_empty() || s.title.to_lowercase().contains(&q) || s.id.contains(&q)).take(200).collect()
    }

    pub fn open_sessions(&mut self) {
        self.menu_open = false;
        self.sessions_popup = Some((String::new(), 0));
        let (api, tx) = (self.api.clone(), self.tx.clone());
        tokio::spawn(async move {
            let _ = tx.send(match api.sessions().await {
                Ok(list) => NetMsg::Sessions(list),
                Err(e) => NetMsg::Error(format!("sessions: {e:#}")),
            });
        });
    }

    /// T23/P8: reset the screen to `s`, or to a pending new session (created on the first prompt).
    pub fn switch_to(&mut self, s: Option<Session>) {
        let config = std::mem::take(&mut self.store.config);
        let agents = std::mem::take(&mut self.store.agents);
        let skills = std::mem::take(&mut self.store.skills);
        self.store = Store::default();
        self.store.config = config;
        self.store.agents = agents;
        self.store.skills = skills;
        self.root = s.as_ref().map(|s| s.id.clone()).unwrap_or_default();
        self.viewing = self.root.clone();
        if let Some(s) = s {
            self.store.sessions.insert(s.id.clone(), s);
        }
        self.view = View::Chat;
        self.chat_scroll = 0;
        self.files_scroll = 0;
        self.recipe = None;
        self.out_chars.clear();
        self.loaded.clear();
        let root = self.root.clone();
        self.ensure_loaded(&root);
        self.load_children();
        self.refresh_recipe();
        self.load_pending();
    }

    fn save_global(&mut self, patch: serde_json::Value, done: String) {
        let (api, tx) = (self.api.clone(), self.tx.clone());
        tokio::spawn(async move {
            match api.patch_global(patch).await {
                Ok(()) => {
                    let _ = tx.send(NetMsg::Notice(done));
                    if let Ok(cfg) = api.config().await {
                        let _ = tx.send(NetMsg::Config(cfg));
                    }
                }
                Err(e) => {
                    let _ = tx.send(NetMsg::Error(format!("saving settings: {e:#}")));
                }
            }
        });
    }

    pub fn menu_visible(&self) -> Vec<(String, Item)> {
        let all = self.menu_items();
        menu::filter(&all, &self.menu_query).into_iter().map(|i| all[i].clone()).collect()
    }

    /// Running foreground `task` calls in the root session.
    pub fn foreground_tasks(&self) -> usize {
        self.store
            .messages(&self.root)
            .iter()
            .flat_map(|m| self.store.parts(m.id()))
            .filter(|p| matches!(&p.kind, PartKind::Tool { tool, state, .. } if tool == "task" && state.is_active()))
            .count()
    }

    /// Approximate output tokens streamed so far by a session's current assistant message.
    pub fn live_output(&self, session: &str) -> Option<usize> {
        let last = self.store.messages(session).last()?.assistant()?;
        self.out_chars.get(&last.id).map(|c| c / 4).filter(|t| *t > 0)
    }

    /// Permission/question requests that were already waiting (e.g. after a restart or reconnect).
    pub fn load_pending(&self) {
        let (api, tx) = (self.api.clone(), self.tx.clone());
        tokio::spawn(async move {
            let permissions = api.permissions().await.unwrap_or_default();
            let questions = api.questions().await.unwrap_or_default();
            let _ = tx.send(NetMsg::Pending { permissions, questions });
        });
    }

    /// T28: (re)load the agents' models; after start and every settings save.
    pub fn load_agent_models(&self) {
        let (api, tx) = (self.api.clone(), self.tx.clone());
        tokio::spawn(async move {
            if let Ok(list) = api.agent_models().await {
                let _ = tx.send(NetMsg::AgentModels(list));
            }
        });
    }

    pub fn load_meta(&self) {
        self.load_agent_models();
        let (api, tx) = (self.api.clone(), self.tx.clone());
        tokio::spawn(async move {
            if let Ok(list) = api.commands().await {
                let entries = list
                    .into_iter()
                    .map(|c| Entry {
                        skill: c.source.as_deref() == Some("skill"),
                        description: c.description.unwrap_or_default(),
                        name: c.name,
                    })
                    .collect();
                let _ = tx.send(NetMsg::Commands(entries));
            }
            if let Ok(models) = api.models().await {
                let _ = tx.send(NetMsg::Models(models));
            }
            if let Ok(v) = api.variants().await {
                let _ = tx.send(NetMsg::Variants(v));
            }
        });
    }

    fn reset_input(&mut self) {
        let mut t = TextArea::default();
        t.set_cursor_line_style(ratatui::style::Style::default());
        self.input = t;
        self.complete_dismissed = None;
        self.popup_sel = 0;
    }

    pub fn set_input(&mut self, text: &str) {
        self.reset_input();
        self.input.insert_str(text);
    }

    fn run_command(&mut self, name: String, args: String) {
        let agent = self.agent.clone();
        self.reset_input();
        self.viewing = self.root.clone();
        self.view = View::Chat;
        self.chat_scroll = 0;
        self.with_root(move |api, root| async move { api.command(&root, &name, &args, &agent).await });
    }

    pub fn open_connect(&mut self) {
        self.menu_open = false;
        self.connect = Some(Step::Pick { query: String::new(), sel: 0 });
        let (api, tx) = (self.api.clone(), self.tx.clone());
        tokio::spawn(async move {
            let providers = api.provider_choices().await.unwrap_or_default();
            let methods = api.auth_methods().await.unwrap_or_default();
            let _ = tx.send(NetMsg::ConnectData { providers, methods });
        });
    }

    fn connect_input(&mut self, input: connect::Input) {
        let Some(step) = self.connect.take() else { return };
        let methods = self.connect_methods.clone();
        let (step, effect) = connect::next(step, input, &self.connect_providers, |id| methods.get(id).cloned().unwrap_or_default());
        let name = |id: &str| self.connect_providers.iter().find(|p| p.id == id).map(|p| p.name.clone()).unwrap_or_else(|| id.to_string());
        self.connect = Some(step);
        let (api, tx) = (self.api.clone(), self.tx.clone());
        match effect {
            Effect::None => {}
            Effect::SetKey { provider, key } => {
                let label = name(&provider);
                tokio::spawn(async move {
                    let _ = tx.send(match api.set_api_key(&provider, &key).await {
                        Ok(()) => NetMsg::Connected(label),
                        Err(e) => NetMsg::ConnectStep(Step::Busy(format!("failed: {e:#} (esc)"))),
                    });
                });
            }
            Effect::Authorize { provider, method } => {
                let choice = self.connect_providers.iter().find(|p| p.id == provider).cloned().unwrap_or(connect::ProviderChoice {
                    id: provider.clone(),
                    name: provider.clone(),
                    connected: false,
                });
                tokio::spawn(async move {
                    match api.oauth_authorize(&provider, method).await {
                        Ok((url, auto, instructions)) => {
                            let _ = std::process::Command::new("xdg-open")
                                .arg(&url)
                                .stdout(std::process::Stdio::null())
                                .stderr(std::process::Stdio::null())
                                .spawn();
                            let label = choice.name.clone();
                            let _ = tx.send(NetMsg::ConnectStep(Step::Oauth { provider: choice, method, url, auto, instructions, code: String::new() }));
                            if auto {
                                // auto methods finish once the browser login completes
                                let _ = tx.send(match api.oauth_callback(&provider, method, None).await {
                                    Ok(()) => NetMsg::Connected(label),
                                    Err(e) => NetMsg::ConnectStep(Step::Busy(format!("failed: {e:#} (esc)"))),
                                });
                            }
                        }
                        Err(e) => {
                            let _ = tx.send(NetMsg::ConnectStep(Step::Busy(format!("failed: {e:#} (esc)"))));
                        }
                    }
                });
            }
            Effect::Callback { provider, method, code } => {
                let label = name(&provider);
                tokio::spawn(async move {
                    let _ = tx.send(match api.oauth_callback(&provider, method, code).await {
                        Ok(()) => NetMsg::Connected(label),
                        Err(e) => NetMsg::ConnectStep(Step::Busy(format!("failed: {e:#} (esc)"))),
                    });
                });
            }
        }
    }

    pub fn apply_menu(&mut self, item: Item) {
        self.menu_open = false;
        match item {
            Item::Model { provider, model } => {
                self.notice = Some(format!("model for next prompts: {model}"));
                self.model = Some((provider, model));
            }
            Item::Agent(a) => self.agent = a,
            Item::Detail(full) => self.compact = !full,
            Item::Blueprint => self.update(Action::Blueprint),
            Item::StopAll => self.update(Action::StopAll),
            Item::Background => self.update(Action::Background),
            Item::Connect => self.open_connect(),
            Item::Usage => self.update(Action::Usage),
            Item::Sessions => self.open_sessions(),
            Item::AgentModels => {
                self.load_agent_models();
                self.agents_popup = Some(AgentsPopup::default());
            }
            Item::Rename => {
                self.rename = Some(self.store.sessions.get(&self.root).map(|s| s.title.clone()).unwrap_or_default());
            }
            Item::Delete => self.dialog = Some(Dialog::Delete(self.root.clone())),
            Item::Undo => {
                // last user turn that is still visible
                let last = derive::visible(&self.store, &self.root)
                    .iter()
                    .rev()
                    .find(|m| matches!(m, crate::types::Message::User(_)))
                    .map(|m| m.id().to_string());
                let Some(msg) = last else {
                    self.notice = Some("nothing to undo".into());
                    return;
                };
                // put the undone prompt back in the input, like an editor's undo
                let text: String = self
                    .store
                    .parts(&msg)
                    .iter()
                    .filter_map(|p| match &p.kind {
                        crate::types::PartKind::Text { text } => Some(text.clone()),
                        _ => None,
                    })
                    .collect::<Vec<_>>()
                    .join("\n");
                self.set_input(&text);
                let (api, id) = (self.api.clone(), self.root.clone());
                self.spawn(async move { api.revert(&id, &msg).await });
            }
            Item::Redo => {
                let (api, id) = (self.api.clone(), self.root.clone());
                self.reset_input();
                self.spawn(async move { api.unrevert(&id).await });
            }
            Item::Export => {
                let s = self.store.sessions.get(&self.root);
                let name = s.map(|s| if s.slug.is_empty() { s.id.clone() } else { s.slug.clone() }).unwrap_or_default();
                let path = std::path::Path::new(self.api.directory()).join(format!("aioven-{name}.md"));
                self.notice = Some(match std::fs::write(&path, derive::transcript(&self.store, &self.root)) {
                    Ok(()) => format!("exported → {}", path.display()),
                    Err(e) => format!("export failed: {e}"),
                });
            }
            Item::Theme(name) => {
                crate::ui::Theme::set(&name);
                self.save_global(serde_json::json!({ "aioven": { "theme": name } }), format!("theme: {name}"));
            }
            Item::Variant(v) => {
                self.notice = Some(format!("reasoning: {}", v.as_deref().unwrap_or("default")));
                self.variant = v;
            }
            Item::Terse(level) => {
                self.save_global(serde_json::json!({ "aioven": { "terse": level } }), format!("caveman level: {level} (next prompt)"))
            }
            Item::TierModel { tier, provider, model } => self.save_global(
                serde_json::json!({ "aioven": { "tiers": { tier.clone(): format!("{provider}/{model}") } } }),
                format!("tier {tier} → {model}"),
            ),
            Item::Quit => self.quit = true,
        }
    }

    /// Skills the session tree used (from `skill` tool calls), in first-use order.
    pub fn used_skills(&self) -> Vec<String> {
        let mut out: Vec<String> = Vec::new();
        for id in self.tabs() {
            for m in self.store.messages(&id) {
                for p in self.store.parts(m.id()) {
                    if let PartKind::Tool { tool, state, .. } = &p.kind {
                        if tool == "skill" {
                            if let Some(name) = state.input().get("name").and_then(|v| v.as_str()) {
                                if !out.iter().any(|n| n == name) {
                                    out.push(name.to_string());
                                }
                            }
                        }
                    }
                }
            }
        }
        out
    }

    // ---- network work ---------------------------------------------------------------------------

    pub fn ensure_loaded(&mut self, session: &str) {
        if session.is_empty() || !self.loaded.insert(session.to_string()) {
            return;
        }
        let (api, tx, id) = (self.api.clone(), self.tx.clone(), session.to_string());
        tokio::spawn(async move {
            if let Ok(s) = api.session(&id).await {
                let _ = tx.send(NetMsg::Session(s));
            }
            match api.messages(&id, 200).await {
                Ok(items) => {
                    let _ = tx.send(NetMsg::History { session: id.clone(), items });
                }
                Err(e) => {
                    let _ = tx.send(NetMsg::Error(format!("{e:#}")));
                }
            }
            if let Ok(todos) = api.todos(&id).await {
                let _ = tx.send(NetMsg::Todos { session: id.clone(), todos });
            }
            if let Ok(diff) = api.diff(&id).await {
                let _ = tx.send(NetMsg::Diff { session: id.clone(), diff });
            }
        });
    }

    pub fn load_children(&mut self) {
        if self.root.is_empty() {
            return;
        }
        let (api, tx, root) = (self.api.clone(), self.tx.clone(), self.root.clone());
        tokio::spawn(async move {
            if let Ok(kids) = api.children(&root).await {
                for k in kids {
                    let _ = tx.send(NetMsg::Session(k));
                }
            }
        });
    }

    pub fn refresh_recipe(&self) {
        let Some(s) = self.store.sessions.get(&self.root) else { return };
        let path = format!(".opencode/plans/{}-{}.md", s.time.created, s.slug);
        let (api, tx) = (self.api.clone(), self.tx.clone());
        tokio::spawn(async move {
            let _ = tx.send(NetMsg::Recipe(api.read_file(&path).await.ok().flatten()));
        });
    }

    /// P8: run `work` against the root session, creating it first (in the same task) when pending.
    fn with_root<W, F>(&self, work: W)
    where
        W: FnOnce(Api, String) -> F + Send + 'static,
        F: std::future::Future<Output = anyhow::Result<()>> + Send + 'static,
    {
        let (api, tx, root) = (self.api.clone(), self.tx.clone(), self.root.clone());
        tokio::spawn(async move {
            let root = if root.is_empty() {
                match api.session_create().await {
                    Ok(s) => {
                        let id = s.id.clone();
                        let _ = tx.send(NetMsg::Created(s));
                        id
                    }
                    Err(e) => {
                        let _ = tx.send(NetMsg::Error(format!("could not create a session: {e:#}")));
                        return;
                    }
                }
            } else {
                root
            };
            if let Err(e) = work(api, root).await {
                let _ = tx.send(NetMsg::Error(format!("{e:#}")));
            }
        });
    }

    fn spawn<F>(&self, work: F)
    where
        F: std::future::Future<Output = anyhow::Result<()>> + Send + 'static,
    {
        let tx = self.tx.clone();
        tokio::spawn(async move {
            if let Err(e) = work.await {
                let _ = tx.send(NetMsg::Error(format!("{e:#}")));
            }
        });
    }

    // ---- incoming -------------------------------------------------------------------------------

    pub fn on_net(&mut self, msg: NetMsg) {
        match msg {
            NetMsg::History { session, items } => self.store.load_history(&session, items),
            NetMsg::Session(s) => {
                self.store.sessions.insert(s.id.clone(), s);
            }
            NetMsg::Todos { session, todos } => {
                self.store.todos.insert(session, todos);
            }
            NetMsg::Diff { session, diff } => {
                self.store.diffs.insert(session, diff);
            }
            NetMsg::Recipe(md) => self.recipe = md,
            NetMsg::Commands(mut c) => {
                // client-side command: provider login
                c.push(Entry { name: "connect".into(), description: "log in to a model provider".into(), skill: false });
                c.push(Entry { name: "sessions".into(), description: "open a past session".into(), skill: false });
                self.commands = c;
            }
            NetMsg::Models(m) => self.models = m,
            NetMsg::Notice(n) => self.notice = Some(n),
            NetMsg::ConnectData { providers, methods } => {
                self.connect_providers = providers;
                self.connect_methods = methods;
            }
            NetMsg::ConnectStep(step) => {
                if self.connect.is_some() {
                    self.connect = Some(step);
                }
            }
            NetMsg::Sessions(list) => self.session_list = list,
            NetMsg::Files { query, list } => self.file_matches = (query, list),
            NetMsg::Variants(v) => self.variants = v,
            NetMsg::AgentModels(list) => self.agent_models = list,
            NetMsg::Config(cfg) => {
                self.store.config = cfg;
                self.load_agent_models();
                crate::ui::Theme::set(&self.theme_name());
            }
            NetMsg::Created(s) => {
                if self.root.is_empty() {
                    self.loaded.insert(s.id.clone());
                    self.root = s.id.clone();
                    self.viewing = s.id.clone();
                    self.store.sessions.insert(s.id.clone(), s);
                }
            }
            NetMsg::Usage(u) => {
                if self.usage_year == 0 {
                    self.usage_year = u.days.last().and_then(|d| crate::usage::parse(&d.day)).map(|(y, _, _)| y).unwrap_or(1970);
                }
                self.usage = Some(u);
            }
            NetMsg::Connected(name) => {
                self.connect = None;
                self.notice = Some(format!("connected {name} — pick a model with ctrl+p"));
                self.models.clear();
                self.load_meta();
            }
            NetMsg::Pending { permissions, questions } => {
                for p in permissions {
                    store::apply(&mut self.store, Event::PermissionAsked(p));
                }
                for q in questions {
                    store::apply(&mut self.store, Event::QuestionAsked(q));
                }
            }
            NetMsg::Error(e) => self.notice = Some(e),
        }
        self.follow_children();
    }

    pub fn on_event(&mut self, event: Event) {
        let todos_changed = matches!(&event, Event::TodoUpdated { session_id, .. } if *session_id == self.root);
        // the server's live diff event is empty (summary resets it); re-read the diff after edits and when idle
        let diff_changed = !self.root.is_empty()
            && match &event {
                Event::SessionStatus { session_id, status } => {
                    *session_id == self.root && matches!(status, crate::types::SessionStatus::Idle)
                }
                Event::PartUpdated(p) => matches!(&p.kind, PartKind::Patch {}),
                _ => false,
            };
        if let Event::PartDelta { message_id, delta, .. } = &event {
            *self.out_chars.entry(message_id.clone()).or_default() += delta.chars().count();
        }
        store::apply(&mut self.store, event);
        if todos_changed {
            self.refresh_recipe();
        }
        if diff_changed {
            let (api, tx, root) = (self.api.clone(), self.tx.clone(), self.root.clone());
            tokio::spawn(async move {
                if let Ok(diff) = api.diff(&root).await {
                    let _ = tx.send(NetMsg::Diff { session: root, diff });
                }
            });
        }
        self.follow_children();
    }

    /// Load history for subagent sessions as soon as they appear.
    fn follow_children(&mut self) {
        let mut ids = derive::task_sessions(&self.store, &self.root);
        ids.extend(self.tabs().into_iter().skip(1));
        for id in ids {
            self.ensure_loaded(&id);
        }
    }

    // ---- actions --------------------------------------------------------------------------------

    pub fn update(&mut self, action: Action) {
        if !matches!(action, Action::Quit) {
            self.quit_armed = false;
        }
        match action {
            Action::SelectTab(i) => {
                if let Some(id) = self.tabs().get(i as usize).cloned() {
                    self.ensure_loaded(&id);
                    self.viewing = id;
                    self.view = View::Chat;
                    self.chat_scroll = 0;
                }
            }
            Action::Blueprint => {
                // toggle the full-height blueprint (it is always shown above the chat)
                self.view = if self.view == View::Blueprint { View::Chat } else { View::Blueprint };
                self.refresh_recipe();
            }
            Action::NextSkill => {
                let skills = self.used_skills();
                if skills.is_empty() {
                    self.notice = Some("no skills used in this session yet".into());
                    return;
                }
                let next = match &self.view {
                    View::Skill(cur) => skills.iter().position(|s| s == cur).map(|i| (i + 1) % skills.len()).unwrap_or(0),
                    _ => 0,
                };
                self.view = View::Skill(skills[next].clone());
            }
            Action::BackToChat => self.view = View::Chat,
            Action::Send => {
                let text = self.input_text();
                if text.trim().is_empty() {
                    return;
                }
                if text.trim() == "/sessions" || text.trim() == "\\sessions" {
                    self.reset_input();
                    return self.open_sessions();
                }
                if text.trim() == "/connect" || text.trim() == "\\connect" {
                    self.reset_input();
                    return self.open_connect();
                }
                if let Some(cmd) = text.trim().strip_prefix('!').map(str::trim).filter(|c| !c.is_empty()) {
                    let (cmd, agent, model) = (cmd.to_string(), self.agent.clone(), self.model.clone());
                    self.reset_input();
                    self.viewing = self.root.clone();
                    self.view = View::Chat;
                    self.chat_scroll = 0;
                    return self.with_root(move |api, root| async move { api.shell(&root, &agent, &cmd, model).await });
                }
                if let Some((name, args)) = complete::split(&text) {
                    if self.commands.iter().any(|c| c.name == name) {
                        let (name, args) = (name.to_string(), args.to_string());
                        return self.run_command(name, args);
                    }
                }
                self.reset_input();
                self.viewing = self.root.clone();
                self.view = View::Chat;
                self.chat_scroll = 0;
                let (agent, model, variant) = (self.agent.clone(), self.model.clone(), self.variant.clone());
                let files = self.file_parts(&text);
                self.with_root(move |api, root| async move { api.prompt(&root, &text, &agent, model, variant, files).await });
            }
            Action::StopAsk => {
                if derive::wait_of(&self.store, &self.viewing).is_busy() {
                    self.dialog = Some(Dialog::StopOne(self.viewing.clone()));
                } else if self.viewing != self.root {
                    self.viewing = self.root.clone();
                }
            }
            Action::StopAll => {
                let tabs = self.tabs();
                let (root, kids) = tabs.split_first().map(|(r, k)| (r.clone(), k.to_vec())).unwrap_or_default();
                // children first so the root never waits on a child being stopped
                let busy: Vec<String> = kids
                    .into_iter()
                    .chain(std::iter::once(root))
                    .filter(|id| derive::wait_of(&self.store, id).is_busy())
                    .collect();
                if !busy.is_empty() {
                    self.dialog = Some(Dialog::StopAll(busy));
                }
            }
            Action::Confirm(yes) => {
                let dialog = self.dialog.take();
                if !yes {
                    return;
                }
                let ids = match dialog {
                    Some(Dialog::StopOne(id)) => vec![id],
                    Some(Dialog::StopAll(ids)) => ids,
                    Some(Dialog::Delete(id)) => {
                        let api = self.api.clone();
                        self.switch_to(None);
                        self.notice = Some("session deleted".into());
                        return self.spawn(async move { api.delete(&id).await });
                    }
                    None => vec![],
                };
                let api = self.api.clone();
                self.spawn(async move {
                    for id in ids {
                        api.abort(&id).await?;
                    }
                    Ok(())
                });
            }
            Action::Permission(reply) => {
                if let Some((req, session)) = self.pending_permission().map(|r| (r.id.clone(), r.session_id.clone())) {
                    // answer once: drop it locally so a repeated key can't reply twice (server 404)
                    store::apply(&mut self.store, Event::PermissionReplied { session_id: session, request_id: req.clone() });
                    let api = self.api.clone();
                    self.spawn(async move { api.permission_reply(&req, reply).await });
                }
            }
            Action::QuestionMove(d) => {
                let n = self.pending_question().and_then(|q| q.questions.first()).map_or(0, |q| q.options.len());
                if n > 0 {
                    self.question_sel = (self.question_sel as i64 + d as i64).rem_euclid(n as i64) as usize;
                }
            }
            Action::QuestionAnswer => {
                let Some(q) = self.pending_question() else { return };
                let label = q.questions.first().and_then(|i| i.options.get(self.question_sel)).map(|o| o.label.clone());
                let (api, id, session) = (self.api.clone(), q.id.clone(), q.session_id.clone());
                self.question_sel = 0;
                store::apply(&mut self.store, Event::QuestionDone { session_id: session, request_id: id.clone() });
                if let Some(label) = label {
                    self.spawn(async move { api.question_reply(&id, vec![vec![label]]).await });
                }
            }
            Action::QuestionReject => {
                if let Some(id) = self.pending_question().map(|q| q.id.clone()) {
                    let api = self.api.clone();
                    self.spawn(async move { api.question_reject(&id).await });
                }
            }
            Action::ToggleAgent => self.agent = if self.agent == "bake" { "recipe".into() } else { "bake".into() },
            Action::ToggleDetail => self.compact = !self.compact,
            Action::Background => {
                let n = self.foreground_tasks();
                if n == 0 {
                    self.notice = Some("nothing running in the foreground".into());
                    return;
                }
                let (api, root, tx) = (self.api.clone(), self.root.clone(), self.tx.clone());
                tokio::spawn(async move {
                    let msg = match api.background(&root).await {
                        Ok(()) => NetMsg::Notice(format!("moved {n} task(s) to the background")),
                        Err(e) => NetMsg::Error(format!("{e:#}")),
                    };
                    let _ = tx.send(msg);
                });
            }
            Action::Usage => {
                self.view = if self.view == View::Usage { View::Chat } else { View::Usage };
                let (api, tx) = (self.api.clone(), self.tx.clone());
                tokio::spawn(async move {
                    let _ = tx.send(match api.usage().await {
                        Ok(u) => NetMsg::Usage(u),
                        Err(e) => NetMsg::Error(format!("usage: {e:#}")),
                    });
                });
            }
            Action::Year(d) => self.usage_year += d as i64,
            Action::Editor => self.edit_request = Some(self.input_text()),
            Action::OpenMenu => {
                self.menu_open = true;
                self.menu_query.clear();
                self.popup_sel = 0;
                if self.models.is_empty() {
                    self.load_meta();
                }
            }
            Action::FocusNext => {
                self.focus = match self.focus {
                    Focus::Input => Focus::Chat,
                    Focus::Chat => Focus::Files,
                    Focus::Files => Focus::Input,
                }
            }
            Action::FocusInput => self.focus = Focus::Input,
            Action::SelectFile(d) => {
                let n = self.store.diffs.get(&self.root).map(Vec::len).unwrap_or(0);
                if n > 0 {
                    self.file_sel = (self.file_sel as i64 + d as i64).rem_euclid(n as i64) as usize;
                }
            }
            Action::OpenDiff => {
                if let Some(file) = self.selected_file() {
                    self.view = View::Diff(file);
                    self.files_scroll = 0;
                    self.focus = Focus::Files;
                }
            }
            Action::Scroll(Pane::Chat, d) => self.chat_scroll = (self.chat_scroll as i32 - d as i32).clamp(0, 10_000) as u16,
            Action::Scroll(Pane::Files, d) => self.files_scroll = (self.files_scroll as i32 + d as i32).clamp(0, 10_000) as u16,
            Action::ScrollEdge(Pane::Chat, top) => self.chat_scroll = if top { 10_000 } else { 0 },
            Action::ScrollEdge(Pane::Files, top) => self.files_scroll = if top { 0 } else { 10_000 },
            Action::PopupMove(d) if self.agents_popup.is_some() => {
                let n = self.agents_popup_rows().len().max(1) as i64;
                if let Some(p) = self.agents_popup.as_mut() {
                    p.sel = (p.sel as i64 + d as i64).rem_euclid(n) as usize;
                }
            }
            Action::PopupAccept if self.agents_popup.is_some() => {
                let rows = self.agents_popup_rows();
                let Some(p) = self.agents_popup.clone() else { return };
                match p.agent {
                    None if p.sel < self.agent_models.len() => {
                        self.agents_popup = Some(AgentsPopup { agent: Some(p.sel), sel: 0, query: String::new() })
                    }
                    Some(i) => {
                        if let (Some(a), Some((label, Some(choice)))) = (self.agent_models.get(i).cloned(), rows.get(p.sel).cloned()) {
                            let label = label.trim_end_matches("  ✓").to_string();
                            self.save_global(crate::agent_models::patch(&a, &choice), format!("{} → {label}", a.name));
                        }
                        self.agents_popup = Some(AgentsPopup { agent: None, sel: i, query: String::new() });
                    }
                    None => {}
                }
            }
            Action::PopupClose if self.agents_popup.is_some() => {
                let back = self.agents_popup.as_ref().and_then(|p| p.agent);
                self.agents_popup = back.map(|i| AgentsPopup { agent: None, sel: i, query: String::new() });
            }
            Action::MenuInput(key) if self.agents_popup.is_some() => {
                use crossterm::event::KeyCode;
                if let Some(p) = self.agents_popup.as_mut().filter(|p| p.agent.is_some()) {
                    match key.code {
                        KeyCode::Char(c) => p.query.push(c),
                        KeyCode::Backspace => {
                            p.query.pop();
                        }
                        _ => {}
                    }
                    p.sel = 0;
                }
            }
            Action::PopupAccept if self.rename.is_some() => {
                let title = self.rename.take().unwrap_or_default();
                let (api, id) = (self.api.clone(), self.root.clone());
                if !title.trim().is_empty() && !id.is_empty() {
                    self.spawn(async move { api.rename(&id, title.trim()).await });
                }
            }
            Action::PopupClose if self.rename.is_some() => self.rename = None,
            Action::MenuInput(key) if self.rename.is_some() => {
                use crossterm::event::KeyCode;
                if let Some(t) = self.rename.as_mut() {
                    match key.code {
                        KeyCode::Char(c) => t.push(c),
                        KeyCode::Backspace => {
                            t.pop();
                        }
                        _ => {}
                    }
                }
            }
            Action::PopupMove(_) if self.rename.is_some() => {}
            Action::PopupMove(d) if self.sessions_popup.is_some() => {
                let n = self.sessions_visible().len() as i64 + 1; // row 0 = new session
                if let Some((_, sel)) = self.sessions_popup.as_mut() {
                    *sel = (*sel as i64 + d as i64).rem_euclid(n) as usize;
                }
            }
            Action::PopupAccept if self.sessions_popup.is_some() => {
                let sel = self.sessions_popup.as_ref().map(|(_, s)| *s).unwrap_or(0);
                let picked = if sel == 0 { None } else { self.sessions_visible().get(sel - 1).map(|s| (*s).clone()) };
                self.sessions_popup = None;
                self.switch_to(picked);
            }
            Action::PopupClose if self.sessions_popup.is_some() => self.sessions_popup = None,
            Action::MenuInput(key) if self.sessions_popup.is_some() => {
                use crossterm::event::KeyCode;
                if let Some((q, sel)) = self.sessions_popup.as_mut() {
                    match key.code {
                        KeyCode::Char(c) => q.push(c),
                        KeyCode::Backspace => {
                            q.pop();
                        }
                        _ => {}
                    }
                    *sel = 0;
                }
            }
            Action::PopupMove(d) if self.connect.is_some() => self.connect_input(connect::Input::Move(d)),
            Action::PopupAccept if self.connect.is_some() => self.connect_input(connect::Input::Enter),
            Action::PopupClose if self.connect.is_some() => self.connect = None,
            Action::MenuInput(key) if self.connect.is_some() => {
                use crossterm::event::KeyCode;
                match key.code {
                    KeyCode::Char(c) => self.connect_input(connect::Input::Char(c)),
                    KeyCode::Backspace => self.connect_input(connect::Input::Backspace),
                    _ => {}
                }
            }
            Action::PopupMove(d) => {
                let files = self.file_completions().len();
                let n = if self.menu_open { self.menu_visible().len() } else if files > 0 { files } else { self.completions().len() };
                if n > 0 {
                    self.popup_sel = (self.popup_sel as i64 + d as i64).rem_euclid(n as i64) as usize;
                }
            }
            Action::PopupComplete | Action::PopupAccept if !self.menu_open && !self.file_completions().is_empty() => {
                if let Some(path) = self.file_completions().get(self.popup_sel).cloned() {
                    let text = complete::insert_file(&self.input_text(), &path);
                    self.set_input(&text);
                }
            }
            Action::PopupComplete => {
                let prefix = self.input_text().chars().next().unwrap_or('/');
                if let Some(e) = self.completions().get(self.popup_sel).map(|e| e.name.clone()) {
                    self.set_input(&format!("{prefix}{e} "));
                }
            }
            Action::PopupAccept => {
                if self.menu_open {
                    if let Some((_, item)) = self.menu_visible().get(self.popup_sel).cloned() {
                        self.apply_menu(item);
                    }
                } else if let Some(name) = self.completions().get(self.popup_sel).map(|e| e.name.clone()) {
                    if name == "connect" {
                        self.reset_input();
                        return self.open_connect();
                    }
                    if name == "sessions" {
                        self.reset_input();
                        return self.open_sessions();
                    }
                    let args = complete::split(&self.input_text()).map(|(_, a)| a.to_string()).unwrap_or_default();
                    self.run_command(name, args);
                }
            }
            Action::PopupClose => {
                if self.menu_open {
                    self.menu_open = false;
                } else {
                    self.complete_dismissed = Some(self.input_text());
                }
            }
            Action::MenuInput(key) => {
                use crossterm::event::KeyCode;
                match key.code {
                    KeyCode::Char(c) => self.menu_query.push(c),
                    KeyCode::Backspace => {
                        self.menu_query.pop();
                    }
                    _ => {}
                }
                self.popup_sel = 0;
            }
            Action::ScrollChat(d) => self.chat_scroll = (self.chat_scroll as i32 - d as i32).max(0) as u16,
            Action::ScrollFiles(d) => self.files_scroll = (self.files_scroll as i32 + d as i32).max(0) as u16,
            Action::Quit => {
                if self.quit_armed {
                    self.quit = true;
                } else {
                    self.quit_armed = true;
                    self.notice = Some("press Ctrl+C again to quit".into());
                }
            }
            Action::Input(key) => {
                self.input.input(key);
                self.popup_sel = 0;
                self.search_files();
            }
        }
    }

    pub fn wait(&self, id: &str) -> Wait {
        derive::wait_of(&self.store, id)
    }
}
