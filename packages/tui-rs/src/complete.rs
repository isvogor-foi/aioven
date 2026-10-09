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

/// P3: the "@query" being typed at the end of the input (after whitespace or at the start).
pub fn trigger_file(input: &str) -> Option<&str> {
    let token = input.rsplit(char::is_whitespace).next()?;
    token.strip_prefix('@')
}

/// P3: every "@path" mentioned in the text (trailing punctuation dropped).
pub fn mentions(input: &str) -> Vec<&str> {
    let mut out: Vec<&str> = input
        .split_whitespace()
        .filter_map(|t| t.strip_prefix('@'))
        .map(|t| t.trim_end_matches(|c: char| ",.;:!?)\"'".contains(c)))
        .filter(|t| !t.is_empty())
        .collect();
    out.dedup();
    out
}

/// P3: mime type the server expects for a file part (text files are read by the server's read tool).
pub fn mime(path: &str) -> &'static str {
    let ext = path.rsplit('.').next().unwrap_or("").to_lowercase();
    match ext.as_str() {
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "webp" => "image/webp",
        "pdf" => "application/pdf",
        _ => "text/plain",
    }
}

/// P3: replace the "@query" at the end of the input with "@path ".
pub fn insert_file(input: &str, path: &str) -> String {
    let token = input.rsplit(char::is_whitespace).next().unwrap_or("");
    format!("{}@{path} ", &input[..input.len() - token.len()])
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
    fn file_mentions() {
        assert_eq!(trigger_file("look at @src/ma"), Some("src/ma"));
        assert_eq!(trigger_file("@"), Some(""));
        assert_eq!(trigger_file("look at @src/main.rs now"), None);
        assert_eq!(trigger_file("mail a@b"), None);
        assert_eq!(mentions("see @a.rs, and @img.png."), ["a.rs", "img.png"]);
        assert_eq!((mime("x.PNG"), mime("y.rs")), ("image/png", "text/plain"));
        assert_eq!(insert_file("fix @sr", "src/main.rs"), "fix @src/main.rs ");
    }

    #[test]
    fn splits_name_and_arguments() {
        assert_eq!(split("/pdf merge a.pdf b.pdf"), Some(("pdf", "merge a.pdf b.pdf")));
        assert_eq!(split("\\review"), Some(("review", "")));
        assert_eq!(split("/"), None);
        assert_eq!(split("text"), None);
    }
}
