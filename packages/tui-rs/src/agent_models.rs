//! T29 Agents & models: choice list for one agent and the config patch for a choice (pure).

use serde_json::{Value, json};

use crate::types::AgentModel;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Choice {
    /// recommended size, with that size's model
    Recommended,
    /// one of small | medium | large
    Size(String),
    Model { provider: String, model: String },
}

pub const SIZES: [&str; 3] = ["small", "medium", "large"];

fn short(model: &str) -> &str {
    model.rsplit('/').next().unwrap_or(model)
}

/// Rows for `agent`: ★ recommended, the three sizes, then every connected model (tagged with its size).
/// `tiers` = (size, "provider/model") from `aioven.tiers`; `models` = (provider, model, label).
pub fn choices(agent: &AgentModel, tiers: &[(String, String)], models: &[(String, String, String)]) -> Vec<(String, Choice)> {
    let tier_model = |size: &str| tiers.iter().find(|(t, _)| t == size).map(|(_, m)| short(m).to_string());
    let own = agent.source == "agent";
    let mark = |on: bool| if on { "  ✓" } else { "" };
    let mut out = vec![(
        format!(
            "★ Recommended: {} → {}{}",
            agent.recommended,
            tier_model(&agent.recommended).unwrap_or_else(|| "default model".into()),
            mark(!own && agent.tier == agent.recommended)
        ),
        Choice::Recommended,
    )];
    for size in SIZES.iter().filter(|s| **s != agent.recommended) {
        out.push((
            format!("Size {size} → {}{}", tier_model(size).unwrap_or_else(|| "not set".into()), mark(!own && agent.tier == *size)),
            Choice::Size(size.to_string()),
        ));
    }
    for (provider, model, label) in models {
        let id = format!("{provider}/{model}");
        let tag = tiers.iter().find(|(_, m)| *m == id).map(|(t, _)| format!(" [{}]", &t[..1].to_uppercase())).unwrap_or_default();
        out.push((
            format!("{label}{tag}{}", mark(own && agent.model.as_deref() == Some(id.as_str()))),
            Choice::Model { provider: provider.clone(), model: model.clone() },
        ));
    }
    out
}

/// Global-config patch that applies `choice` to `agent` (an empty model means "use the size's model").
pub fn patch(agent: &AgentModel, choice: &Choice) -> Value {
    let setting = match choice {
        Choice::Recommended => json!({ "tier": agent.recommended, "model": "" }),
        Choice::Size(size) => json!({ "tier": size, "model": "" }),
        Choice::Model { provider, model } => json!({ "model": format!("{provider}/{model}") }),
    };
    json!({ "aioven": { "agents": { agent.name.clone(): setting } } })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pantry(source: &str, tier: &str, model: Option<&str>) -> AgentModel {
        AgentModel {
            name: "pantry".into(),
            mode: "subagent".into(),
            recommended: "small".into(),
            tier: tier.into(),
            model: model.map(str::to_string),
            source: source.into(),
        }
    }

    #[test]
    fn rows_mark_the_current_choice() {
        let tiers = vec![("small".to_string(), "gh/mini".to_string())];
        let models = vec![("gh".to_string(), "mini".to_string(), "Mini · Copilot".to_string()), ("gh".to_string(), "opus".to_string(), "Opus · Copilot".to_string())];
        let rows = choices(&pantry("tier", "small", Some("gh/mini")), &tiers, &models);
        let labels: Vec<&str> = rows.iter().map(|(l, _)| l.as_str()).collect();
        assert_eq!(labels, ["★ Recommended: small → mini  ✓", "Size medium → not set", "Size large → not set", "Mini · Copilot [S]", "Opus · Copilot"]);
        let rows = choices(&pantry("agent", "small", Some("gh/opus")), &tiers, &models);
        assert_eq!(rows[4].0, "Opus · Copilot  ✓");
        assert!(!rows[0].0.ends_with('✓'));
    }

    #[test]
    fn patches() {
        let a = pantry("tier", "small", None);
        assert_eq!(patch(&a, &Choice::Recommended), json!({ "aioven": { "agents": { "pantry": { "tier": "small", "model": "" } } } }));
        assert_eq!(patch(&a, &Choice::Size("large".into()))["aioven"]["agents"]["pantry"]["tier"], "large");
        assert_eq!(
            patch(&a, &Choice::Model { provider: "gh".into(), model: "opus".into() }),
            json!({ "aioven": { "agents": { "pantry": { "model": "gh/opus" } } } })
        );
    }
}
