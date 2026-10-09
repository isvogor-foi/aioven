//! T14 Menu: Ctrl+P settings/command popup items and filtering (pure).

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Item {
    Model { provider: String, model: String },
    Agent(String),
    Detail(bool),
    Blueprint,
    StopAll,
    Background,
    Connect,
    Usage,
    Sessions,
    Terse(String),
    TierModel { tier: String, provider: String, model: String },
    Quit,
}

pub struct Context<'a> {
    pub agent: &'a str,
    pub compact: bool,
    /// (provider, model, label) of connected providers
    pub models: &'a [(String, String, String)],
    pub current_model: Option<&'a (String, String)>,
    /// current caveman level (aioven.terse; default ultra)
    pub terse: &'a str,
    /// current tier models "provider/model" by tier name
    pub tiers: &'a [(String, String)],
}

pub fn items(ctx: &Context) -> Vec<(String, Item)> {
    let mut out = vec![
        (format!("Agent: bake{}", if ctx.agent == "bake" { "  ✓" } else { "" }), Item::Agent("bake".into())),
        (format!("Agent: recipe{}", if ctx.agent == "recipe" { "  ✓" } else { "" }), Item::Agent("recipe".into())),
        (
            if ctx.compact { "Transcript: show full detail".to_string() } else { "Transcript: compact".to_string() },
            Item::Detail(!ctx.compact),
        ),
        ("Open blueprint".into(), Item::Blueprint),
        ("Sessions…".into(), Item::Sessions),
        ("Connect provider…".into(), Item::Connect),
        ("Usage stats (heat map)".into(), Item::Usage),
        ("Move running tasks to background".into(), Item::Background),
        ("Stop all agents".into(), Item::StopAll),
        ("Quit".into(), Item::Quit),
    ];
    for level in ["ultra", "full", "lite", "off"] {
        let mark = if ctx.terse == level { "  ✓" } else { "" };
        out.push((format!("Caveman: {level}{mark}"), Item::Terse(level.into())));
    }
    for (provider, model, label) in ctx.models {
        for tier in ["small", "medium", "large"] {
            let current = ctx.tiers.iter().any(|(t, m)| t == tier && *m == format!("{provider}/{model}"));
            out.push((
                format!("Tier {tier}: {label}{}", if current { "  ✓" } else { "" }),
                Item::TierModel { tier: tier.into(), provider: provider.clone(), model: model.clone() },
            ));
        }
    }
    for (provider, model, label) in ctx.models {
        let current = ctx.current_model.is_some_and(|(p, m)| p == provider && m == model);
        out.push((
            format!("Model: {label}{}", if current { "  ✓" } else { "" }),
            Item::Model { provider: provider.clone(), model: model.clone() },
        ));
    }
    out
}

/// Indexes of items whose label contains every word of the query (case-insensitive).
pub fn filter(items: &[(String, Item)], query: &str) -> Vec<usize> {
    let words: Vec<String> = query.split_whitespace().map(str::to_lowercase).collect();
    items
        .iter()
        .enumerate()
        .filter(|(_, (label, _))| {
            let l = label.to_lowercase();
            words.iter().all(|w| l.contains(w))
        })
        .map(|(i, _)| i)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn items_mark_current_and_filter() {
        let models = vec![("gh".to_string(), "gpt-5-mini".to_string(), "GPT-5 mini · Copilot".to_string())];
        let current = ("gh".to_string(), "gpt-5-mini".to_string());
        let tiers = vec![("small".to_string(), "gh/gpt-5-mini".to_string())];
        let ctx = Context { agent: "recipe", compact: true, models: &models, current_model: Some(&current), terse: "ultra", tiers: &tiers };
        let list = items(&ctx);
        assert!(list.iter().any(|(l, _)| l == "Agent: recipe  ✓"));
        assert!(list.iter().any(|(l, _)| l == "Model: GPT-5 mini · Copilot  ✓"));
        let hits = filter(&list, "model mini");
        assert_eq!(hits.len(), 1);
        assert_eq!(list[hits[0]].1, Item::Model { provider: "gh".into(), model: "gpt-5-mini".into() });
        assert_eq!(filter(&list, "").len(), list.len());
        assert!(list.iter().any(|(l, i)| l == "Caveman: ultra  ✓" && *i == Item::Terse("ultra".into())));
        assert!(list.iter().any(|(l, _)| l == "Tier small: GPT-5 mini · Copilot  ✓"));
        assert_eq!(filter(&list, "tier large").len(), 1);
    }
}
