//! R7 Keymap: pure key → Action mapping. Tab keys never type into the input.

use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Modal {
    None,
    Confirm,
    Permission,
    Question,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Focus {
    Input,
    Chat,
    Files,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Popup {
    None,
    Complete,
    Menu,
    Connect,
    Sessions,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Pane {
    Chat,
    Files,
}

#[derive(Debug, Clone, Copy)]
pub struct KeyContext {
    pub modal: Modal,
    /// true when the chat view is shown (not blueprint / a skill tab)
    pub in_chat: bool,
    pub focus: Focus,
    pub popup: Popup,
    /// usage page is shown (←/→ switch year)
    pub usage: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Action {
    SelectTab(u8),
    Blueprint,
    NextSkill,
    BackToChat,
    Send,
    StopAsk,
    StopAll,
    Confirm(bool),
    Permission(crate::types::Reply),
    QuestionMove(i8),
    QuestionAnswer,
    QuestionReject,
    ToggleAgent,
    ToggleDetail,
    ScrollChat(i16),
    ScrollFiles(i16),
    Scroll(Pane, i16),
    ScrollEdge(Pane, bool),
    Background,
    OpenMenu,
    Usage,
    Year(i8),
    FocusNext,
    FocusInput,
    PopupMove(i8),
    PopupAccept,
    PopupComplete,
    PopupClose,
    MenuInput(KeyEvent),
    Quit,
    Input(KeyEvent),
}

fn digit(c: char) -> Option<u8> {
    c.to_digit(10).map(|d| d as u8)
}

pub fn map_key(key: KeyEvent, ctx: &KeyContext) -> Option<Action> {
    if key.kind == KeyEventKind::Release {
        return None;
    }
    let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
    let alt = key.modifiers.contains(KeyModifiers::ALT);
    let shift = key.modifiers.contains(KeyModifiers::SHIFT);

    if ctrl && key.code == KeyCode::Char('c') {
        return Some(Action::Quit);
    }

    match ctx.modal {
        Modal::Confirm => {
            return match key.code {
                KeyCode::Char('y') | KeyCode::Enter => Some(Action::Confirm(true)),
                KeyCode::Char('n') | KeyCode::Esc => Some(Action::Confirm(false)),
                _ => None,
            };
        }
        Modal::Permission => {
            use crate::types::Reply;
            return match key.code {
                KeyCode::Char('y') | KeyCode::Enter => Some(Action::Permission(Reply::Once)),
                KeyCode::Char('a') => Some(Action::Permission(Reply::Always)),
                KeyCode::Char('n') | KeyCode::Esc => Some(Action::Permission(Reply::Reject)),
                _ => None,
            };
        }
        Modal::Question => {
            return match key.code {
                KeyCode::Up => Some(Action::QuestionMove(-1)),
                KeyCode::Down => Some(Action::QuestionMove(1)),
                KeyCode::Enter => Some(Action::QuestionAnswer),
                KeyCode::Esc => Some(Action::QuestionReject),
                _ => None,
            };
        }
        Modal::None => {}
    }

    match ctx.popup {
        Popup::Menu | Popup::Connect | Popup::Sessions => {
            return match key.code {
                KeyCode::Up => Some(Action::PopupMove(-1)),
                KeyCode::Down => Some(Action::PopupMove(1)),
                KeyCode::Enter => Some(Action::PopupAccept),
                KeyCode::Esc => Some(Action::PopupClose),
                _ => Some(Action::MenuInput(key)),
            };
        }
        Popup::Complete => match key.code {
            KeyCode::Up => return Some(Action::PopupMove(-1)),
            KeyCode::Down => return Some(Action::PopupMove(1)),
            KeyCode::Tab => return Some(Action::PopupComplete),
            KeyCode::Enter if !shift && !alt => return Some(Action::PopupAccept),
            KeyCode::Esc => return Some(Action::PopupClose),
            _ => {}
        },
        Popup::None => {}
    }

    // keys that work in every focus
    match key.code {
        KeyCode::Char(c) if (alt || ctrl) && digit(c).is_some() => return Some(Action::SelectTab(digit(c)?)),
        // Ctrl+B = move running subagents to the background (inside tmux press it twice); blueprint is Ctrl+G
        KeyCode::Char('b') if ctrl => return Some(Action::Background),
        KeyCode::Char('p') if ctrl => return Some(Action::OpenMenu),
        KeyCode::Char('g') if ctrl => return Some(Action::Blueprint),
        KeyCode::Char('u') if ctrl => return Some(Action::Usage),
        KeyCode::Left if ctx.usage => return Some(Action::Year(-1)),
        KeyCode::Right if ctx.usage => return Some(Action::Year(1)),
        KeyCode::Up if ctrl => return Some(Action::FocusNext),
        _ => {}
    }

    if ctx.focus != Focus::Input {
        let pane = if ctx.focus == Focus::Chat { Pane::Chat } else { Pane::Files };
        return match key.code {
            KeyCode::Up | KeyCode::Char('k') => Some(Action::Scroll(pane, -1)),
            KeyCode::Down | KeyCode::Char('j') => Some(Action::Scroll(pane, 1)),
            KeyCode::PageUp => Some(Action::Scroll(pane, -10)),
            KeyCode::PageDown => Some(Action::Scroll(pane, 10)),
            KeyCode::Home | KeyCode::Char('g') => Some(Action::ScrollEdge(pane, true)),
            KeyCode::End | KeyCode::Char('G') => Some(Action::ScrollEdge(pane, false)),
            KeyCode::Esc | KeyCode::Char('i') => Some(Action::FocusInput),
            KeyCode::Tab => Some(Action::FocusNext),
            KeyCode::Char('b') if alt => Some(Action::Blueprint),
            KeyCode::Char('x') if ctrl => Some(Action::StopAll),
            _ => None,
        };
    }

    match key.code {
        KeyCode::Char('b') if alt => Some(Action::Blueprint),
        KeyCode::Char('s') if alt => Some(Action::NextSkill),
        KeyCode::Char('x') if ctrl => Some(Action::StopAll),
        KeyCode::Char('o') if ctrl => Some(Action::ToggleDetail),
        KeyCode::Enter if shift || alt => Some(Action::Send),
        KeyCode::Esc if !ctx.in_chat => Some(Action::BackToChat),
        KeyCode::Esc => Some(Action::StopAsk),
        KeyCode::BackTab => Some(Action::ToggleAgent),
        KeyCode::Tab if !shift => Some(Action::ToggleAgent),
        KeyCode::PageUp if shift => Some(Action::ScrollFiles(-5)),
        KeyCode::PageDown if shift => Some(Action::ScrollFiles(5)),
        KeyCode::PageUp => Some(Action::ScrollChat(-10)),
        KeyCode::PageDown => Some(Action::ScrollChat(10)),
        _ => Some(Action::Input(key)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::Reply;

    fn k(code: KeyCode, m: KeyModifiers) -> KeyEvent {
        KeyEvent::new(code, m)
    }
    const CHAT: KeyContext = KeyContext { modal: Modal::None, in_chat: true, focus: Focus::Input, popup: Popup::None, usage: false };

    #[test]
    fn tabs_never_type() {
        assert_eq!(map_key(k(KeyCode::Char('3'), KeyModifiers::ALT), &CHAT), Some(Action::SelectTab(3)));
        assert_eq!(map_key(k(KeyCode::Char('0'), KeyModifiers::CONTROL), &CHAT), Some(Action::SelectTab(0)));
        // plain digits and shifted symbols are text
        assert!(matches!(map_key(k(KeyCode::Char('3'), KeyModifiers::NONE), &CHAT), Some(Action::Input(_))));
        assert!(matches!(map_key(k(KeyCode::Char('!'), KeyModifiers::SHIFT), &CHAT), Some(Action::Input(_))));
    }

    #[test]
    fn enter_and_esc() {
        assert!(matches!(map_key(k(KeyCode::Enter, KeyModifiers::NONE), &CHAT), Some(Action::Input(_))));
        assert_eq!(map_key(k(KeyCode::Enter, KeyModifiers::SHIFT), &CHAT), Some(Action::Send));
        assert_eq!(map_key(k(KeyCode::Enter, KeyModifiers::ALT), &CHAT), Some(Action::Send));
        assert_eq!(map_key(k(KeyCode::Esc, KeyModifiers::NONE), &CHAT), Some(Action::StopAsk));
        let blueprint = KeyContext { in_chat: false, ..CHAT };
        assert_eq!(map_key(k(KeyCode::Esc, KeyModifiers::NONE), &blueprint), Some(Action::BackToChat));
    }

    #[test]
    fn modals_capture_keys() {
        let confirm = KeyContext { modal: Modal::Confirm, ..CHAT };
        assert_eq!(map_key(k(KeyCode::Char('y'), KeyModifiers::NONE), &confirm), Some(Action::Confirm(true)));
        assert_eq!(map_key(k(KeyCode::Char('3'), KeyModifiers::ALT), &confirm), None);
        let perm = KeyContext { modal: Modal::Permission, ..CHAT };
        assert_eq!(map_key(k(KeyCode::Char('a'), KeyModifiers::NONE), &perm), Some(Action::Permission(Reply::Always)));
    }

    #[test]
    fn focus_popups_and_globals() {
        let chat = KeyContext { focus: Focus::Chat, ..CHAT };
        assert_eq!(map_key(k(KeyCode::Char('j'), KeyModifiers::NONE), &chat), Some(Action::Scroll(Pane::Chat, 1)));
        assert_eq!(map_key(k(KeyCode::Esc, KeyModifiers::NONE), &chat), Some(Action::FocusInput));
        assert_eq!(map_key(k(KeyCode::Char('x'), KeyModifiers::NONE), &chat), None); // no typing while focused
        assert_eq!(map_key(k(KeyCode::Up, KeyModifiers::CONTROL), &CHAT), Some(Action::FocusNext));
        assert_eq!(map_key(k(KeyCode::Char('b'), KeyModifiers::CONTROL), &CHAT), Some(Action::Background));
        assert_eq!(map_key(k(KeyCode::Char('p'), KeyModifiers::CONTROL), &CHAT), Some(Action::OpenMenu));
        assert_eq!(map_key(k(KeyCode::Char('g'), KeyModifiers::CONTROL), &CHAT), Some(Action::Blueprint));
        let complete = KeyContext { popup: Popup::Complete, ..CHAT };
        assert_eq!(map_key(k(KeyCode::Tab, KeyModifiers::NONE), &complete), Some(Action::PopupComplete));
        assert!(matches!(map_key(k(KeyCode::Char('d'), KeyModifiers::NONE), &complete), Some(Action::Input(_))));
        let menu = KeyContext { popup: Popup::Menu, ..CHAT };
        assert!(matches!(map_key(k(KeyCode::Char('m'), KeyModifiers::NONE), &menu), Some(Action::MenuInput(_))));
        assert_eq!(map_key(k(KeyCode::Enter, KeyModifiers::NONE), &menu), Some(Action::PopupAccept));
    }
}
