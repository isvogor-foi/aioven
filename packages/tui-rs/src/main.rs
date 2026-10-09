//! aioven-tui: Ratatui client for the AIOven server (see ARCHITECTURE-AIOVEN.md, T6).
//!
//! Usage: aioven-tui [project-dir] [-s <session-id>] [--attach <url>]

mod agent_models;
mod api;
mod app;
mod brand;
mod budget;
mod compact;
mod complete;
mod connect;
mod derive;
mod events;
mod keys;
mod markdown;
mod menu;
mod plan_model;
mod server;
mod store;
mod types;
mod usage;
mod ui;

use std::io::stdout;
use std::path::PathBuf;
use std::time::Duration;

use anyhow::{Context, Result};
use crossterm::event::{
    DisableMouseCapture, EnableMouseCapture, Event as TermEvent, EventStream, KeyboardEnhancementFlags,
    MouseEventKind, PopKeyboardEnhancementFlags, PushKeyboardEnhancementFlags,
};
use crossterm::execute;
use crossterm::terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode};
use futures::StreamExt;
use ratatui::Terminal;
use ratatui::backend::CrosstermBackend;
use tokio::sync::mpsc;

use crate::api::Api;
use crate::app::{App, NetMsg, now_ms};
use crate::events::StreamMsg;
use crate::server::Server;

struct Args {
    project: PathBuf,
    session: Option<String>,
    attach: Option<url::Url>,
    continue_last: bool,
    model: Option<(String, String)>,
    agent: Option<String>,
    prompt: Option<String>,
}

fn parse_args() -> Result<Args> {
    let mut project = std::env::current_dir()?;
    let mut session = None;
    let mut attach = None;
    let (mut continue_last, mut model, mut agent, mut prompt) = (false, None, None, None);
    let mut it = std::env::args().skip(1);
    while let Some(a) = it.next() {
        match a.as_str() {
            "-s" | "--session" => session = it.next(),
            "--attach" => attach = Some(url::Url::parse(&it.next().context("--attach needs a url")?)?),
            "-c" | "--continue" => continue_last = true,
            "-m" | "--model" => {
                let m = it.next().context("--model needs provider/model")?;
                let (p, id) = m.split_once('/').context("--model must be provider/model")?;
                model = Some((p.to_string(), id.to_string()));
            }
            "--agent" => agent = it.next(),
            "--prompt" => prompt = it.next(),
            "-h" | "--help" => {
                println!("aioven-tui [project-dir] [-s <session-id> | -c] [--attach <url>] [--model provider/model] [--agent name] [--prompt text]");
                std::process::exit(0);
            }
            other => project = std::fs::canonicalize(other).with_context(|| format!("no such directory: {other}"))?,
        }
    }
    Ok(Args { project, session, attach, continue_last, model, agent, prompt })
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = parse_args()?;
    let server = match args.attach {
        Some(url) => Server::attach(url, &args.project),
        None => Server::start(&args.project).await?,
    };
    let api = Api::new(server.base_url.clone(), server.directory.to_string_lossy().to_string());

    let latest = if args.continue_last && args.session.is_none() { api.latest_session().await.ok().flatten() } else { None };
    let root = match (&args.session, latest) {
        (Some(id), _) => Some(api.session(id).await.with_context(|| format!("session {id} not found"))?),
        (None, Some(s)) => Some(s),
        // P8: no session until the first prompt
        (None, None) => None,
    };
    let (net_tx, mut net_rx) = mpsc::unbounded_channel::<NetMsg>();
    let mut app = App::new(api.clone(), root, net_tx);
    if let Some(a) = args.agent {
        app.agent = a;
    }
    app.model = args.model;
    app.store.agents = api.agents().await.unwrap_or_default();
    app.store.skills = api.skills().await.unwrap_or_default();
    app.store.config = api.config().await.unwrap_or_default();
    ui::Theme::set(&app.theme_name());
    app.store.status = api.status().await.unwrap_or_default();
    let root_id = app.root.clone();
    app.ensure_loaded(&root_id);
    app.load_children();
    app.refresh_recipe();
    app.load_meta();
    app.load_pending();
    let mut stream = events::subscribe(&api);
    if let Some(text) = args.prompt {
        app.input.insert_str(&text);
        app.update(keys::Action::Send);
    }

    let enhanced = enter_tui()?;
    let mut terminal = Terminal::new(CrosstermBackend::new(stdout()))?;
    let result = run(&mut terminal, &mut app, &mut net_rx, &mut stream, enhanced).await;
    leave_tui(enhanced);
    terminal.show_cursor()?;
    drop(server);

    // exit screen (T1 Brand)
    let title = app.store.sessions.get(&app.root).map(|s| s.title.clone()).unwrap_or_default();
    println!();
    for line in brand::ansi_logo("  ") {
        println!("{line}");
    }
    println!();
    if !app.root.is_empty() {
        println!("  \x1b[90mSession   \x1b[0m\x1b[1m{title}\x1b[0m");
        println!("  \x1b[90mContinue  \x1b[0m\x1b[1m{} -s {}\x1b[0m", brand::COMMAND, app.root);
    }
    println!();
    result
}

/// Raw mode, alternate screen, mouse, kitty keys and modifyOtherKeys. Returns whether kitty keys were enabled.
fn enter_tui() -> Result<bool> {
    use std::io::Write;
    enable_raw_mode()?;
    let mut out = stdout();
    execute!(out, EnterAlternateScreen, EnableMouseCapture)?;
    // kitty protocol: lets Shift+Enter and Ctrl+digits be told apart (ignored by terminals without it)
    let enhanced =
        execute!(out, PushKeyboardEnhancementFlags(KeyboardEnhancementFlags::DISAMBIGUATE_ESCAPE_CODES)).is_ok();
    // xterm modifyOtherKeys: tmux (extended-keys on) only reports Shift+Enter etc. when the app asks this way
    let _ = out.write_all(b"\x1b[>4;2m");
    let _ = out.flush();
    Ok(enhanced)
}

/// Undo everything `enter_tui` set.
fn leave_tui(enhanced: bool) {
    use std::io::Write;
    let mut out = stdout();
    if enhanced {
        let _ = execute!(out, PopKeyboardEnhancementFlags);
    }
    let _ = out.write_all(b"\x1b[>4;0m");
    let _ = disable_raw_mode();
    let _ = execute!(out, DisableMouseCapture, LeaveAlternateScreen, crossterm::cursor::Show);
    let _ = out.flush();
}

/// P4: edit `text` in $VISUAL/$EDITOR (fallback vi); None when the editor failed.
fn edit_in_editor(text: &str) -> Option<String> {
    let path = std::env::temp_dir().join(format!("aioven-prompt-{}.md", std::process::id()));
    std::fs::write(&path, text).ok()?;
    let editor = std::env::var("VISUAL").or_else(|_| std::env::var("EDITOR")).unwrap_or_else(|_| "vi".into());
    let mut parts = editor.split_whitespace();
    let status = std::process::Command::new(parts.next()?).args(parts).arg(&path).status().ok()?;
    let out = std::fs::read_to_string(&path).ok();
    let _ = std::fs::remove_file(&path);
    status.success().then_some(out?.trim_end().to_string())
}

async fn run(
    terminal: &mut Terminal<CrosstermBackend<std::io::Stdout>>,
    app: &mut App,
    net_rx: &mut mpsc::UnboundedReceiver<NetMsg>,
    stream: &mut mpsc::UnboundedReceiver<StreamMsg>,
    enhanced: bool,
) -> Result<()> {
    let mut keys = EventStream::new();
    // 100 ms ticks keep spinners and timers live
    let mut tick = tokio::time::interval(Duration::from_millis(100));
    loop {
        app.now = now_ms();
        terminal.draw(|f| ui::render(f, app))?;
        tokio::select! {
            Some(Ok(ev)) = keys.next() => {
                match ev {
                    TermEvent::Key(key) => {
                        if let Some(action) = keys::map_key(key, &app.key_context()) {
                            app.update(action);
                        }
                    }
                    TermEvent::Mouse(m) => {
                        let delta = match m.kind {
                            MouseEventKind::ScrollUp => -3,
                            MouseEventKind::ScrollDown => 3,
                            _ => 0,
                        };
                        let hit = |r: ratatui::layout::Rect| m.column >= r.x && m.column < r.x + r.width && m.row >= r.y && m.row < r.y + r.height;
                        if delta != 0 {
                            if hit(app.chat_area) {
                                app.update(keys::Action::Scroll(keys::Pane::Chat, delta));
                            } else if hit(app.files_area) {
                                app.update(keys::Action::Scroll(keys::Pane::Files, delta));
                            }
                        }
                    }
                    _ => {}
                }
            }
            Some(msg) = stream.recv() => match msg {
                StreamMsg::Event(e) => app.on_event(e),
                StreamMsg::Connected => {
                    app.notice = None;
                    app.load_pending();
                }
                StreamMsg::Disconnected(e) => app.notice = Some(format!("reconnecting… {e}")),
            },
            Some(msg) = net_rx.recv() => app.on_net(msg),
            _ = tick.tick() => {}
        }
        if app.quit {
            return Ok(());
        }
        if let Some(text) = app.edit_request.take() {
            // stop reading keys first, or the event reader steals the editor's input
            drop(keys);
            leave_tui(enhanced);
            let edited = edit_in_editor(&text);
            enter_tui()?;
            terminal.clear()?;
            keys = EventStream::new();
            match edited {
                Some(t) => app.set_input(&t),
                None => app.notice = Some("editor failed (set $EDITOR)".into()),
            }
        }
    }
}
