//! R8 App: owns Store + UI state; `update(Action)` changes state and starts network work.
//! Network results come back as `NetMsg` so rendering never blocks.

use std::collections::HashSet;

use tokio::sync::mpsc::UnboundedSender;
use tui_textarea::TextArea;

use crate::api::Api;
use crate::derive::{self, Wait};
use crate::complete::{self, Entry};
use crate::keys::{Action, Focus, KeyContext, Modal, Pane, Popup};
use crate::menu::{self, Item};
use crate::store::{self, Store};
use crate::types::*;

#[derive(Debug, Clone, PartialEq)]
pub enum View {
    Chat,
    Blueprint,
    Skill(String),
}

#[derive(Debug, Clone, PartialEq)]
pub enum Dialog {
    StopOne(String),
    StopAll(Vec<String>),
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
    Error(String),
}

pub struct App {
    pub store: Store,
    pub api: Api,
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

pub fn now_ms() -> i64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_millis() as i64).unwrap_or(0)
}

impl App {
    pub fn new(api: Api, root: Session, tx: UnboundedSender<NetMsg>) -> App {
        let mut input = TextArea::default();
        input.set_cursor_line_style(ratatui::style::Style::default());
        input.set_placeholder_text("Ask anything…  (Enter = new line, Shift/Alt+Enter = send)");
        let id = root.id.clone();
        let mut store = Store::default();
        store.sessions.insert(id.clone(), root);
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
        let popup = if self.menu_open {
            Popup::Menu
        } else if !self.completions().is_empty() {
            Popup::Complete
        } else {
            Popup::None
        };
        KeyContext { modal, in_chat: self.view == View::Chat, focus: self.focus, popup }
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

    pub fn menu_items(&self) -> Vec<(String, Item)> {
        menu::items(&menu::Context {
            agent: &self.agent,
            compact: self.compact,
            models: &self.models,
            current_model: self.model.as_ref(),
        })
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

    pub fn load_meta(&self) {
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
        });
    }

    fn reset_input(&mut self) {
        let mut t = TextArea::default();
        t.set_cursor_line_style(ratatui::style::Style::default());
        self.input = t;
        self.complete_dismissed = None;
        self.popup_sel = 0;
    }

    fn set_input(&mut self, text: &str) {
        self.reset_input();
        self.input.insert_str(text);
    }

    fn run_command(&mut self, name: String, args: String) {
        let (api, root, agent) = (self.api.clone(), self.root.clone(), self.agent.clone());
        self.reset_input();
        self.viewing = self.root.clone();
        self.view = View::Chat;
        self.chat_scroll = 0;
        self.spawn(async move { api.command(&root, &name, &args, &agent).await });
    }

    fn apply_menu(&mut self, item: Item) {
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
        if !self.loaded.insert(session.to_string()) {
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
            NetMsg::Commands(c) => self.commands = c,
            NetMsg::Models(m) => self.models = m,
            NetMsg::Notice(n) => self.notice = Some(n),
            NetMsg::Error(e) => self.notice = Some(e),
        }
        self.follow_children();
    }

    pub fn on_event(&mut self, event: Event) {
        let todos_changed = matches!(&event, Event::TodoUpdated { session_id, .. } if *session_id == self.root);
        if let Event::PartDelta { message_id, delta, .. } = &event {
            *self.out_chars.entry(message_id.clone()).or_default() += delta.chars().count();
        }
        store::apply(&mut self.store, event);
        if todos_changed {
            self.refresh_recipe();
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
                self.view = View::Blueprint;
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
                let (api, root, agent, model) = (self.api.clone(), self.root.clone(), self.agent.clone(), self.model.clone());
                self.spawn(async move { api.prompt(&root, &text, &agent, model).await });
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
                if let Some(req) = self.pending_permission().map(|r| r.id.clone()) {
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
                let (api, id) = (self.api.clone(), q.id.clone());
                self.question_sel = 0;
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
            Action::Scroll(Pane::Chat, d) => self.chat_scroll = (self.chat_scroll as i32 - d as i32).clamp(0, 10_000) as u16,
            Action::Scroll(Pane::Files, d) => self.files_scroll = (self.files_scroll as i32 + d as i32).clamp(0, 10_000) as u16,
            Action::ScrollEdge(Pane::Chat, top) => self.chat_scroll = if top { 10_000 } else { 0 },
            Action::ScrollEdge(Pane::Files, top) => self.files_scroll = if top { 0 } else { 10_000 },
            Action::PopupMove(d) => {
                let n = if self.menu_open { self.menu_visible().len() } else { self.completions().len() };
                if n > 0 {
                    self.popup_sel = (self.popup_sel as i64 + d as i64).rem_euclid(n as i64) as usize;
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
            }
        }
    }

    pub fn wait(&self, id: &str) -> Wait {
        derive::wait_of(&self.store, id)
    }
}
