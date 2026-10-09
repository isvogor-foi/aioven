//! P10: render tests on ratatui's TestBackend (screen text assertions).

use ratatui::Terminal;
use ratatui::backend::TestBackend;

use crate::api::Api;
use crate::app::{App, View};
use crate::keys::{Action, Focus};
use crate::types::{AgentModel, FileDiff, PermissionRequest, Session};

fn app(root: Option<Session>) -> App {
    let (tx, _rx) = tokio::sync::mpsc::unbounded_channel();
    let api = Api::new(url::Url::parse("http://127.0.0.1:1/").unwrap(), "/tmp/project".into());
    App::new(api, root, tx)
}

fn session(id: &str) -> Session {
    Session { id: id.into(), title: "Fix the oven".into(), ..Default::default() }
}

fn screen(app: &mut App, w: u16, h: u16) -> String {
    let mut t = Terminal::new(TestBackend::new(w, h)).unwrap();
    t.draw(|f| super::render(f, app)).unwrap();
    let buf = t.backend().buffer().clone();
    (0..h).map(|y| (0..w).map(|x| buf[(x, y)].symbol().to_string()).collect::<String>() + "\n").collect()
}

#[test]
fn top_bar_and_status_line() {
    let mut a = app(Some(session("ses_1")));
    let s = screen(&mut a, 160, 30);
    let top = s.lines().next().unwrap();
    assert!(top.contains("0 bake"), "{top}");
    assert!(top.contains("U usage") && top.contains("G blueprint"), "{top}");
    let status = s.lines().last().unwrap();
    assert!(status.contains("⇧⏎ send") && status.contains("ctrl+g blueprint"), "{status}");
    assert!(s.contains("BLUEPRINT"));
}

#[test]
fn pending_session_renders_without_root() {
    let mut a = app(None);
    let s = screen(&mut a, 120, 24);
    assert!(s.contains("0 bake"));
    assert!(s.contains("No recipe and no changed files yet."));
}

#[test]
fn permission_dialog() {
    let mut a = app(Some(session("ses_1")));
    a.store.permissions.insert(
        "ses_1".into(),
        vec![PermissionRequest { id: "per_1".into(), session_id: "ses_1".into(), permission: "bash".into(), patterns: vec!["rm -rf build".into()] }],
    );
    let s = screen(&mut a, 120, 30);
    assert!(s.contains("PERMISSION") && s.contains("wants permission: bash") && s.contains("rm -rf build"), "{s}");
}

#[test]
fn diff_view_and_file_selection() {
    let mut a = app(Some(session("ses_1")));
    a.store.diffs.insert(
        "ses_1".into(),
        vec![FileDiff {
            file: Some("src/oven.rs".into()),
            additions: 1,
            deletions: 1,
            status: Some("modified".into()),
            patch: Some("--- src/oven.rs\n+++ src/oven.rs\n@@ -1 +1 @@\n-cold\n+hot".into()),
        }],
    );
    a.focus = Focus::Files;
    let s = screen(&mut a, 140, 30);
    assert!(s.contains("[ ] file · ⏎ diff"), "{s}");
    a.update(Action::OpenDiff);
    assert_eq!(a.view, View::Diff("src/oven.rs".into()));
    let s = screen(&mut a, 140, 30);
    assert!(s.contains("DIFF src/oven.rs +1 −1") && s.contains("+hot") && s.contains("-cold"), "{s}");
}

#[test]
fn usage_page_and_theme() {
    let mut a = app(Some(session("ses_1")));
    a.view = View::Usage;
    let s = screen(&mut a, 160, 40);
    assert!(s.contains("USAGE"), "{s}");
    super::Theme::set("light");
    assert_eq!(super::Theme::c(), super::palette("light"));
    super::Theme::set("blue");
}

#[tokio::test]
async fn agents_section_and_models_popup() {
    let mut a = app(Some(session("ses_1")));
    let agent = |name: &str, rec: &str, tier: &str, model: Option<&str>, source: &str| AgentModel {
        name: name.into(),
        mode: "subagent".into(),
        recommended: rec.into(),
        tier: tier.into(),
        model: model.map(str::to_string),
        source: source.into(),
    };
    a.agent_models = vec![
        agent("bake", "medium", "medium", Some("google/gemini-flash"), "default"),
        agent("pantry", "small", "small", Some("gh/gpt-mini"), "tier"),
    ];
    let s = screen(&mut a, 160, 40);
    assert!(s.contains("AGENTS · ctrl+p models"), "{s}");
    assert!(s.contains("bake        M gemini-flash") && s.contains("pantry      S gpt-mini"), "{s}");
    a.update(Action::OpenMenu);
    a.apply_menu(crate::menu::Item::AgentModels);
    a.update(Action::PopupMove(1));
    a.update(Action::PopupAccept);
    let s = screen(&mut a, 160, 40);
    assert!(s.contains("PANTRY MODEL") && s.contains("★ Recommended: small → default model"), "{s}");
}
