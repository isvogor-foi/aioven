//! T13 Completion: '/' or '\' at the start of the input lists skills and commands (pure).

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entry {
    pub name: String,
    pub description: String,
    pub skill: bool,
}

/// The query after a leading '/' or '\', while the user is still typing the name (no space yet).
pub fn trigger(input: &str) -> Option<&str> {
    let rest = input.strip_prefix('/').or_else(|| input.strip_prefix('\\'))?;
    (!rest.contains(char::is_whitespace)).then_some(rest)
}

/// Prefix matches first, then substring matches; skills before commands within each group.
pub fn matches<'a>(entries: &'a [Entry], query: &str) -> Vec<&'a Entry> {
    let q = query.to_lowercase();
    let mut prefix: Vec<&Entry> = entries.iter().filter(|e| e.name.to_lowercase().starts_with(&q)).collect();
    let mut inner: Vec<&Entry> =
        entries.iter().filter(|e| !e.name.to_lowercase().starts_with(&q) && e.name.to_lowercase().contains(&q)).collect();
    let order = |a: &&Entry, b: &&Entry| (!a.skill, &a.name).cmp(&(!b.skill, &b.name));
    prefix.sort_by(order);
    inner.sort_by(order);
    prefix.extend(inner);
    prefix
}

/// Split "/name rest of text" into (name, arguments).
pub fn split(input: &str) -> Option<(&str, &str)> {
    let rest = input.strip_prefix('/').or_else(|| input.strip_prefix('\\'))?;
    let (name, args) = rest.split_once(char::is_whitespace).unwrap_or((rest, ""));
    (!name.is_empty()).then_some((name, args.trim()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn e(name: &str, skill: bool) -> Entry {
        Entry { name: name.into(), description: String::new(), skill }
    }

    #[test]
    fn trigger_on_slash_or_backslash_until_space() {
        assert_eq!(trigger("/pd"), Some("pd"));
        assert_eq!(trigger("\\x"), Some("x"));
        assert_eq!(trigger("/"), Some(""));
        assert_eq!(trigger("/pdf now"), None);
        assert_eq!(trigger("hello /pdf"), None);
    }

    #[test]
    fn prefix_first_skills_first() {
        let list = [e("review", false), e("pdf", true), e("pptx", true), e("deep-research", true), e("prompt", false)];
        let names: Vec<&str> = matches(&list, "p").iter().map(|e| e.name.as_str()).collect();
        assert_eq!(names, ["pdf", "pptx", "prompt", "deep-research"]);
        assert_eq!(matches(&list, "").len(), 5);
    }

    #[test]
    fn splits_name_and_arguments() {
        assert_eq!(split("/pdf merge a.pdf b.pdf"), Some(("pdf", "merge a.pdf b.pdf")));
        assert_eq!(split("\\review"), Some(("review", "")));
        assert_eq!(split("/"), None);
        assert_eq!(split("text"), None);
    }
}
