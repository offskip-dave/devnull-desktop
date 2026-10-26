use std::collections::HashMap;
use std::sync::OnceLock;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Suggestion {
    pub name: String,
    pub emoji: String,
}

fn shortcodes() -> &'static HashMap<String, String> {
    static MAP: OnceLock<HashMap<String, String>> = OnceLock::new();
    MAP.get_or_init(|| {
        serde_json::from_str(include_str!("emoji-shortcodes.json")).unwrap_or_default()
    })
}

fn is_shortcode_char(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '_' || c == '+' || c == '-'
}

fn is_trigger_prefix(c: char) -> bool {
    c.is_alphanumeric() || c == '_' || c == ':'
}

/// The `:shortcode` being typed before the caret, from two characters on so times like 10:30 stay quiet.
pub fn trigger_at(before: &str) -> Option<(usize, String)> {
    let chars: Vec<(usize, char)> = before.char_indices().collect();
    let mut i = chars.len();
    let mut name_len = 0usize;
    while i > 0 {
        let (_, c) = chars[i - 1];
        if is_shortcode_char(c) {
            name_len += 1;
            i -= 1;
        } else {
            break;
        }
    }
    if name_len < 2 || i == 0 {
        return None;
    }
    let (colon_idx, colon) = chars[i - 1];
    if colon != ':' {
        return None;
    }
    if i >= 2 && is_trigger_prefix(chars[i - 2].1) {
        return None;
    }
    let query: String = chars[i..].iter().map(|(_, c)| *c).collect();
    Some((before[..colon_idx].chars().count(), query))
}

pub fn search(query: &str, limit: usize) -> Vec<Suggestion> {
    let partial = query.to_ascii_lowercase();
    if partial.is_empty() {
        return Vec::new();
    }
    let rank = |name: &str| -> u8 {
        if name.starts_with(&partial) {
            0
        } else if name
            .split(['_', '-'])
            .any(|word| word.starts_with(&partial))
        {
            1
        } else {
            2
        }
    };
    let mut hits: Vec<(&str, &str, u8)> = shortcodes()
        .iter()
        .filter(|(name, _)| name.contains(&partial))
        .map(|(name, emoji)| (name.as_str(), emoji.as_str(), rank(name)))
        .collect();
    hits.sort_by(|left, right| {
        left.2
            .cmp(&right.2)
            .then_with(|| left.0.len().cmp(&right.0.len()))
            .then_with(|| left.0.cmp(right.0))
    });
    hits.into_iter()
        .take(limit)
        .map(|(name, emoji, _)| Suggestion {
            name: name.to_owned(),
            emoji: emoji.to_owned(),
        })
        .collect()
}

pub fn emojify(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(start) = rest.find(':') {
        out.push_str(&rest[..start]);
        let after = &rest[start + 1..];
        if let Some(end) = after.find(':') {
            let name = &after[..end];
            if let Some(emoji) = shortcodes().get(name) {
                out.push_str(emoji);
                rest = &after[end + 1..];
                continue;
            }
        }
        out.push(':');
        rest = after;
    }
    out.push_str(rest);
    out
}

pub fn emojify_input(text: &str, caret: usize) -> (String, usize) {
    let prefix: String = text.chars().take(caret).collect();
    let emojified = emojify(text);
    let caret = emojify(&prefix).chars().count();
    (emojified, caret)
}

pub fn match_index(text: &str, query: &str) -> Option<usize> {
    if query.is_empty() {
        return None;
    }
    let lower = text.to_ascii_lowercase();
    let needle = query.to_ascii_lowercase();
    let mut from = 0;
    while let Some(at) = lower[from..].find(&needle) {
        let at = from + at;
        if at == 0 {
            return Some(at);
        }
        let prev = lower[..at].chars().next_back()?;
        if !prev.is_alphanumeric() {
            return Some(at);
        }
        from = at + 1;
    }
    lower.find(&needle)
}

pub fn highlighted_shortcode(name: &str, query: &str) -> String {
    let escape = |text: &str| {
        text.replace('&', "&amp;")
            .replace('<', "&lt;")
            .replace('>', "&gt;")
    };
    if let Some(index) = match_index(name, query) {
        let end = index + query.chars().count().min(name[index..].chars().count());
        let (head, rest) = name.split_at(index);
        let (hit, tail) = rest.split_at(end - index);
        format!(":{}<b>{}</b>{}:", escape(head), escape(hit), escape(tail))
    } else {
        format!(":{}:", escape(name))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn search_prefers_prefix_then_shortest() {
        let names: Vec<_> = search("smi", 8).into_iter().map(|s| s.name).collect();
        assert_eq!(&names[..3], ["smile", "smirk", "smiley"]);
        assert!(names.iter().all(|name| name.starts_with("smi")));
    }

    #[test]
    fn search_falls_back_to_word_starts() {
        let names: Vec<_> = search("ART", 8).into_iter().map(|s| s.name).collect();
        assert_eq!(&names[..2], ["art", "artist"]);
        assert!(names.iter().any(|n| n == "man_artist"));
        assert!(names.iter().any(|n| n == "performing_arts"));
    }

    #[test]
    fn search_lists_contains_last() {
        let names: Vec<_> = search("apple", 8).into_iter().map(|s| s.name).collect();
        assert_eq!(names, ["apple", "green_apple", "pineapple"]);
    }

    #[test]
    fn search_pairs_emoji_and_limits() {
        assert_eq!(search("a", 5).len(), 5);
        assert_eq!(
            search("+1", 1),
            vec![Suggestion {
                name: "+1".into(),
                emoji: "👍".into(),
            }]
        );
        assert!(search("no_such_shortcode", 8).is_empty());
    }

    #[test]
    fn emojify_replaces_closed_shortcodes() {
        assert_eq!(
            emojify("shipped :rocket:, :heart::like: and :fire:s"),
            "shipped 🚀, ❤️👍 and 🔥s"
        );
        assert_eq!(
            emojify("plan :a, then :smile and :heart"),
            "plan :a, then :smile and :heart"
        );
    }

    #[test]
    fn emojify_input_keeps_open_shortcodes() {
        assert_eq!(emojify_input(":smile", 6), (":smile".into(), 6));
        assert_eq!(emojify_input("so :smile: ok", 10), ("so 😄 ok".into(), 4));
    }

    #[test]
    fn trigger_needs_two_chars_and_a_break() {
        assert_eq!(trigger_at(":s"), None);
        assert_eq!(trigger_at(":sm"), Some((0, "sm".into())));
        assert_eq!(trigger_at("10:30"), None);
        assert_eq!(trigger_at("hi :smile"), Some((3, "smile".into())));
    }
}
