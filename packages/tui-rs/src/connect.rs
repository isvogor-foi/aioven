//! T19 Connect: provider login flow as a pure state machine (pick → method → key | oauth).

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProviderChoice {
    pub id: String,
    pub name: String,
    pub connected: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Method {
    pub oauth: bool,
    pub label: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Step {
    Pick { query: String, sel: usize },
    Method { provider: ProviderChoice, methods: Vec<Method>, sel: usize },
    Key { provider: ProviderChoice, key: String },
    Oauth { provider: ProviderChoice, method: usize, url: String, auto: bool, instructions: String, code: String },
    Busy(String),
}

/// Side effects the app performs for a transition.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Effect {
    None,
    Authorize { provider: String, method: usize },
    Callback { provider: String, method: usize, code: Option<String> },
    SetKey { provider: String, key: String },
}

pub enum Input {
    Char(char),
    Backspace,
    Move(i8),
    Enter,
}

pub fn filter<'a>(providers: &'a [ProviderChoice], query: &str) -> Vec<&'a ProviderChoice> {
    let q = query.to_lowercase();
    let mut list: Vec<&ProviderChoice> =
        providers.iter().filter(|p| p.name.to_lowercase().contains(&q) || p.id.to_lowercase().contains(&q)).collect();
    list.sort_by(|a, b| (!a.connected, a.name.to_lowercase()).cmp(&(!b.connected, b.name.to_lowercase())));
    list
}

fn pick_method(provider: ProviderChoice, methods: Vec<Method>, index: usize) -> (Step, Effect) {
    match methods.get(index) {
        Some(m) if m.oauth => (
            Step::Busy(format!("opening {} login…", provider.name)),
            Effect::Authorize { provider: provider.id, method: index },
        ),
        _ => (Step::Key { provider, key: String::new() }, Effect::None),
    }
}

/// One user input → next step + effect. `methods_of` gives the auth methods of a provider (empty = API key).
pub fn next(step: Step, input: Input, providers: &[ProviderChoice], methods_of: impl Fn(&str) -> Vec<Method>) -> (Step, Effect) {
    match (step, input) {
        (Step::Pick { mut query, .. }, Input::Char(c)) => {
            query.push(c);
            (Step::Pick { query, sel: 0 }, Effect::None)
        }
        (Step::Pick { mut query, .. }, Input::Backspace) => {
            query.pop();
            (Step::Pick { query, sel: 0 }, Effect::None)
        }
        (Step::Pick { query, sel }, Input::Move(d)) => {
            let n = filter(providers, &query).len().max(1) as i64;
            (Step::Pick { query, sel: (sel as i64 + d as i64).rem_euclid(n) as usize }, Effect::None)
        }
        (Step::Pick { query, sel }, Input::Enter) => {
            let Some(p) = filter(providers, &query).get(sel).map(|p| (*p).clone()) else {
                return (Step::Pick { query, sel }, Effect::None);
            };
            let methods = methods_of(&p.id);
            if methods.len() > 1 {
                (Step::Method { provider: p, methods, sel: 0 }, Effect::None)
            } else {
                pick_method(p, methods, 0)
            }
        }
        (Step::Method { provider, methods, sel }, Input::Move(d)) => {
            let n = methods.len().max(1) as i64;
            (Step::Method { provider, methods, sel: (sel as i64 + d as i64).rem_euclid(n) as usize }, Effect::None)
        }
        (Step::Method { provider, methods, sel }, Input::Enter) => pick_method(provider, methods, sel),
        (Step::Key { provider, mut key }, Input::Char(c)) => {
            key.push(c);
            (Step::Key { provider, key }, Effect::None)
        }
        (Step::Key { provider, mut key }, Input::Backspace) => {
            key.pop();
            (Step::Key { provider, key }, Effect::None)
        }
        (Step::Key { provider, key }, Input::Enter) if !key.trim().is_empty() => (
            Step::Busy(format!("saving {} key…", provider.name)),
            Effect::SetKey { provider: provider.id, key: key.trim().to_string() },
        ),
        (Step::Oauth { provider, method, url, auto, instructions, mut code }, Input::Char(c)) if !auto => {
            code.push(c);
            (Step::Oauth { provider, method, url, auto, instructions, code }, Effect::None)
        }
        (Step::Oauth { provider, method, url, auto, instructions, mut code }, Input::Backspace) if !auto => {
            code.pop();
            (Step::Oauth { provider, method, url, auto, instructions, code }, Effect::None)
        }
        (Step::Oauth { provider, method, auto, code, .. }, Input::Enter) if auto || !code.trim().is_empty() => (
            Step::Busy(format!("finishing {} login…", provider.name)),
            Effect::Callback { provider: provider.id, method, code: (!auto).then(|| code.trim().to_string()) },
        ),
        (step, _) => (step, Effect::None),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn providers() -> Vec<ProviderChoice> {
        vec![
            ProviderChoice { id: "google".into(), name: "Google".into(), connected: true },
            ProviderChoice { id: "github-copilot".into(), name: "GitHub Copilot".into(), connected: false },
            ProviderChoice { id: "openrouter".into(), name: "OpenRouter".into(), connected: false },
        ]
    }
    fn methods(id: &str) -> Vec<Method> {
        match id {
            "github-copilot" => vec![Method { oauth: true, label: "Login with GitHub".into() }],
            "openrouter" => vec![],
            _ => vec![Method { oauth: true, label: "OAuth".into() }, Method { oauth: false, label: "API key".into() }],
        }
    }

    #[test]
    fn connected_first_then_filter() {
        let p = providers();
        assert_eq!(filter(&p, "")[0].id, "google");
        assert_eq!(filter(&p, "router").len(), 1);
    }

    #[test]
    fn api_key_flow() {
        let p = providers();
        let s = Step::Pick { query: "open".into(), sel: 0 };
        let (s, e) = next(s, Input::Enter, &p, methods);
        assert!(matches!(s, Step::Key { .. }) && e == Effect::None);
        let (s, _) = next(s, Input::Char('k'), &p, methods);
        let (_, e) = next(s, Input::Enter, &p, methods);
        assert_eq!(e, Effect::SetKey { provider: "openrouter".into(), key: "k".into() });
    }

    #[test]
    fn oauth_and_method_choice() {
        let p = providers();
        let (_, e) = next(Step::Pick { query: "copilot".into(), sel: 0 }, Input::Enter, &p, methods);
        assert_eq!(e, Effect::Authorize { provider: "github-copilot".into(), method: 0 });
        let (s, _) = next(Step::Pick { query: "goo".into(), sel: 0 }, Input::Enter, &p, methods);
        assert!(matches!(s, Step::Method { .. }));
        let (s, _) = next(s, Input::Move(1), &p, methods);
        let (s, _) = next(s, Input::Enter, &p, methods);
        assert!(matches!(s, Step::Key { .. }));
        let code = Step::Oauth { provider: p[1].clone(), method: 0, url: "u".into(), auto: false, instructions: String::new(), code: "12".into() };
        let (_, e) = next(code, Input::Enter, &p, methods);
        assert_eq!(e, Effect::Callback { provider: "github-copilot".into(), method: 0, code: Some("12".into()) });
    }
}
