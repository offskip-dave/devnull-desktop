use pulldown_cmark::{Event, Options, Parser, Tag, TagEnd};
use relm4::gtk;
use relm4::gtk::glib;
use relm4::gtk::pango;
use relm4::gtk::prelude::*;
use relm4::Sender;
use std::collections::HashMap;
use std::sync::OnceLock;
use syntect::easy::HighlightLines;
use syntect::highlighting::{FontStyle, ThemeSet};
use syntect::parsing::SyntaxSet;
use syntect::util::LinesWithEndings;

use crate::app::AppMsg;
use crate::ui;
use devnull_api::Commit;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Segment {
    Text(String),
    Code(String),
    Block {
        language: Option<String>,
        filename: Option<String>,
        code: String,
    },
}

#[derive(Debug, Clone, Copy, Default)]
pub struct CapturedMentions<'a> {
    pub users: Option<&'a HashMap<String, String>>,
    pub organizations: Option<&'a HashMap<String, String>>,
}

impl<'a> CapturedMentions<'a> {
    pub fn from_commit(commit: &'a Commit) -> Self {
        Self {
            users: commit.mentions(),
            organizations: commit.organization_mentions(),
        }
    }

    pub fn users_only(users: Option<&'a HashMap<String, String>>) -> Self {
        Self {
            users,
            organizations: None,
        }
    }
}

pub fn body_widget(
    text: &str,
    class: &str,
    mentions: CapturedMentions<'_>,
    sender: Option<&Sender<AppMsg>>,
    pasteis_mode: bool,
) -> gtk::Widget {
    body_from(text, text, class, mentions, sender, pasteis_mode)
}

fn body_from(
    display: &str,
    source: &str,
    class: &str,
    mentions: CapturedMentions<'_>,
    sender: Option<&Sender<AppMsg>>,
    pasteis_mode: bool,
) -> gtk::Widget {
    let sender = sender.cloned();
    let wrap = gtk::Box::new(gtk::Orientation::Vertical, 4);
    for class in class.split_whitespace() {
        wrap.add_css_class(class);
    }
    wrap.set_hexpand(true);
    let mut prose = String::new();
    let flush = |prose: &mut String, wrap: &gtk::Box| {
        if prose.is_empty() {
            return;
        }
        wrap.append(&prose_widget(prose, sender.as_ref()));
        prose.clear();
    };
    let copy_blocks = block_sources(source);
    let mut block_i = 0;
    for segment in without_fade_ellipsis(segments(display), &copy_blocks) {
        match segment {
            Segment::Text(text) => {
                prose.push_str(&prose_markup(&text, mentions, pasteis_mode));
            }
            Segment::Code(code) => {
                let chip = if is_dark() { "#232326" } else { "#eef0f2" };
                prose.push_str(&format!(
                    "<span font_family=\"monospace\" background=\"{chip}\"> "
                ));
                prose.push_str(&escape(&code));
                prose.push_str(" </span>");
            }
            Segment::Block {
                language,
                filename,
                code,
            } => {
                flush(&mut prose, &wrap);
                let copy = copy_blocks
                    .get(block_i)
                    .map(String::as_str)
                    .unwrap_or(&code);
                let faded = copy != code;
                block_i += 1;
                wrap.append(&code_block(
                    language.as_deref(),
                    filename.as_deref(),
                    &code,
                    copy,
                    faded,
                ));
            }
        }
    }
    flush(&mut prose, &wrap);
    wrap.upcast()
}

pub fn body_markup(text: &str) -> String {
    render_markup(text, CapturedMentions::default(), false)
}

/// Lists show a longer body folded to its first characters and lines. Keep in sync with
/// `App\Models\Commit` and `App\Services\CommitBodies`.
pub const PREVIEW_LENGTH: usize = 280;
pub const PREVIEW_LINES: usize = 8;

pub fn folded_body_widget(
    text: &str,
    class: &str,
    mentions: CapturedMentions<'_>,
    sender: Option<&Sender<AppMsg>>,
    pasteis_mode: bool,
) -> gtk::Widget {
    let Some(preview) = excerpt(text) else {
        return body_widget(text, class, mentions, sender, pasteis_mode);
    };
    let wrap = gtk::Box::new(gtk::Orientation::Vertical, 2);
    wrap.set_hexpand(true);
    wrap.append(&body_from(
        &preview,
        text,
        class,
        mentions,
        sender,
        pasteis_mode,
    ));
    let more = gtk::Button::with_label("Show more");
    more.add_css_class("flat");
    more.add_css_class("dn-show-more");
    more.set_halign(gtk::Align::Start);
    more.set_has_frame(false);
    more.set_cursor_from_name(Some("pointer"));
    more.set_can_focus(true);
    let class = class.to_owned();
    let full = text.to_owned();
    let users = mentions.users.cloned();
    let organizations = mentions.organizations.cloned();
    let sender = sender.cloned();
    more.connect_clicked(move |btn| {
        let Some(wrap) = btn.parent().and_then(|p| p.downcast::<gtk::Box>().ok()) else {
            return;
        };
        while let Some(child) = wrap.first_child() {
            wrap.remove(&child);
        }
        wrap.append(&body_widget(
            &full,
            &class,
            CapturedMentions {
                users: users.as_ref(),
                organizations: organizations.as_ref(),
            },
            sender.as_ref(),
            pasteis_mode,
        ));
    });
    wrap.append(&more);
    wrap.upcast()
}

pub fn clamped_body_widget(
    text: &str,
    class: &str,
    mentions: CapturedMentions<'_>,
    sender: Option<&Sender<AppMsg>>,
    max_height: i32,
    pasteis_mode: bool,
) -> gtk::Widget {
    let preview = excerpt(text).unwrap_or_else(|| text.to_owned());
    let scroll = gtk::ScrolledWindow::new();
    scroll.add_css_class("dn-body-clamp");
    scroll.set_policy(gtk::PolicyType::Never, gtk::PolicyType::Never);
    scroll.set_max_content_height(max_height);
    scroll.set_propagate_natural_height(true);
    scroll.set_hexpand(true);
    scroll.set_overflow(gtk::Overflow::Hidden);
    scroll.set_child(Some(&body_widget(
        &preview,
        class,
        mentions,
        sender,
        pasteis_mode,
    )));
    scroll.upcast()
}

/// The start of a body too long to show whole in a list, or `None` when it fits.
pub fn excerpt(body: &str) -> Option<String> {
    let body = body.replace("\r\n", "\n").replace('\r', "\n");
    let mut limit = preview_limit(&body)?;
    let mut excerpt = String::new();
    for (index, part) in split_code_spans(&body).into_iter().enumerate() {
        let length = char_len(&part);
        if length <= limit {
            excerpt.push_str(&part);
            limit -= length;
            continue;
        }
        let is_first = excerpt.trim().is_empty();
        excerpt.push_str(&if index % 2 == 0 {
            cut_prose(&part, limit, is_first)
        } else if is_first {
            cut_code(&part, limit)
        } else {
            String::new()
        });
        break;
    }
    let excerpt = excerpt.trim_end().to_owned();
    Some(if excerpt.ends_with("```") {
        format!("{excerpt}\n…")
    } else {
        format!("{excerpt}…")
    })
}

fn preview_limit(body: &str) -> Option<usize> {
    let mut lines = body.split('\n');
    let mut first = Vec::new();
    for _ in 0..PREVIEW_LINES {
        match lines.next() {
            Some(line) => first.push(line),
            None => break,
        }
    }
    if lines.next().is_some() {
        return Some(PREVIEW_LENGTH.min(char_len(&first.join("\n"))));
    }
    let len = char_len(body);
    (len > PREVIEW_LENGTH).then_some(PREVIEW_LENGTH)
}

fn split_code_spans(text: &str) -> Vec<String> {
    let chars: Vec<char> = text.chars().collect();
    let mut i = 0;
    let mut parts = Vec::new();
    let mut prose = String::new();
    while i < chars.len() {
        if let Some(end) = take_code_span(&chars, i) {
            parts.push(std::mem::take(&mut prose));
            parts.push(chars[i..end].iter().collect());
            i = end;
            continue;
        }
        prose.push(chars[i]);
        i += 1;
    }
    parts.push(prose);
    parts
}

fn take_code_span(chars: &[char], i: usize) -> Option<usize> {
    take_fence(chars, i)
        .map(|(_, end)| end)
        .or_else(|| take_inline(chars, i).map(|(_, end)| end))
}

fn cut_prose(prose: &str, limit: usize, is_first: bool) -> String {
    let cut = char_take(prose, limit);
    if prose.chars().nth(limit).is_some_and(char::is_whitespace) {
        return cut;
    }
    if let Some((idx, _)) = cut.char_indices().rev().find(|(_, c)| c.is_whitespace()) {
        return cut[..idx].to_owned();
    }
    if is_first {
        cut
    } else {
        String::new()
    }
}

fn cut_code(code: &str, limit: usize) -> String {
    if !code.starts_with("```") {
        return format!("{}`", char_take(code, limit.saturating_sub(1).max(1)));
    }
    let mut lines = code.split('\n');
    let Some(header) = lines.next() else {
        return char_take(code, limit);
    };
    let fence: String = header.chars().take_while(|c| *c == '`').collect();
    let mut kept = vec![header.to_owned()];
    let mut rest = limit.saturating_sub(char_len(header) + char_len(&fence) + 1);
    let inner: Vec<&str> = lines.collect();
    let body_lines = inner.split_last().map(|(_, body)| body).unwrap_or(&[]);
    for line in body_lines.iter().copied() {
        if char_len(line) + 1 > rest {
            if kept.len() == 1 {
                kept.push(char_take(line, rest.saturating_sub(1).max(1)));
            }
            break;
        }
        kept.push(line.to_owned());
        rest = rest.saturating_sub(char_len(line) + 1);
    }
    kept.push(fence);
    kept.join("\n")
}

fn char_len(text: &str) -> usize {
    text.chars().count()
}

fn char_take(text: &str, n: usize) -> String {
    text.chars().take(n).collect()
}

fn prose_markup(text: &str, mentions: CapturedMentions<'_>, pasteis_mode: bool) -> String {
    markdown_markup(&crate::emoji::emojify(text), mentions, pasteis_mode)
}

fn render_markup(text: &str, mentions: CapturedMentions<'_>, pasteis_mode: bool) -> String {
    let mut out = String::new();
    for (index, segment) in segments(text).into_iter().enumerate() {
        if index > 0 && !out.ends_with('\n') {
            match segment {
                Segment::Block { .. } | Segment::Text(_) => out.push('\n'),
                Segment::Code(_) => {}
            }
        }
        match segment {
            Segment::Text(text) => out.push_str(&prose_markup(&text, mentions, pasteis_mode)),
            Segment::Code(code) => {
                out.push_str("<tt>");
                out.push_str(&escape(&code));
                out.push_str("</tt>");
            }
            Segment::Block { language, code, .. } => {
                out.push_str(&highlight_pango(&code, language.as_deref()));
            }
        }
    }
    out
}

/// A pastel word drawn as an icon, or the prose between those words. Escaped `\pastel`
/// keeps the word and drops the backslash. Keep in sync with `resources/js/lib/pasteis.ts`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PasteisPart {
    pub text: String,
    pub pastel: bool,
}

/// Splits text into pastel words and the prose between them (#1583).
pub fn pasteis_parts(text: &str) -> Vec<PasteisPart> {
    let chars: Vec<char> = text.chars().collect();
    let mut parts = Vec::new();
    let mut prose = String::new();
    let mut i = 0;
    while i < chars.len() {
        if let Some((escaped, word, end)) = take_pastel_word(&chars, i) {
            if escaped {
                prose.push_str(&word);
            } else {
                if !prose.is_empty() {
                    parts.push(PasteisPart {
                        text: std::mem::take(&mut prose),
                        pastel: false,
                    });
                }
                parts.push(PasteisPart {
                    text: word.to_owned(),
                    pastel: true,
                });
            }
            i = end;
            continue;
        }
        prose.push(chars[i]);
        i += 1;
    }
    if !prose.is_empty() {
        parts.push(PasteisPart {
            text: prose,
            pastel: false,
        });
    }
    parts
}

/// Match `(?<![\p{L}\p{M}\p{N}_\\])(\\?)(pastel|pasteis|pastéis)(?![\p{L}\p{M}\p{N}_])`.
fn take_pastel_word(chars: &[char], i: usize) -> Option<(bool, String, usize)> {
    let before = i.checked_sub(1).map(|j| chars[j]);
    if before.is_some_and(|c| is_pastel_boundary_char(c) || c == '\\') {
        return None;
    }
    let escaped = chars.get(i) == Some(&'\\');
    let word_at = if escaped { i + 1 } else { i };
    if word_at >= chars.len() {
        return None;
    }
    let word_len = pastel_word_len(&chars[word_at..])?;
    let end = word_at + word_len;
    if chars.get(end).copied().is_some_and(is_pastel_boundary_char) {
        return None;
    }
    let word: String = chars[word_at..end].iter().collect();
    Some((escaped, word, end))
}

fn pastel_word_len(chars: &[char]) -> Option<usize> {
    let lower: String = chars.iter().copied().flat_map(char::to_lowercase).collect();
    if lower.starts_with("pastéis") {
        Some("pastéis".chars().count())
    } else if lower.starts_with("pasteis") {
        Some(7)
    } else if lower.starts_with("pastel") {
        Some(6)
    } else {
        None
    }
}

fn is_pastel_boundary_char(c: char) -> bool {
    // JS `\p{L}\p{M}\p{N}_`
    c == '_' || c.is_alphanumeric() || unicode_general_category_mark(c)
}

fn unicode_general_category_mark(c: char) -> bool {
    matches!(
        c,
        '\u{0300}'..='\u{036F}'
            | '\u{1AB0}'..='\u{1AFF}'
            | '\u{1DC0}'..='\u{1DFF}'
            | '\u{20D0}'..='\u{20FF}'
            | '\u{FE20}'..='\u{FE2F}'
    )
}

fn is_dark() -> bool {
    gtk::is_initialized() && relm4::adw::StyleManager::default().is_dark()
}

pub fn with_wordmarks(text: &str) -> String {
    let color = if is_dark() { "#ff8a4c" } else { "#d9480f" };
    let mark = format!(
        "<span font_weight=\"700\" letter_spacing=\"-1200\">/dev<span foreground=\"{color}\">/</span>null</span>"
    );
    let needle = "/dev/null";
    let mut out = String::new();
    let mut rest = text;
    while let Some(at) = rest.find(needle) {
        let before = &rest[..at];
        let after = &rest[at + needle.len()..];
        let left = before.chars().next_back();
        let right = after.chars().next();
        out.push_str(before);
        if left_ok(left) && right_ok(right) {
            out.push_str(&mark);
        } else {
            out.push_str(needle);
        }
        rest = after;
    }
    out.push_str(rest);
    out
}

pub fn segments(text: &str) -> Vec<Segment> {
    let text = text.replace("\r\n", "\n");
    let chars: Vec<char> = text.chars().collect();
    let mut i = 0;
    let mut raw = Vec::new();
    let mut prose = String::new();
    while i < chars.len() {
        if let Some((seg, end)) = take_fence(&chars, i) {
            if !prose.is_empty() {
                raw.push(Segment::Text(std::mem::take(&mut prose)));
            }
            raw.push(seg);
            i = end;
            continue;
        }
        if let Some((code, end)) = take_inline(&chars, i) {
            if !prose.is_empty() {
                raw.push(Segment::Text(std::mem::take(&mut prose)));
            }
            raw.push(Segment::Code(code));
            i = end;
            continue;
        }
        prose.push(chars[i]);
        i += 1;
    }
    if !prose.is_empty() {
        raw.push(Segment::Text(prose));
    }
    trim_block_edges(raw)
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ColorSpan {
    value: String,
    /// Shown next to the swatch; for `color(#hex)` this is `#hex`, not the wrapper.
    label: String,
    index: usize,
    length: usize,
}

/// Keep in sync with `app/Services/Mentions.php` and `linkified-text.tsx`.
#[derive(Debug, Clone, PartialEq, Eq)]
struct MentionSpan {
    start: usize,
    end: usize,
    username: String,
}

fn with_mentions(text: &str, mentions: CapturedMentions<'_>) -> String {
    let users = mentions.users.filter(|map| !map.is_empty());
    let organizations = mentions.organizations.filter(|map| !map.is_empty());
    let spans = mention_spans(text);
    if spans.is_empty() {
        return decorate_plain(text);
    }
    let mut out = String::new();
    let mut last = 0;
    for mention in spans {
        out.push_str(&decorate_plain(&text[last..mention.start]));
        let token = &text[mention.start..mention.end];
        let user = users
            .and_then(|map| captured_mention(map, &mention.username))
            .is_some();
        let organization = organizations
            .and_then(|map| captured_mention(map, &mention.username))
            .is_some();
        if user {
            out.push_str(&format!(
                "<a href=\"mention:{}\"><span underline=\"none\">{}</span></a>",
                escape(&mention.username),
                escape(token)
            ));
        } else if organization {
            out.push_str(&format!(
                "<a href=\"org:{}\"><span underline=\"none\">{}</span></a>",
                escape(&mention.username),
                escape(token)
            ));
        } else if users.is_none() {
            out.push_str(&format!(
                "<a href=\"mention:{}\"><span underline=\"none\">{}</span></a>",
                escape(&mention.username),
                escape(token)
            ));
        } else {
            out.push_str(&decorate_plain(token));
        }
        last = mention.end;
    }
    out.push_str(&decorate_plain(&text[last..]));
    out
}

fn captured_mention<'a>(mentions: &'a HashMap<String, String>, username: &str) -> Option<&'a str> {
    mentions
        .get(&username.to_ascii_lowercase())
        .or_else(|| mentions.get(username))
        .map(String::as_str)
}

fn mention_spans(text: &str) -> Vec<MentionSpan> {
    let mut out = Vec::new();
    let mut i = 0;
    while i < text.len() {
        let Some(at) = text[i..].find('@') else {
            break;
        };
        let start = i + at;
        if !mention_left_ok(text, start) {
            i = start + 1;
            continue;
        }
        let Some(username) = take_github_username(&text[start + 1..]) else {
            i = start + 1;
            continue;
        };
        let end = start + 1 + username.len();
        if !mention_right_ok(text, end) {
            i = start + 1;
            continue;
        }
        out.push(MentionSpan {
            start,
            end,
            username: username.to_ascii_lowercase(),
        });
        i = end;
    }
    out
}

#[cfg(test)]
fn mention_usernames(text: &str) -> Vec<String> {
    let mut names = Vec::new();
    for mention in mention_spans(text) {
        if !names.contains(&mention.username) {
            names.push(mention.username);
        }
    }
    names
}

fn mention_left_ok(text: &str, at: usize) -> bool {
    let Some(prev) = text[..at].chars().next_back() else {
        return true;
    };
    !(prev.is_alphabetic() || prev.is_numeric() || matches!(prev, '_' | '.' | '/' | '@' | '-'))
}

fn take_github_username(s: &str) -> Option<&str> {
    let bytes = s.as_bytes();
    if !bytes.first().is_some_and(u8::is_ascii_alphanumeric) {
        return None;
    }
    let mut i = 1;
    let mut n = 1;
    while i < bytes.len() && n < 39 {
        let b = bytes[i];
        let hyphen = b == b'-'
            && bytes
                .get(i + 1)
                .is_some_and(|next| next.is_ascii_alphanumeric());
        if !b.is_ascii_alphanumeric() && !hyphen {
            break;
        }
        i += 1;
        n += 1;
    }
    Some(&s[..i])
}

/// The `@handle` being typed right before the caret, as the web composer finds it.
pub fn mention_trigger_at(before: &str) -> Option<(usize, String)> {
    let bytes = before.as_bytes();
    let at = bytes.iter().rposition(|&b| b == b'@')?;
    if !mention_left_ok(before, at) {
        return None;
    }
    let query = &before[at + 1..];
    if query.is_empty() {
        return Some((before[..at].chars().count(), String::new()));
    }
    if !query
        .bytes()
        .next()
        .is_some_and(|b| b.is_ascii_alphanumeric())
        || query.len() > 39
        || !query
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-')
    {
        return None;
    }
    Some((before[..at].chars().count(), query.to_owned()))
}

fn mention_right_ok(text: &str, end: usize) -> bool {
    let rest = &text[end..];
    let mut chars = rest.chars();
    let Some(next) = chars.next() else {
        return true;
    };
    if next.is_ascii_alphanumeric() || matches!(next, '_' | '@' | '/' | '-') {
        return false;
    }
    if next == '.'
        && chars
            .next()
            .is_some_and(|after| after.is_ascii_alphanumeric())
    {
        return false;
    }
    true
}

fn with_colors(text: &str) -> String {
    let mut out = String::new();
    let mut last = 0;
    for color in colors_in(text, false) {
        out.push_str(&text[last..color.index]);
        if let Some(css) = css_hex(&color.value) {
            out.push_str(&format!(
                "<span foreground=\"{css}\">■</span>{}",
                escape(&color.label)
            ));
        } else {
            out.push_str(&escape(&text[color.index..color.index + color.length]));
        }
        last = color.index + color.length;
    }
    out.push_str(&text[last..]);
    out
}

enum PlainMark {
    Color { value: String, label: String },
    Hashtag,
}

/// Explicit `color(#hex)` and digit-led bare hex win over hashtags. Letter-led `#dead` is a hashtag (#1630).
fn decorate_plain(text: &str) -> String {
    let mut marks: Vec<(usize, usize, PlainMark)> = Vec::new();
    for color in colors_in(text, false) {
        marks.push((
            color.index,
            color.index + color.length,
            PlainMark::Color {
                value: color.value,
                label: color.label,
            },
        ));
    }
    for (start, end) in hashtag_spans(text) {
        let overlaps = marks
            .iter()
            .any(|(color_start, color_end, _)| start < *color_end && *color_start < end);
        if !overlaps {
            marks.push((start, end, PlainMark::Hashtag));
        }
    }
    marks.sort_by_key(|(start, end, _)| (*start, *end));
    let mut out = String::new();
    let mut last = 0;
    for (start, end, kind) in marks {
        if start < last || end > text.len() {
            continue;
        }
        out.push_str(&with_wordmarks(&escape(&text[last..start])));
        match kind {
            PlainMark::Color { value, label } => {
                if let Some(css) = css_hex(&value) {
                    out.push_str(&format!(
                        "<span foreground=\"{css}\">■</span>{}",
                        escape(&label)
                    ));
                } else {
                    out.push_str(&escape(&text[start..end]));
                }
            }
            PlainMark::Hashtag => {
                let token = &text[start..end];
                out.push_str(&format!(
                    "<a href=\"hashtag:{}\"><span underline=\"none\">{}</span></a>",
                    escape(token),
                    escape(token)
                ));
            }
        }
        last = end;
    }
    out.push_str(&with_wordmarks(&escape(&text[last..])));
    out
}

fn hashtag_spans(text: &str) -> Vec<(usize, usize)> {
    let mut out = Vec::new();
    let mut i = 0;
    while i < text.len() {
        let Some(ch) = text[i..].chars().next() else {
            break;
        };
        let ch_len = ch.len_utf8();
        if ch != '#' && ch != '\u{FF03}' {
            i += ch_len;
            continue;
        }
        if !hashtag_left_ok(text, i) {
            i += ch_len;
            continue;
        }
        let rest = &text[i + ch_len..];
        let Some(first) = rest.chars().next() else {
            i += ch_len;
            continue;
        };
        if !first.is_alphabetic() {
            i += ch_len;
            continue;
        }
        let mut end = i + ch_len + first.len_utf8();
        while end < text.len() {
            let Some(next) = text[end..].chars().next() else {
                break;
            };
            if next.is_alphanumeric() || next == '_' || is_combining_mark(next) {
                end += next.len_utf8();
            } else {
                break;
            }
        }
        out.push((i, end));
        i = end;
    }
    out
}

fn hashtag_left_ok(text: &str, index: usize) -> bool {
    let Some(prev) = text[..index].chars().next_back() else {
        return true;
    };
    if prev.is_alphanumeric() || is_combining_mark(prev) {
        return false;
    }
    !matches!(prev, '_' | '&' | '/' | '#' | '\u{FF03}' | '>' | '-')
}

fn is_combining_mark(ch: char) -> bool {
    matches!(
        ch,
        '\u{0300}'..='\u{036F}'
            | '\u{1AB0}'..='\u{1AFF}'
            | '\u{1DC0}'..='\u{1DFF}'
            | '\u{20D0}'..='\u{20FF}'
            | '\u{FE20}'..='\u{FE2F}'
    )
}

/// Hex colors in prose. Keep in sync with `resources/js/lib/colors.ts` (#1630):
/// `color(#hex)` always, digit-led bare `#000` / `#333333`, and full-width `＃fff`.
/// Letter-led `#dead` / `#ff8a4c` need the explicit wrapper.
fn colors_in(text: &str, starts_text: bool) -> Vec<ColorSpan> {
    let hay = if starts_text {
        format!("\n{text}")
    } else {
        text.to_owned()
    };
    let shift = usize::from(starts_text);
    let mut out = Vec::new();
    let mut i = 0;
    while i < hay.len() {
        if let Some(span) = take_explicit_color(&hay, i) {
            out.push(ColorSpan {
                value: span.value,
                label: span.label,
                index: span.index.saturating_sub(shift),
                length: span.length,
            });
            i = span.index + span.length;
            continue;
        }
        if let Some(span) = take_bare_color(&hay, i) {
            out.push(ColorSpan {
                value: span.value,
                label: span.label,
                index: span.index.saturating_sub(shift),
                length: span.length,
            });
            i = span.index + span.length;
            continue;
        }
        let Some(next) = hay[i..].chars().next() else {
            break;
        };
        i += next.len_utf8();
    }
    out
}

fn take_explicit_color(text: &str, i: usize) -> Option<ColorSpan> {
    let rest = &text[i..];
    if rest.len() < 7 || !rest.is_char_boundary(7) || !rest[..7].eq_ignore_ascii_case("color(#") {
        return None;
    }
    if i > 0 {
        let before = text[..i].chars().next_back()?;
        if is_pastel_boundary_char(before) || before == '#' || before == '\u{FF03}' {
            return None;
        }
    }
    let open = 7;
    let (digits, hex_len) = take_color_digits(&rest[open..])?;
    let after_hex = open + hex_len;
    if rest.as_bytes().get(after_hex) != Some(&b')') {
        return None;
    }
    let length = after_hex + 1;
    Some(ColorSpan {
        value: digits.clone(),
        label: format!("#{digits}"),
        index: i,
        length,
    })
}

fn take_bare_color(text: &str, i: usize) -> Option<ColorSpan> {
    let rest = &text[i..];
    let hash = rest.chars().next()?;
    let hash_len = hash.len_utf8();
    let ascii_hash = hash == '#';
    let fullwidth = hash == '\u{FF03}';
    if !ascii_hash && !fullwidth {
        return None;
    }
    if !color_left_ok(text, i) {
        return None;
    }
    let hex_at = i + hash_len;
    let hex_rest = &text[hex_at..];
    if ascii_hash {
        let first = hex_rest.chars().next()?;
        if !first.is_ascii_digit() {
            return None;
        }
    }
    let (digits, hex_len) = take_color_digits(hex_rest)?;
    let end = hex_at + hex_len;
    if !color_right_ok(&text[end..]) {
        return None;
    }
    let label: String = text[i..end].chars().collect();
    Some(ColorSpan {
        value: digits,
        label,
        index: i,
        length: end - i,
    })
}

fn color_left_ok(text: &str, i: usize) -> bool {
    if i == 0 {
        return true;
    }
    let before = &text[..i];
    let mut chars = before.chars().rev();
    match chars.next() {
        None => true,
        Some(c) if c.is_whitespace() => true,
        Some('>') => chars.next() == Some('\n'),
        _ => false,
    }
}

fn color_right_ok(rest: &str) -> bool {
    let mut chars = rest.chars();
    loop {
        match chars.next() {
            None => return true,
            Some(c) if is_color_space(c) => return true,
            Some(c) if is_color_punct(c) => {}
            _ => return false,
        }
    }
}

fn is_color_space(c: char) -> bool {
    c.is_whitespace()
        || matches!(
            c,
            '\u{00AD}'
                | '\u{2060}'
                | '\u{200A}'
                | '\u{200B}'
                | '\u{200C}'
                | '\u{200D}'
                | '\u{20E2}'
        )
}

fn is_color_punct(c: char) -> bool {
    c.is_ascii_punctuation()
        || matches!(
            c,
            '—' | '–'
                | '…'
                | '“'
                | '”'
                | '‘'
                | '’'
                | '。'
                | '、'
                | '！'
                | '？'
                | '：'
                | '；'
        )
}

fn take_color_digits(rest: &str) -> Option<(String, usize)> {
    let mut hex_len = 0;
    for (offset, c) in rest.char_indices() {
        if c.is_ascii_hexdigit() && offset < 8 {
            hex_len = offset + 1;
        } else {
            break;
        }
    }
    let take = if hex_len >= 8 {
        8
    } else if hex_len >= 6 {
        6
    } else if hex_len >= 4 && short_color_ok(&rest[..4]) {
        4
    } else if hex_len >= 3 && short_color_ok(&rest[..3]) {
        3
    } else {
        return None;
    };
    Some((rest[..take].to_owned(), take))
}

fn short_color_ok(digits: &str) -> bool {
    digits.as_bytes().first() == Some(&b'0')
        || digits
            .bytes()
            .any(|b| b.is_ascii_hexdigit() && !b.is_ascii_digit())
}

/// True when `text` is exactly a `#` hex color token (`#fff`, `#ff8a4c`, …).
pub fn is_hex_color(text: &str) -> bool {
    let Some(rest) = text.strip_prefix('#') else {
        return false;
    };
    take_color_digits(rest).is_some_and(|(_, len)| len == rest.len())
}

/// True when the caret sits inside an unfinished `color(#…` prefix.
pub fn is_typing_explicit_color(text: &str) -> bool {
    let lower: String = text.chars().flat_map(char::to_lowercase).collect();
    lower.ends_with("color(")
        || lower
            .rsplit_once("color(")
            .is_some_and(|(_, after)| after.starts_with('#') && !after.contains(')'))
}

/// The last hashtag in a draft that could be meant as a swatch (`#dead` → `color(#dead)`).
pub fn ambiguous_color_in_draft(text: &str) -> Option<(usize, String)> {
    hashtag_spans(text)
        .into_iter()
        .rev()
        .find(|(start, end)| {
            let token = &text[*start..*end];
            is_hex_color(token) && !is_typing_explicit_color(&text[..*start])
        })
        .map(|(start, end)| (start, text[start..end].to_owned()))
}

/// Wrap the last ambiguous color hashtag as `color(#hex)` for the composer hint (#1634).
pub fn apply_color_swatch_hint(text: &str) -> Option<(String, usize)> {
    let (index, hex) = ambiguous_color_in_draft(text)?;
    let markup = format!("color({hex})");
    let mut out = String::with_capacity(text.len() + markup.len());
    out.push_str(&text[..index]);
    out.push_str(&markup);
    out.push_str(&text[index + hex.len()..]);
    let caret = index + markup.len();
    Some((out, caret))
}

fn css_hex(digits: &str) -> Option<String> {
    let d = digits.to_ascii_lowercase();
    let rgb = match d.len() {
        3 | 4 => {
            let b = d.as_bytes();
            format!(
                "{}{}{}{}{}{}",
                b[0] as char, b[0] as char, b[1] as char, b[1] as char, b[2] as char, b[2] as char
            )
        }
        6 | 8 => d[..6].to_owned(),
        _ => return None,
    };
    Some(format!("#{rgb}"))
}

fn left_ok(prev: Option<char>) -> bool {
    match prev {
        None => true,
        Some(c) => !(c.is_alphanumeric() || matches!(c, '_' | '.' | '/' | '-')),
    }
}

fn right_ok(next: Option<char>) -> bool {
    match next {
        None => true,
        Some(c) => !(c.is_alphanumeric() || matches!(c, '_' | '/' | '-')),
    }
}

fn take_fence(chars: &[char], i: usize) -> Option<(Segment, usize)> {
    if i != 0 && chars[i - 1] != '\n' {
        return None;
    }
    let mut ticks = 0;
    while i + ticks < chars.len() && chars[i + ticks] == '`' {
        ticks += 1;
    }
    if ticks < 3 {
        return None;
    }
    let mut j = i + ticks;
    let lang_start = j;
    while j < chars.len() && chars[j] != '\n' {
        if chars[j] == '`' {
            return None;
        }
        j += 1;
    }
    if j >= chars.len() {
        return None;
    }
    let header: String = chars[lang_start..j].iter().collect();
    j += 1;
    let body_start = j;
    let mut line_start = j;
    while j <= chars.len() {
        let at_eol = j == chars.len() || chars[j] == '\n';
        if at_eol {
            let line = &chars[line_start..j];
            if is_closing_fence(line, ticks) {
                let body: String = chars[body_start..line_start].iter().collect();
                let code = body.strip_suffix('\n').unwrap_or(&body).to_owned();
                let (language, filename) = fence_language_and_filename(&header);
                let end = if j < chars.len() { j + 1 } else { j };
                return Some((
                    Segment::Block {
                        language,
                        filename,
                        code,
                    },
                    end,
                ));
            }
            if j == chars.len() {
                break;
            }
            j += 1;
            line_start = j;
            continue;
        }
        j += 1;
    }
    None
}

fn is_closing_fence(line: &[char], open_ticks: usize) -> bool {
    let mut ticks = 0;
    while ticks < line.len() && line[ticks] == '`' {
        ticks += 1;
    }
    ticks >= open_ticks && line[ticks..].iter().all(|c| *c == ' ' || *c == '\t')
}

/// Language is the first space/tab-separated word. `title="…"` labels the block; `filename="…"` is the alias.
fn fence_language_and_filename(header: &str) -> (Option<String>, Option<String>) {
    let header = header.trim_matches([' ', '\t']);
    if header.is_empty() {
        return (None, None);
    }
    let Some(split) = header.find([' ', '\t']) else {
        return (Some(header.to_owned()), None);
    };
    let language = header[..split].to_owned();
    let meta = header[split..].trim_start_matches([' ', '\t']);
    (
        if language.is_empty() {
            None
        } else {
            Some(language)
        },
        fence_filename(meta),
    )
}

fn fence_filename(meta: &str) -> Option<String> {
    let mut title = None;
    let mut filename = None;
    let mut i = 0;
    while i < meta.len() {
        let rest = &meta[i..];
        let skipped = rest
            .bytes()
            .take_while(|b| *b == b' ' || *b == b'\t')
            .count();
        i += skipped;
        if i >= meta.len() {
            break;
        }
        let rest = &meta[i..];
        if rest.starts_with('{') {
            match rest.find('}') {
                Some(end) => {
                    i += end + 1;
                    continue;
                }
                None => break,
            }
        }
        if let Some((key, value, consumed)) = take_fence_attr(rest) {
            if !value.is_empty() {
                if key.eq_ignore_ascii_case("title") {
                    title.get_or_insert_with(|| value.to_owned());
                } else if key.eq_ignore_ascii_case("filename") {
                    filename.get_or_insert_with(|| value.to_owned());
                }
            }
            i += consumed;
            continue;
        }
        i += rest.find([' ', '\t']).unwrap_or(rest.len()).max(1);
    }
    title.or(filename)
}

fn take_fence_attr(s: &str) -> Option<(&str, &str, usize)> {
    let bytes = s.as_bytes();
    if !bytes.first()?.is_ascii_alphabetic() {
        return None;
    }
    let mut i = 1;
    while i < bytes.len()
        && (bytes[i].is_ascii_alphanumeric() || bytes[i] == b'_' || bytes[i] == b'-')
    {
        i += 1;
    }
    if bytes.get(i).copied() != Some(b'=') || bytes.get(i + 1).copied() != Some(b'"') {
        return None;
    }
    let key = &s[..i];
    let value_at = i + 2;
    let close = s.get(value_at..)?.find('"')?;
    let value = &s[value_at..value_at + close];
    Some((key, value, value_at + close + 1))
}

fn take_inline(chars: &[char], i: usize) -> Option<(String, usize)> {
    if chars[i] != '`' {
        return None;
    }
    let mut j = i + 1;
    while j < chars.len() && chars[j] != '`' && chars[j] != '\n' {
        j += 1;
    }
    if j < chars.len() && chars[j] == '`' && j > i + 1 {
        let code: String = chars[i + 1..j].iter().collect();
        Some((code, j + 1))
    } else {
        None
    }
}

fn trim_block_edges(mut raw: Vec<Segment>) -> Vec<Segment> {
    let is_block_at = |raw: &[Segment], i: usize| matches!(raw.get(i), Some(Segment::Block { .. }));
    for i in 0..raw.len() {
        let strip_start = i > 0 && is_block_at(&raw, i - 1);
        let strip_end = is_block_at(&raw, i + 1);
        if let Segment::Text(text) = &mut raw[i] {
            if strip_start {
                if let Some(stripped) = text.strip_prefix('\n') {
                    *text = stripped.to_owned();
                }
            }
            if strip_end {
                if let Some(stripped) = text.strip_suffix('\n') {
                    *text = stripped.to_owned();
                }
            }
        }
    }
    raw.retain(|seg| !matches!(seg, Segment::Text(t) if t.is_empty()));
    raw
}

struct ListState {
    ordered: bool,
    next: u64,
}

fn markdown_markup(text: &str, mentions: CapturedMentions<'_>, pasteis_mode: bool) -> String {
    let mut out = String::new();
    let mut in_link = false;
    let mut lists = Vec::new();
    let mut options = Options::empty();
    options.insert(Options::ENABLE_STRIKETHROUGH);
    let parser = Parser::new_ext(text, options);
    for event in parser {
        match event {
            Event::Start(Tag::Emphasis) => out.push_str("<i>"),
            Event::End(TagEnd::Emphasis) => out.push_str("</i>"),
            Event::Start(Tag::Strong) => out.push_str("<b>"),
            Event::End(TagEnd::Strong) => out.push_str("</b>"),
            Event::Start(Tag::Strikethrough) => out.push_str("<s>"),
            Event::End(TagEnd::Strikethrough) => out.push_str("</s>"),
            Event::Start(Tag::Heading { .. }) => out.push_str("<b>"),
            Event::End(TagEnd::Heading(_)) => {
                out.push_str("</b>");
                push_break(&mut out, lists.is_empty());
            }
            Event::Start(Tag::CodeBlock(_)) => out.push_str("<tt>"),
            Event::End(TagEnd::CodeBlock) => out.push_str("</tt>"),
            Event::Code(code) => {
                out.push_str("<tt>");
                out.push_str(&escape(&code));
                out.push_str("</tt>");
            }
            Event::Text(t) | Event::Html(t) | Event::InlineHtml(t) => {
                if in_link {
                    out.push_str(&with_pasteis(&t, pasteis_mode, |chunk| {
                        with_wordmarks(&with_colors(&escape(chunk)))
                    }));
                } else {
                    out.push_str(&with_pasteis(&t, pasteis_mode, |chunk| {
                        with_urls(chunk, mentions)
                    }));
                }
            }
            Event::SoftBreak | Event::HardBreak => out.push('\n'),
            Event::Start(Tag::Link { dest_url, .. }) => {
                in_link = true;
                out.push_str("<a href=\"");
                out.push_str(&escape(&dest_url));
                out.push_str("\"><span underline=\"none\">");
            }
            Event::End(TagEnd::Link) => {
                in_link = false;
                out.push_str("</span></a>");
            }
            Event::Start(Tag::List(start)) => {
                lists.push(ListState {
                    ordered: start.is_some(),
                    next: start.unwrap_or(1),
                });
                if !out.is_empty() && !out.ends_with('\n') {
                    out.push('\n');
                }
            }
            Event::End(TagEnd::List(_)) => {
                lists.pop();
                if !out.ends_with('\n') {
                    out.push('\n');
                }
            }
            Event::Start(Tag::Item) => {
                let indent = "  ".repeat(lists.len().saturating_sub(1));
                if let Some(list) = lists.last_mut() {
                    if list.ordered {
                        out.push_str(&format!("{indent}{}. ", list.next));
                        list.next += 1;
                    } else {
                        out.push_str(&format!("{indent}• "));
                    }
                }
            }
            Event::End(TagEnd::Item) => {
                if !out.ends_with('\n') {
                    out.push('\n');
                }
            }
            Event::End(TagEnd::Paragraph) => push_break(&mut out, lists.is_empty()),
            Event::Rule => out.push_str("\n\n"),
            _ => {}
        }
    }
    out.trim_end_matches('\n').to_owned()
}

fn push_break(out: &mut String, blank_line: bool) {
    if !out.ends_with('\n') {
        out.push('\n');
    }
    if blank_line {
        out.push('\n');
    }
}

/// Sentinel in Pango markup for a pastel word; `prose_widget` swaps it for the nata icon.
const PASTEL_MARK: char = '\u{E000}';

/// When Pastéis mode is on, mark pastel words and strip the escape backslash from `\pastel`.
/// Keep matching `pasteisParts` on the web.
fn with_pasteis(text: &str, pasteis_mode: bool, decorate: impl Fn(&str) -> String) -> String {
    if !pasteis_mode {
        return decorate(text);
    }
    let mut out = String::new();
    for part in pasteis_parts(text) {
        if part.pastel {
            out.push(PASTEL_MARK);
        } else {
            out.push_str(&decorate(&part.text));
        }
    }
    out
}

/// Keep in sync with `app/Services/Urls.php` and `linkified-text.tsx`.
const TOP_LEVEL_DOMAINS: &[&str] = &[
    "com", "org", "net", "edu", "gov", "mil", "info", "biz", "io", "dev", "app", "co", "ai", "me",
    "xyz", "tech", "gg", "tv", "fm", "ly", "to", "is", "blog", "cloud", "site", "online", "design",
    "studio", "tools", "software", "codes", "wiki", "news", "website", "club", "ninja", "rocks",
    "academy", "guide", "social", "digital", "agency", "fyi", "lol", "live", "uk", "de", "fr",
    "nl", "es", "it", "pt", "br", "ca", "au", "jp", "ch", "se", "no", "dk", "fi", "be", "at", "ie",
    "nz", "us", "eu", "cz", "sk", "hu", "ro", "bg", "gr", "hr", "si", "ee", "lv", "lt", "ru", "ua",
    "tr", "il", "za", "mx", "ar", "kr", "cn", "tw", "hk", "sg", "vn", "th",
];

struct UrlSpan {
    start: usize,
    end: usize,
    href: String,
}

fn with_urls(text: &str, mentions: CapturedMentions<'_>) -> String {
    let spans = url_spans(text);
    if spans.is_empty() {
        return with_mentions(text, mentions);
    }
    let mut out = String::new();
    let mut last = 0;
    for span in spans {
        out.push_str(&with_mentions(&text[last..span.start], mentions));
        let token = &text[span.start..span.end];
        out.push_str(&format!(
            "<a href=\"{}\"><span underline=\"none\">{}</span></a>",
            escape(&span.href),
            escape(&shorten_url_label(token))
        ));
        last = span.end;
    }
    out.push_str(&with_mentions(&text[last..], mentions));
    out
}

/// HTTP(S) and bare URLs in `text`, in order, suitable for link-card choice.
pub fn body_urls(text: &str) -> Vec<String> {
    url_spans(text).into_iter().map(|span| span.href).collect()
}

fn url_spans(text: &str) -> Vec<UrlSpan> {
    let mut out = Vec::new();
    let mut i = 0;
    while i < text.len() {
        if let Some(span) = take_scheme_url(text, i).or_else(|| take_bare_url(text, i)) {
            i = span.end;
            out.push(span);
            continue;
        }
        i += text[i..].chars().next().map(char::len_utf8).unwrap_or(1);
    }
    out
}

fn take_scheme_url(text: &str, i: usize) -> Option<UrlSpan> {
    if i > 0 {
        let prev = text[..i].chars().next_back()?;
        if prev.is_ascii_alphanumeric() {
            return None;
        }
    }
    let rest = &text[i..];
    let prefix = if ascii_prefix(rest, "https://") {
        8
    } else if ascii_prefix(rest, "http://") {
        7
    } else {
        return None;
    };
    let mut j = prefix;
    while j < rest.len() {
        let c = rest[j..].chars().next()?;
        if c.is_whitespace() || matches!(c, '<' | '>') {
            break;
        }
        j += c.len_utf8();
    }
    if j == prefix {
        return None;
    }
    let trimmed = without_trailing_punct(&rest[..j]);
    if trimmed.len() == prefix || trimmed.ends_with("://") || trimmed.chars().count() > 2048 {
        return None;
    }
    Some(UrlSpan {
        start: i,
        end: i + trimmed.len(),
        href: trimmed.to_owned(),
    })
}

fn take_bare_url(text: &str, i: usize) -> Option<UrlSpan> {
    if !url_left_ok(text, i) {
        return None;
    }
    let rest = &text[i..];
    let bytes = rest.as_bytes();
    if !bytes.first().is_some_and(u8::is_ascii_alphanumeric) {
        return None;
    }
    let mut j = 0;
    let mut last_dot = None;
    while j < bytes.len() {
        if !bytes[j].is_ascii_alphanumeric() {
            return None;
        }
        let mut last_alnum = j;
        j += 1;
        while j < bytes.len() && (bytes[j].is_ascii_alphanumeric() || bytes[j] == b'-') {
            if bytes[j].is_ascii_alphanumeric() {
                last_alnum = j;
            }
            j += 1;
        }
        j = last_alnum + 1;
        if j < bytes.len()
            && bytes[j] == b'.'
            && bytes.get(j + 1).is_some_and(|b| b.is_ascii_alphanumeric())
        {
            last_dot = Some(j);
            j += 1;
            continue;
        }
        break;
    }
    let dot = last_dot?;
    let tld = &rest[dot + 1..j];
    if tld.is_empty() || !is_allowed_tld(tld) {
        return None;
    }
    if let Some(next) = rest[j..].chars().next() {
        if next.is_ascii_alphanumeric() || matches!(next, '_' | '(' | '-') {
            return None;
        }
        if next == '.'
            && rest[j + 1..]
                .chars()
                .next()
                .is_some_and(|after| after.is_ascii_alphanumeric())
        {
            return None;
        }
    }
    if let Some(next) = rest[j..].chars().next() {
        if matches!(next, '/' | '?' | '#') {
            j += next.len_utf8();
            while j < rest.len() {
                let c = rest[j..].chars().next()?;
                if c.is_whitespace() || matches!(c, '<' | '>') {
                    break;
                }
                j += c.len_utf8();
            }
        }
    }
    let trimmed = without_trailing_punct(&rest[..j]);
    if trimmed.is_empty() || trimmed.chars().count() > 2048 {
        return None;
    }
    Some(UrlSpan {
        start: i,
        end: i + trimmed.len(),
        href: format!("https://{trimmed}"),
    })
}

fn url_left_ok(text: &str, i: usize) -> bool {
    let Some(prev) = text[..i].chars().next_back() else {
        return true;
    };
    !(prev.is_alphabetic() || prev.is_numeric() || matches!(prev, '_' | '.' | '/' | '@' | '-'))
}

fn is_allowed_tld(tld: &str) -> bool {
    if TOP_LEVEL_DOMAINS.contains(&tld) {
        return true;
    }
    if !tld.bytes().all(|b| b.is_ascii_uppercase()) {
        return false;
    }
    let lower = tld.to_ascii_lowercase();
    TOP_LEVEL_DOMAINS.contains(&lower.as_str())
}

fn without_trailing_punct(url: &str) -> &str {
    let mut end = url.len();
    loop {
        let previous = end;
        while end > 0 {
            let ch = url[..end].chars().next_back().unwrap();
            if matches!(ch, '.' | ',' | ';' | ':' | '!' | '?' | '\'' | '"') {
                end -= ch.len_utf8();
            } else {
                break;
            }
        }
        if end > 0 && url[..end].ends_with(')') {
            let opens = bytecount(&url.as_bytes()[..end], b'(');
            let closes = bytecount(&url.as_bytes()[..end], b')');
            if opens < closes {
                end -= 1;
            }
        }
        if end > 0 && url[..end].ends_with(']') {
            let opens = bytecount(&url.as_bytes()[..end], b'[');
            let closes = bytecount(&url.as_bytes()[..end], b']');
            if opens < closes {
                end -= 1;
            }
        }
        if end == previous {
            break;
        }
    }
    &url[..end]
}

fn bytecount(bytes: &[u8], needle: u8) -> usize {
    bytes.iter().filter(|b| **b == needle).count()
}

fn ascii_prefix(s: &str, prefix: &str) -> bool {
    s.get(..prefix.len())
        .is_some_and(|head| head.eq_ignore_ascii_case(prefix))
}

fn shorten_url_label(url: &str) -> String {
    let mut rest = url;
    if ascii_prefix(rest, "https://") {
        rest = &rest[8..];
    } else if ascii_prefix(rest, "http://") {
        rest = &rest[7..];
    }
    if ascii_prefix(rest, "www.") {
        let after = &rest[4..];
        let host_end = after.find(['/', '?', '#']).unwrap_or(after.len());
        if after[..host_end].contains('.') {
            rest = after;
        }
    }
    let host_end = rest.find(['/', '?', '#']).unwrap_or(rest.len());
    let host = &rest[..host_end];
    let path = &rest[host_end..];
    let path_chars: Vec<char> = path.chars().collect();
    if path_chars.len() <= 20 {
        return rest.to_owned();
    }
    let mut kept: String = path_chars.iter().take(19).collect();
    if kept.ends_with('%') {
        kept.pop();
    } else if kept.len() >= 2 {
        let bytes = kept.as_bytes();
        let n = bytes.len();
        if bytes[n - 2] == b'%' && bytes[n - 1].is_ascii_hexdigit() {
            kept.pop();
            kept.pop();
        }
    }
    format!("{host}{kept}…")
}

fn prose_widget(markup: &str, sender: Option<&Sender<AppMsg>>) -> gtk::Widget {
    if !markup.contains(PASTEL_MARK) {
        return prose_label(markup, sender).upcast();
    }
    let row = gtk::Box::new(gtk::Orientation::Horizontal, 2);
    row.set_hexpand(true);
    row.add_css_class("dn-pastel-prose");
    // FlowBox keeps pastel icons inline with wrapping prose chunks.
    let flow = gtk::FlowBox::new();
    flow.set_selection_mode(gtk::SelectionMode::None);
    flow.set_homogeneous(false);
    flow.set_hexpand(true);
    flow.set_max_children_per_line(64);
    flow.set_min_children_per_line(1);
    flow.set_column_spacing(2);
    flow.set_row_spacing(0);
    flow.set_valign(gtk::Align::Start);
    for (index, chunk) in markup.split(PASTEL_MARK).enumerate() {
        if index > 0 {
            let icon = gtk::Image::from_file(ui::pastel_path());
            icon.set_pixel_size(18);
            icon.set_valign(gtk::Align::Center);
            icon.set_tooltip_text(Some("pastel"));
            flow.insert(&icon, -1);
        }
        if chunk.is_empty() {
            continue;
        }
        let label = prose_label(chunk, sender);
        label.set_hexpand(false);
        label.set_width_chars(-1);
        flow.insert(&label, -1);
    }
    row.append(&flow);
    row.upcast()
}

fn prose_label(markup: &str, sender: Option<&Sender<AppMsg>>) -> gtk::Label {
    let label = gtk::Label::new(None);
    label.set_wrap(true);
    label.set_wrap_mode(pango::WrapMode::WordChar);
    label.set_xalign(0.0);
    label.set_hexpand(true);
    label.set_width_chars(1);
    label.set_selectable(true);
    label.set_use_markup(true);
    label.set_markup(markup);
    if let Some(sender) = sender {
        let sender = sender.clone();
        label.connect_activate_link(move |_, uri| {
            if let Some(handle) = uri.strip_prefix("org:") {
                sender.emit(AppMsg::OpenOrganization(handle.to_string()));
            } else if let Some(username) = uri.strip_prefix("mention:") {
                sender.emit(AppMsg::OpenProfile(username.to_string()));
            } else if let Some(tag) = uri.strip_prefix("hashtag:") {
                sender.emit(AppMsg::RunSearch(tag.to_string()));
            } else {
                sender.emit(AppMsg::OpenWeb(uri.to_string()));
            }
            glib::Propagation::Stop
        });
    }
    label
}

/// A faded truncated code block already shows that it continues, so drop a trailing `…`.
fn without_fade_ellipsis(mut segs: Vec<Segment>, sources: &[String]) -> Vec<Segment> {
    let Some(Segment::Text(text)) = segs.last() else {
        return segs;
    };
    if text.trim() != "…" {
        return segs;
    }
    let block_index = segs
        .iter()
        .filter(|segment| matches!(segment, Segment::Block { .. }))
        .count()
        .saturating_sub(1);
    let Some(Segment::Block { code, .. }) = segs
        .iter()
        .rev()
        .find(|segment| matches!(segment, Segment::Block { .. }))
    else {
        return segs;
    };
    if sources
        .get(block_index)
        .is_some_and(|source| source != code)
    {
        segs.pop();
    }
    segs
}

fn block_sources(text: &str) -> Vec<String> {
    segments(text)
        .into_iter()
        .filter_map(|segment| match segment {
            Segment::Block { code, .. } => Some(code),
            _ => None,
        })
        .collect()
}

#[cfg(test)]
fn copy_source_for_block(preview: &str, source: &str, index: usize) -> Option<String> {
    let displayed = segments(preview)
        .into_iter()
        .filter_map(|segment| match segment {
            Segment::Block { code, .. } => Some(code),
            _ => None,
        })
        .nth(index)?;
    Some(
        block_sources(source)
            .into_iter()
            .nth(index)
            .unwrap_or(displayed),
    )
}

fn code_block(
    language: Option<&str>,
    filename: Option<&str>,
    code: &str,
    source: &str,
    faded: bool,
) -> gtk::Widget {
    let wrap = gtk::Box::new(gtk::Orientation::Vertical, 0);
    wrap.add_css_class("dn-code");
    wrap.set_overflow(gtk::Overflow::Hidden);
    wrap.set_hexpand(true);
    let header = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    header.add_css_class("dn-code-header");
    let label = filename.filter(|value| !value.is_empty()).or(language);
    let lang = gtk::Label::new(label);
    lang.add_css_class("dn-mono");
    lang.add_css_class("dn-muted");
    lang.set_xalign(0.0);
    lang.set_hexpand(true);
    lang.set_ellipsize(pango::EllipsizeMode::End);
    lang.set_single_line_mode(true);
    lang.set_width_chars(1);
    if let Some(filename) = filename.filter(|value| !value.is_empty()) {
        lang.set_tooltip_text(Some(filename));
    }
    header.append(&lang);
    let copy = gtk::Button::new();
    copy.set_icon_name("dn-copy-symbolic");
    copy.set_tooltip_text(Some("Copy"));
    copy.add_css_class("flat");
    copy.add_css_class("dn-code-copy");
    let copied = source.to_owned();
    copy.connect_clicked(move |_| {
        let _ = ui::copy_text(&copied);
    });
    header.append(&copy);
    wrap.append(&header);
    let body = gtk::Label::new(None);
    body.add_css_class("dn-code-body");
    body.set_xalign(0.0);
    body.set_hexpand(true);
    body.set_width_chars(1);
    body.set_selectable(true);
    body.set_wrap(true);
    body.set_wrap_mode(pango::WrapMode::Char);
    body.set_use_markup(true);
    body.set_markup(&highlight_pango(code, language));
    if faded {
        let overlay = gtk::Overlay::new();
        overlay.set_hexpand(true);
        overlay.set_overflow(gtk::Overflow::Hidden);
        overlay.set_child(Some(&body));
        let fade = gtk::Box::new(gtk::Orientation::Horizontal, 0);
        fade.add_css_class("dn-code-fade");
        fade.set_hexpand(true);
        fade.set_valign(gtk::Align::End);
        fade.set_can_target(false);
        fade.set_size_request(-1, 48);
        overlay.add_overlay(&fade);
        wrap.append(&overlay);
    } else {
        wrap.append(&body);
    }
    wrap.upcast()
}

fn highlight_pango(code: &str, language: Option<&str>) -> String {
    static SYNTAXES: OnceLock<SyntaxSet> = OnceLock::new();
    static THEMES: OnceLock<ThemeSet> = OnceLock::new();
    let syntaxes = SYNTAXES.get_or_init(SyntaxSet::load_defaults_newlines);
    let themes = THEMES.get_or_init(ThemeSet::load_defaults);
    let syntax = language
        .map(str::to_ascii_lowercase)
        .and_then(|lang| syntaxes.find_syntax_by_token(&lang))
        .unwrap_or_else(|| syntaxes.find_syntax_plain_text());
    let theme_name = if is_dark() {
        "base16-ocean.dark"
    } else {
        "InspiredGitHub"
    };
    let Some(theme) = themes.themes.get(theme_name) else {
        return format!("<tt>{}</tt>", escape(code));
    };
    let mut highlighter = HighlightLines::new(syntax, theme);
    let mut out = String::from("<span font_family=\"monospace\">");
    for line in LinesWithEndings::from(code) {
        match highlighter.highlight_line(line, syntaxes) {
            Ok(ranges) => {
                for (style, text) in ranges {
                    let fg = style.foreground;
                    let escaped = escape(text.trim_end_matches('\n'));
                    let italic = style.font_style.contains(FontStyle::ITALIC);
                    let bold = style.font_style.contains(FontStyle::BOLD);
                    out.push_str(&format!(
                        "<span foreground=\"#{:02x}{:02x}{:02x}\">",
                        fg.r, fg.g, fg.b
                    ));
                    if bold {
                        out.push_str("<b>");
                    }
                    if italic {
                        out.push_str("<i>");
                    }
                    out.push_str(&escaped);
                    if italic {
                        out.push_str("</i>");
                    }
                    if bold {
                        out.push_str("</b>");
                    }
                    out.push_str("</span>");
                    if text.ends_with('\n') {
                        out.push('\n');
                    }
                }
            }
            Err(_) => {
                out.push_str(&escape(line.trim_end_matches('\n')));
                if line.ends_with('\n') {
                    out.push('\n');
                }
            }
        }
    }
    out.push_str("</span>");
    out
}

fn escape(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    #[test]
    fn splits_inline_and_fenced_code() {
        let segs = segments("run `make` then\n```rust\nlet x = 1;\n```\ndone");
        assert!(matches!(&segs[0], Segment::Text(t) if t == "run "));
        assert!(matches!(&segs[1], Segment::Code(c) if c == "make"));
        assert!(matches!(&segs[2], Segment::Text(t) if t == " then"));
        assert!(
            matches!(&segs[3], Segment::Block { language, filename, code } if language.as_deref() == Some("rust") && filename.is_none() && code == "let x = 1;")
        );
        assert!(matches!(&segs[4], Segment::Text(t) if t == "done"));
    }

    #[test]
    fn pasteis_draws_pastel_words_in_any_case() {
        assert_eq!(
            pasteis_parts("one Pastel, two PASTÉIS, more pasteis"),
            vec![
                PasteisPart {
                    text: "one ".into(),
                    pastel: false
                },
                PasteisPart {
                    text: "Pastel".into(),
                    pastel: true
                },
                PasteisPart {
                    text: ", two ".into(),
                    pastel: false
                },
                PasteisPart {
                    text: "PASTÉIS".into(),
                    pastel: true
                },
                PasteisPart {
                    text: ", more ".into(),
                    pastel: false
                },
                PasteisPart {
                    text: "pasteis".into(),
                    pastel: true
                },
            ]
        );
    }

    #[test]
    fn pasteis_leaves_words_inside_other_words() {
        assert_eq!(
            pasteis_parts("pastels and pastelaria"),
            vec![PasteisPart {
                text: "pastels and pastelaria".into(),
                pastel: false
            }]
        );
    }

    #[test]
    fn pasteis_keeps_escaped_words_without_the_backslash() {
        assert_eq!(
            pasteis_parts("I love \\pastel de nata, not \\Pastéis"),
            vec![PasteisPart {
                text: "I love pastel de nata, not Pastéis".into(),
                pastel: false
            }]
        );
        assert_eq!(
            pasteis_parts("\\pastel or pastel"),
            vec![
                PasteisPart {
                    text: "pastel or ".into(),
                    pastel: false
                },
                PasteisPart {
                    text: "pastel".into(),
                    pastel: true
                },
            ]
        );
    }

    #[test]
    fn pasteis_leaves_backslash_after_a_word_or_backslash() {
        assert_eq!(
            pasteis_parts("path\\pastel and \\\\pastel"),
            vec![PasteisPart {
                text: "path\\pastel and \\\\pastel".into(),
                pastel: false
            }]
        );
    }

    #[test]
    fn pasteis_keeps_text_without_a_pastel_whole() {
        assert!(pasteis_parts("").is_empty());
        assert_eq!(
            pasteis_parts("custard"),
            vec![PasteisPart {
                text: "custard".into(),
                pastel: false
            }]
        );
    }

    #[test]
    fn markdown_keeps_paragraphs_and_inline_styles() {
        let marked = body_markup("hello\n\n**world** and *now*\n\n~~gone~~");
        assert!(marked.contains("hello\n\n"));
        assert!(marked.contains("<b>world</b>"));
        assert!(marked.contains("<i>now</i>"));
        assert!(marked.contains("<s>gone</s>"));
    }

    #[test]
    fn markdown_lists_show_markers() {
        let bullets = body_markup("- one\n- two");
        assert!(bullets.contains("• one"));
        assert!(bullets.contains("• two"));
        let numbered = body_markup("1. alpha\n2. beta");
        assert!(numbered.contains("1. alpha"));
        assert!(numbered.contains("2. beta"));
    }

    #[test]
    fn markdown_links_urls_in_prose() {
        let marked = body_markup("See https://example.com/docs and nativephp.com.");
        assert!(marked.contains("href=\"https://example.com/docs\""));
        assert!(marked.contains("href=\"https://nativephp.com\""));
        assert!(marked.contains(">example.com/docs</span></a>"));
        assert!(!marked.contains("href=\"https://nativephp.com.\""));
    }

    #[test]
    fn markdown_does_not_slice_through_emoji() {
        let marked = body_markup("hello 👋 https://dev-null.sh");
        assert!(marked.contains("👋"));
        assert!(marked.contains("href=\"https://dev-null.sh\""));
        assert_eq!(
            shorten_url_label("👋example.com/docs"),
            "👋example.com/docs"
        );
    }

    #[test]
    fn unclosed_fence_stays_prose() {
        let segs = segments("```php\n$x = 1;");
        assert!(matches!(&segs[..], [Segment::Text(t)] if t.contains("```php")));
    }

    #[test]
    fn keeps_a_shorter_fence_inside_a_code_block() {
        let segs = segments("before\n````markdown\n```\nfence inside fence\n```\n````\nafter");
        assert!(matches!(&segs[0], Segment::Text(t) if t == "before"));
        assert!(matches!(
            &segs[1],
            Segment::Block { language, filename, code }
                if language.as_deref() == Some("markdown")
                    && filename.is_none()
                    && code == "```\nfence inside fence\n```"
        ));
        assert!(matches!(&segs[2], Segment::Text(t) if t == "after"));
        let longer_close = segments("````\ncode\n`````\nprose");
        assert!(matches!(
            &longer_close[0],
            Segment::Block { code, .. } if code == "code"
        ));
        assert!(matches!(&longer_close[1], Segment::Text(t) if t == "prose"));
    }

    #[test]
    fn labels_a_code_block_from_its_title() {
        let titled = segments("``` php\ttitle=\"app/Models/User.php\" \t\n$name;\n```");
        assert!(matches!(
            &titled[0],
            Segment::Block { language, filename, code }
                if language.as_deref() == Some("php")
                    && filename.as_deref() == Some("app/Models/User.php")
                    && code == "$name;"
        ));
        let alias = segments("```php filename=\"app/Models/User.php\"\n$name;\n```");
        assert!(matches!(
            &alias[0],
            Segment::Block { filename, .. } if filename.as_deref() == Some("app/Models/User.php")
        ));
        let both = segments("```php filename=\"a.php\" title=\"b.php\"\n$name;\n```");
        assert!(matches!(
            &both[0],
            Segment::Block { filename, .. } if filename.as_deref() == Some("b.php")
        ));
        let empty_title = segments("```php title=\"\" filename=\"a.php\"\n$name;\n```");
        assert!(matches!(
            &empty_title[0],
            Segment::Block { filename, .. } if filename.as_deref() == Some("a.php")
        ));
        let first_title = segments("```php title=\"a.php\" title=\"b.php\"\n$name;\n```");
        assert!(matches!(
            &first_title[0],
            Segment::Block { filename, .. } if filename.as_deref() == Some("a.php")
        ));
        let spaced = segments("```php title=\"My Model.php\"\n$name;\n```");
        assert!(matches!(
            &spaced[0],
            Segment::Block { filename, .. } if filename.as_deref() == Some("My Model.php")
        ));
        let extras =
            segments("```php {1, 3} showLineNumbers title=\"app.php\" tab=\"Pest 4\"\n$name;\n```");
        assert!(matches!(
            &extras[0],
            Segment::Block { language, filename, .. }
                if language.as_deref() == Some("php") && filename.as_deref() == Some("app.php")
        ));
        for fence in [
            "```php app/Models/User.php",
            "```php {1,3}",
            "```php tab=Pest",
            "```php title=\"\"",
            "```php title=\"app.php",
            "```php caption=\"see title=\"app.php\"",
            "```php {1 title=\"app.php\"}",
        ] {
            let segs = segments(&format!("{fence}\n$name;\n```"));
            assert!(
                matches!(&segs[0], Segment::Block { language, filename, .. } if language.as_deref() == Some("php") && filename.is_none()),
                "{fence}"
            );
        }
    }

    #[test]
    fn wordmarks_standalone_only() {
        let marked = with_wordmarks("piped to /dev/null, then /dev/null.");
        assert!(marked.contains(">/</span>null"));
        assert_eq!(marked.matches("/dev").count(), 2);
        let kept = with_wordmarks(
            "/dev/nullable /dev/null/log a/dev/null /dev/null_1 /DEV/NULL https://example.com/dev/null @devnull",
        );
        assert!(!kept.contains("foreground="));
    }

    #[test]
    fn finds_hex_colors_and_skips_issue_numbers() {
        let colors = colors_in("color(#Fff), color(#dead) #0ff", false);
        assert_eq!(
            colors
                .iter()
                .map(|c| (c.value.as_str(), c.label.as_str()))
                .collect::<Vec<_>>(),
            [("Fff", "#Fff"), ("dead", "#dead"), ("0ff", "#0ff")]
        );
        assert!(colors_in("#123 is an issue", true).is_empty());
        assert!(colors_in("#fff #dead #ff8a4c", true).is_empty());
        assert!(colors_in("x#0ff", true).is_empty());
        assert!(colors_in("#0ff-x", true).is_empty());
        assert!(colors_in("#0f8a4c.com", true).is_empty());
        assert_eq!(
            colors_in(">#0ff", true)
                .iter()
                .map(|c| c.value.as_str())
                .collect::<Vec<_>>(),
            ["0ff"]
        );
        assert!(colors_in(">#0ff", false).is_empty());
        let marked = with_colors("color(#ff8a4c)");
        assert!(marked.contains("foreground=\"#ff8a4c\""));
        assert!(marked.contains("■"));
        assert!(marked.contains("#ff8a4c"));
        assert!(!marked.contains("color("));
        assert_eq!(
            colors_in("#000, #000000 and #333333", false)
                .iter()
                .map(|c| c.value.as_str())
                .collect::<Vec<_>>(),
            ["000", "000000", "333333"]
        );
        assert!(colors_in("＃ff8a4c", false)
            .iter()
            .any(|c| c.value == "ff8a4c"));
    }

    #[test]
    fn links_hashtags_and_keeps_explicit_colors_as_swatches() {
        let marked = with_mentions(
            "shipping #rust and #dead next to color(#cafe) and #123",
            CapturedMentions::default(),
        );
        assert!(marked.contains("href=\"hashtag:#rust\""));
        assert!(marked.contains("underline=\"none\">#rust</span></a>"));
        assert!(marked.contains("href=\"hashtag:#dead\""));
        assert!(marked.contains("foreground=\"#ccaaff\">■</span>#cafe"));
        assert!(!marked.contains("color(#cafe)"));
        assert!(!marked.contains("href=\"hashtag:#123\""));
        assert!(!marked.contains("href=\"hashtag:#\""));
    }

    #[test]
    fn wraps_an_ambiguous_draft_hashtag_as_an_explicit_color() {
        let (body, caret) = apply_color_swatch_hint("try #dead next").unwrap();
        assert_eq!(body, "try color(#dead) next");
        assert_eq!(caret, "try color(#dead)".len());
        assert!(ambiguous_color_in_draft("try color(#dead) next").is_none());
        assert!(ambiguous_color_in_draft("no colors here").is_none());
    }

    #[test]
    fn finds_mentions_lowercased_once_each_in_order() {
        assert_eq!(
            mention_usernames("@NunoMaduro and @taylorotwell, thanks! cc @nunomaduro"),
            ["nunomaduro", "taylorotwell"]
        );
    }

    #[test]
    fn finds_mentions_next_to_punctuation() {
        assert_eq!(mention_usernames("Thanks @nuno."), ["nuno"]);
        assert_eq!(mention_usernames("Right, @nuno?"), ["nuno"]);
        assert_eq!(mention_usernames("(with @nuno)"), ["nuno"]);
        assert_eq!(mention_usernames("Shipped\n@nuno"), ["nuno"]);
        assert_eq!(mention_usernames("Hi @jess-archer"), ["jess-archer"]);
        assert_eq!(mention_usernames("@nuno: look"), ["nuno"]);
    }

    #[test]
    fn ignores_what_only_looks_like_a_mention() {
        for text in [
            "Mail me at nuno@laravel.com",
            "npm i @inertiajs/react",
            "Try @vue.js",
            "See medium.com/@nuno",
            "Meet @ 5pm",
            "@-nuno",
            "@nuno-",
            "@@nuno",
            &format!("@{}", "a".repeat(40)),
        ] {
            assert!(
                mention_usernames(text).is_empty(),
                "expected no mentions in {text:?}"
            );
        }
    }

    #[test]
    fn allows_usernames_of_up_to_39_characters() {
        let name = "a".repeat(39);
        assert_eq!(mention_usernames(&format!("@{name}")), [name]);
    }

    #[test]
    fn links_captured_mentions_and_keeps_unknown_plain() {
        let id = "019a2b3c-4d5e-7f60-8a9b-0c1d2e3f4a5b";
        let mentions = HashMap::from([("original".into(), id.into())]);
        let marked = with_mentions(
            "Thanks @ORIGINAL and @futuredev.",
            CapturedMentions::users_only(Some(&mentions)),
        );
        assert!(marked.contains("href=\"mention:original\""));
        assert!(marked.contains("underline=\"none\">@ORIGINAL</span></a>"));
        assert!(marked.contains("@futuredev"));
        assert_eq!(marked.matches("href=\"mention:").count(), 1);
    }

    #[test]
    fn links_usernames_when_mentions_were_not_captured() {
        let marked = with_mentions(
            "Thanks @ORIGINAL and @futuredev.",
            CapturedMentions::default(),
        );
        assert!(marked.contains("href=\"mention:original\""));
        assert!(marked.contains("underline=\"none\">@ORIGINAL</span></a>"));
        assert!(marked.contains("href=\"mention:futuredev\""));
        assert!(marked.contains("underline=\"none\">@futuredev</span></a>"));
        assert_eq!(marked.matches("href=\"mention:").count(), 2);
    }

    #[test]
    fn skips_mentions_in_code() {
        let mentions = HashMap::from([
            ("taylorotwell".into(), "t".into()),
            ("override".into(), "o".into()),
            ("deprecated".into(), "d".into()),
        ]);
        let marked = render_markup(
            "thanks @taylorotwell for `@Override`\n```java\n@Deprecated\nvoid run() {}\n```",
            CapturedMentions::users_only(Some(&mentions)),
            false,
        );
        assert!(marked.contains("href=\"mention:taylorotwell\""));
        assert!(!marked.contains("mention:override"));
        assert!(!marked.contains("mention:deprecated"));
        assert!(marked.contains("@Override"));
    }

    #[test]
    fn links_organization_mentions_apart_from_developers() {
        let users = HashMap::from([("nuno".into(), "u1".into())]);
        let organizations = HashMap::from([("laravel".into(), "o1".into())]);
        let marked = with_mentions(
            "Shipped with @nuno at @Laravel and @unknown.",
            CapturedMentions {
                users: Some(&users),
                organizations: Some(&organizations),
            },
        );
        assert!(marked.contains("href=\"mention:nuno\""));
        assert!(marked.contains("href=\"org:laravel\""));
        assert!(marked.contains("@unknown"));
        assert!(!marked.contains("href=\"mention:unknown\""));
        assert!(!marked.contains("href=\"org:unknown\""));
    }

    #[test]
    fn mention_trigger_reads_the_handle_before_the_caret() {
        assert_eq!(mention_trigger_at("@"), Some((0, String::new())));
        assert_eq!(mention_trigger_at("hi @Lara"), Some((3, "Lara".into())));
        assert_eq!(mention_trigger_at("email@host"), None);
        assert_eq!(mention_trigger_at("see laravel"), None);
        assert_eq!(mention_trigger_at("hey @lara-"), Some((4, "lara-".into())));
    }

    #[test]
    fn short_bodies_have_no_excerpt() {
        assert_eq!(excerpt("It was DNS."), None);
        assert_eq!(excerpt(&"a".repeat(280)), None);
        assert_eq!(excerpt(&"🚀".repeat(280)), None);
        assert_eq!(
            excerpt(
                &(1..=8)
                    .map(|n| n.to_string())
                    .collect::<Vec<_>>()
                    .join("\n")
            ),
            None
        );
    }

    #[test]
    fn cuts_a_long_body_at_the_last_whitespace_that_fits() {
        assert_eq!(
            excerpt(&format!("{}overflowing words", "word ".repeat(56))),
            Some(format!("{}…", "word ".repeat(56).trim_end()))
        );
        assert_eq!(
            excerpt(&format!("{} more", "a".repeat(280))),
            Some(format!("{}…", "a".repeat(280)))
        );
        assert_eq!(
            excerpt(
                &(1..=12)
                    .map(|n| n.to_string())
                    .collect::<Vec<_>>()
                    .join("\n")
            ),
            Some(format!(
                "{}…",
                (1..=8)
                    .map(|n| n.to_string())
                    .collect::<Vec<_>>()
                    .join("\n")
            ))
        );
        assert_eq!(
            excerpt(&"é ".repeat(141)),
            Some(format!("{}…", "é ".repeat(140).trim_end()))
        );
    }

    #[test]
    fn cuts_a_word_too_long_only_when_nothing_comes_before_it() {
        assert_eq!(
            excerpt(&format!("https://example.com/{}", "a".repeat(300))),
            Some(format!("https://example.com/{}…", "a".repeat(260)))
        );
        assert_eq!(
            excerpt(&format!("`make`{}", "a".repeat(300))),
            Some("`make`…".into())
        );
    }

    #[test]
    fn ends_the_excerpt_before_code_that_does_not_fit() {
        assert_eq!(
            excerpt(&format!(
                "Look at this:\n```php\n{}```",
                "echo 1;\n".repeat(40)
            )),
            Some("Look at this:…".into())
        );
        let block = format!(
            "One\nTwo\n```\n{}\n```",
            (1..=10)
                .map(|n| n.to_string())
                .collect::<Vec<_>>()
                .join("\n")
        );
        assert_eq!(excerpt(&block), Some("One\nTwo…".into()));
        assert_eq!(
            excerpt(&format!("{} `composer install`", "a".repeat(270))),
            Some(format!("{}…", "a".repeat(270)))
        );
    }

    #[test]
    fn keeps_the_first_lines_of_a_code_block_the_body_starts_with() {
        let echo = |n: u32| format!("echo {n};");
        let body = format!(
            "```php\n{}\n```\nNeat.",
            (1..=12).map(echo).collect::<Vec<_>>().join("\n")
        );
        let preview = format!(
            "```php\n{}\n```\n…",
            (1..=6).map(echo).collect::<Vec<_>>().join("\n")
        );
        assert_eq!(excerpt(&body), Some(preview));
        assert_eq!(
            excerpt(&format!(
                "```\n{}\n{}\n```",
                "a".repeat(200),
                "b".repeat(200)
            )),
            Some(format!("```\n{}\n```\n…", "a".repeat(200)))
        );
        assert_eq!(
            excerpt(&format!("```\n{}\n```", "a".repeat(400))),
            Some(format!("```\n{}\n```\n…", "a".repeat(272)))
        );
        assert_eq!(
            excerpt(&format!("````\n{}````", "line\n".repeat(20))),
            Some(format!("````\n{}\n````\n…", "line\n".repeat(6).trim_end()))
        );
        assert_eq!(
            excerpt(&format!("```\r\n{}```", "line\r\n".repeat(60))),
            Some(format!("```\n{}\n```\n…", "line\n".repeat(6).trim_end()))
        );
        assert_eq!(
            excerpt(&format!(
                "```\nshort\n```\nhttps://example.com/{}",
                "a".repeat(300)
            )),
            Some("```\nshort\n```\n…".into())
        );
        assert_eq!(
            excerpt(&format!("  \n```\n{}```", "line\n".repeat(20))),
            Some(format!(
                "  \n```\n{}\n```\n…",
                "line\n".repeat(5).trim_end()
            ))
        );
    }

    #[test]
    fn cuts_inline_code_short_when_the_body_starts_with_it() {
        assert_eq!(
            excerpt(&format!("`{}` done", "a".repeat(300))),
            Some(format!("`{}`…", "a".repeat(278)))
        );
    }

    #[test]
    fn copies_the_whole_code_block_from_a_folded_preview() {
        let echo = |n: u32| format!("echo {n};");
        let inner = (1..=12).map(echo).collect::<Vec<_>>().join("\n");
        let body = format!("```php\n{inner}\n```\nNeat.");
        let preview = excerpt(&body).expect("long code folds");
        let shown = copy_source_for_block(&preview, &preview, 0).expect("preview block");
        assert!(shown.lines().count() < 12);
        assert_eq!(
            copy_source_for_block(&preview, &body, 0).as_deref(),
            Some(inner.as_str())
        );
    }

    #[test]
    fn splits_the_excerpt_of_a_folded_body_into_the_same_markup() {
        let short = "run `make`";
        assert_eq!(excerpt(short), None);
        let long = format!("run `make` {}", "again ".repeat(60));
        let preview = excerpt(&long).expect("long body folds");
        let segs = segments(&preview);
        assert!(matches!(&segs[0], Segment::Text(t) if t == "run "));
        assert!(matches!(&segs[1], Segment::Code(c) if c == "make"));
        assert!(matches!(&segs[2], Segment::Text(t) if t.ends_with('…')));
    }

    #[test]
    fn drops_the_ellipsis_after_a_faded_code_block() {
        let inner = (1..=12)
            .map(|n| format!("echo {n};"))
            .collect::<Vec<_>>()
            .join("\n");
        let body = format!("```php\n{inner}\n```\nNeat.");
        let preview = excerpt(&body).expect("long code folds");
        let sources = block_sources(&body);
        let segs = without_fade_ellipsis(segments(&preview), &sources);
        assert!(matches!(segs.last(), Some(Segment::Block { .. })));
        assert!(!matches!(
            segs.last(),
            Some(Segment::Text(text)) if text.trim() == "…"
        ));
    }
}
