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

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Segment {
    Text(String),
    Code(String),
    Block {
        language: Option<String>,
        code: String,
    },
}

pub fn body_widget(
    text: &str,
    class: &str,
    mentions: Option<&HashMap<String, String>>,
    sender: Option<&Sender<AppMsg>>,
) -> gtk::Widget {
    let sender = sender.cloned();
    let wrap = gtk::Box::new(gtk::Orientation::Vertical, 8);
    for class in class.split_whitespace() {
        wrap.add_css_class(class);
    }
    wrap.set_hexpand(true);
    let mut prose = String::new();
    let flush = |prose: &mut String, wrap: &gtk::Box| {
        if prose.is_empty() {
            return;
        }
        wrap.append(&prose_label(prose, sender.as_ref()));
        prose.clear();
    };
    for segment in segments(text) {
        match segment {
            Segment::Text(text) => prose.push_str(&markdown_markup(&text, mentions)),
            Segment::Code(code) => {
                let chip = if is_dark() { "#232326" } else { "#eef0f2" };
                prose.push_str(&format!(
                    "<span font_family=\"monospace\" background=\"{chip}\"> "
                ));
                prose.push_str(&escape(&code));
                prose.push_str(" </span>");
            }
            Segment::Block { language, code } => {
                flush(&mut prose, &wrap);
                wrap.append(&code_block(language.as_deref(), &code));
            }
        }
    }
    flush(&mut prose, &wrap);
    wrap.upcast()
}

pub fn body_markup(text: &str) -> String {
    render_markup(text, None)
}

fn render_markup(text: &str, mentions: Option<&HashMap<String, String>>) -> String {
    let mut out = String::new();
    for (index, segment) in segments(text).into_iter().enumerate() {
        if index > 0 && !out.ends_with('\n') {
            match segment {
                Segment::Block { .. } | Segment::Text(_) => out.push('\n'),
                Segment::Code(_) => {}
            }
        }
        match segment {
            Segment::Text(text) => out.push_str(&markdown_markup(&text, mentions)),
            Segment::Code(code) => {
                out.push_str("<tt>");
                out.push_str(&escape(&code));
                out.push_str("</tt>");
            }
            Segment::Block { language, code } => {
                out.push_str(&highlight_pango(&code, language.as_deref()));
            }
        }
    }
    out
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

fn with_mentions(text: &str, mentions: Option<&HashMap<String, String>>) -> String {
    let Some(mentions) = mentions.filter(|map| !map.is_empty()) else {
        return with_wordmarks(&with_colors(&escape(text)));
    };
    let mut out = String::new();
    let mut last = 0;
    for mention in mention_spans(text) {
        out.push_str(&with_wordmarks(&with_colors(&escape(
            &text[last..mention.start],
        ))));
        let token = &text[mention.start..mention.end];
        if captured_mention(mentions, &mention.username).is_some() {
            out.push_str(&format!(
                "<a href=\"mention:{}\">{}</a>",
                escape(&mention.username),
                escape(token)
            ));
        } else {
            out.push_str(&with_wordmarks(&with_colors(&escape(token))));
        }
        last = mention.end;
    }
    out.push_str(&with_wordmarks(&with_colors(&escape(&text[last..]))));
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
        if b.is_ascii_alphanumeric() {
            i += 1;
            n += 1;
        } else if b == b'-'
            && n + 1 <= 39
            && bytes
                .get(i + 1)
                .is_some_and(|next| next.is_ascii_alphanumeric())
        {
            i += 1;
            n += 1;
        } else {
            break;
        }
    }
    Some(&s[..i])
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
    if next == '.' {
        if chars
            .next()
            .is_some_and(|after| after.is_ascii_alphanumeric())
        {
            return false;
        }
    }
    true
}

fn with_colors(text: &str) -> String {
    let mut out = String::new();
    let mut last = 0;
    for color in colors_in(text, false) {
        out.push_str(&text[last..color.index]);
        let token = &text[color.index..color.index + color.length];
        if let Some(css) = css_hex(&color.value) {
            out.push_str(&format!("<span foreground=\"{css}\">■</span>{token}"));
        } else {
            out.push_str(token);
        }
        last = color.index + color.length;
    }
    out.push_str(&text[last..]);
    out
}

/// Hex colors in prose: 3, 4, 6 or 8 digits, a whole word like a hashtag.
/// Short forms need a letter or a leading zero so `#123` stays an issue reference.
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
        let Some(next) = hay[i..].chars().next() else {
            break;
        };
        let hash_len = match next {
            '#' => 1,
            '\u{FF03}' => next.len_utf8(),
            _ => {
                i += next.len_utf8();
                continue;
            }
        };
        if !color_left_ok(&hay, i) {
            i += next.len_utf8();
            continue;
        }
        let hex_at = i + hash_len;
        let Some((digits, hex_len)) = take_color_digits(&hay[hex_at..]) else {
            i += next.len_utf8();
            continue;
        };
        let end = hex_at + hex_len;
        if !color_right_ok(&hay[end..]) {
            i += next.len_utf8();
            continue;
        }
        let index = i.saturating_sub(shift);
        out.push(ColorSpan {
            value: digits,
            index,
            length: hash_len + hex_len,
        });
        i = end;
    }
    out
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
        Some('>') => chars.next().is_none_or(|c| c == '\n'),
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
                let language = header
                    .split(|c: char| c.is_whitespace())
                    .find(|word| !word.is_empty())
                    .map(ToOwned::to_owned);
                let end = if j < chars.len() { j + 1 } else { j };
                return Some((Segment::Block { language, code }, end));
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

fn is_closing_fence(line: &[char], _open_ticks: usize) -> bool {
    let mut ticks = 0;
    while ticks < line.len() && line[ticks] == '`' {
        ticks += 1;
    }
    ticks >= 3 && line[ticks..].iter().all(|c| *c == ' ' || *c == '\t')
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

fn markdown_markup(text: &str, mentions: Option<&HashMap<String, String>>) -> String {
    let mut out = String::new();
    let mut in_link = false;
    let mut options = Options::empty();
    options.insert(Options::ENABLE_STRIKETHROUGH);
    let parser = Parser::new_ext(text, options);
    for event in parser {
        match event {
            Event::Start(Tag::Emphasis) => out.push_str("<i>"),
            Event::End(TagEnd::Emphasis) => out.push_str("</i>"),
            Event::Start(Tag::Strong) => out.push_str("<b>"),
            Event::End(TagEnd::Strong) => out.push_str("</b>"),
            Event::Start(Tag::CodeBlock(_)) => out.push_str("<tt>"),
            Event::End(TagEnd::CodeBlock) => out.push_str("</tt>"),
            Event::Code(code) => {
                out.push_str("<tt>");
                out.push_str(&escape(&code));
                out.push_str("</tt>");
            }
            Event::Text(t) | Event::Html(t) | Event::InlineHtml(t) => {
                if in_link {
                    out.push_str(&with_wordmarks(&with_colors(&escape(&t))));
                } else {
                    out.push_str(&with_mentions(&t, mentions));
                }
            }
            Event::SoftBreak | Event::HardBreak => out.push('\n'),
            Event::Start(Tag::Link { dest_url, .. }) => {
                in_link = true;
                out.push_str("<a href=\"");
                out.push_str(&escape(&dest_url));
                out.push_str("\">");
            }
            Event::End(TagEnd::Link) => {
                in_link = false;
                out.push_str("</a>");
            }
            Event::Rule => out.push('\n'),
            _ => {}
        }
    }
    out
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
            if let Some(username) = uri.strip_prefix("mention:") {
                sender.emit(AppMsg::OpenProfile(username.to_string()));
            } else {
                sender.emit(AppMsg::OpenWeb(uri.to_string()));
            }
            glib::Propagation::Stop
        });
    }
    label
}

fn code_block(language: Option<&str>, code: &str) -> gtk::Widget {
    let wrap = gtk::Box::new(gtk::Orientation::Vertical, 0);
    wrap.add_css_class("dn-code");
    wrap.set_overflow(gtk::Overflow::Hidden);
    wrap.set_hexpand(true);
    let header = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    header.add_css_class("dn-code-header");
    let lang = gtk::Label::new(language);
    lang.add_css_class("dn-mono");
    lang.add_css_class("dn-muted");
    lang.set_xalign(0.0);
    lang.set_hexpand(true);
    header.append(&lang);
    let copy = gtk::Button::new();
    copy.set_icon_name("dn-copy-symbolic");
    copy.set_tooltip_text(Some("Copy"));
    copy.add_css_class("flat");
    copy.add_css_class("dn-code-copy");
    let copied = code.to_owned();
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
    wrap.append(&body);
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
            matches!(&segs[3], Segment::Block { language, code } if language.as_deref() == Some("rust") && code == "let x = 1;")
        );
        assert!(matches!(&segs[4], Segment::Text(t) if t == "done"));
    }

    #[test]
    fn unclosed_fence_stays_prose() {
        let segs = segments("```php\n$x = 1;");
        assert!(matches!(&segs[..], [Segment::Text(t)] if t.contains("```php")));
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
        let colors = colors_in("#fff then #000 and #ff8a4c", true);
        assert_eq!(
            colors.iter().map(|c| c.value.as_str()).collect::<Vec<_>>(),
            ["fff", "000", "ff8a4c"]
        );
        assert!(colors_in("#123 is an issue", true).is_empty());
        assert!(colors_in("x#fff", true).is_empty());
        assert!(colors_in("#fff-x", true).is_empty());
        assert!(colors_in("#ff8a4c.com", true).is_empty());
        assert_eq!(
            colors_in(">#abc", true)
                .iter()
                .map(|c| c.value.as_str())
                .collect::<Vec<_>>(),
            ["abc"]
        );
        let marked = with_colors("#ff8a4c");
        assert!(marked.contains("foreground=\"#ff8a4c\""));
        assert!(marked.contains("■"));
        assert!(marked.contains("#ff8a4c"));
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
        let marked = with_mentions("Thanks @ORIGINAL and @futuredev.", Some(&mentions));
        assert!(marked.contains("href=\"mention:original\""));
        assert!(marked.contains(">@ORIGINAL</a>"));
        assert!(marked.contains("@futuredev"));
        assert_eq!(marked.matches("href=\"mention:").count(), 1);
        assert_eq!(
            with_mentions("Thanks @original!", None),
            "Thanks @original!"
        );
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
            Some(&mentions),
        );
        assert!(marked.contains("href=\"mention:taylorotwell\""));
        assert!(!marked.contains("mention:override"));
        assert!(!marked.contains("mention:deprecated"));
        assert!(marked.contains("@Override"));
    }
}
