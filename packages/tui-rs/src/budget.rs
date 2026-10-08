//! R11 Budgets: Rust copy of `AIOvenDefaults.AGENTS` with `/config` → `aioven.agents` overrides.

use serde_json::Value;

const AGENTS: [(&str, &str, f64); 8] = [
    ("bake", "medium", 200_000.0),
    ("recipe", "medium", 200_000.0),
    ("pantry", "small", 50_000.0),
    ("taster", "medium", 80_000.0),
    ("thermometer", "small", 40_000.0),
    ("cookbook", "small", 50_000.0),
    ("title", "small", 10_000.0),
    ("summary", "small", 20_000.0),
];

fn configured<'a>(config: &'a Value, agent: &str, key: &str) -> Option<&'a Value> {
    config.get("aioven")?.get("agents")?.get(agent)?.get(key)
}

pub fn budget(config: &Value, agent: &str, subagent: bool) -> f64 {
    configured(config, agent, "budget")
        .and_then(Value::as_f64)
        .or_else(|| AGENTS.iter().find(|(n, _, _)| *n == agent).map(|(_, _, b)| *b))
        .unwrap_or(if subagent { 50_000.0 } else { 200_000.0 })
}

pub fn tier(config: &Value, agent: &str) -> Option<String> {
    configured(config, agent, "tier")
        .and_then(Value::as_str)
        .map(str::to_string)
        .or_else(|| AGENTS.iter().find(|(n, _, _)| *n == agent).map(|(_, t, _)| t.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn defaults_and_overrides() {
        let none = json!({});
        assert_eq!(budget(&none, "taster", true), 80_000.0);
        assert_eq!(budget(&none, "custom", true), 50_000.0);
        assert_eq!(tier(&none, "pantry").as_deref(), Some("small"));
        let cfg = json!({"aioven":{"agents":{"bake":{"budget":5,"tier":"large"}}}});
        assert_eq!(budget(&cfg, "bake", false), 5.0);
        assert_eq!(tier(&cfg, "bake").as_deref(), Some("large"));
    }

    #[test]
    fn matches_typescript_defaults() {
        let ts = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/../core/src/aioven.ts")).unwrap();
        for (name, tier, budget) in AGENTS {
            let b = format!("{}", budget as i64);
            let b = format!("{}_{}", &b[..b.len() - 3], &b[b.len() - 3..]);
            let key = if name.contains('-') { format!("\"{name}\"") } else { name.to_string() };
            assert!(ts.contains(&format!("{key}: {{ tier: \"{tier}\", budget: {b} }}")), "{name} differs from aioven.ts");
        }
    }
}
