use pulldown_cmark::{Event, Options, Parser, Tag, TagEnd};
use relm4::gtk;
use relm4::gtk::pango;
use relm4::gtk::prelude::*;
use std::sync::OnceLock;
use syntect::easy::HighlightLines;
use syntect::highlighting::{FontStyle, ThemeSet};
use syntect::parsing::SyntaxSet;
use syntect::util::LinesWithEndings;

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

pub fn body_widget(text: &str, class: &str) -> gtk::Widget {
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
        wrap.append(&prose_label(prose));
        prose.clear();
    };
    for segment in segments(text) {
        match segment {
            Segment::Text(text) => prose.push_str(&markdown_markup(&text)),
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
    let mut out = String::new();
    for (index, segment) in segments(text).into_iter().enumerate() {
        if index > 0 && !out.ends_with('\n') {
            match segment {
                Segment::Block { .. } | Segment::Text(_) => out.push('\n'),
                Segment::Code(_) => {}
            }
        }
        match segment {
            Segment::Text(text) => out.push_str(&markdown_markup(&text)),
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

fn markdown_markup(text: &str) -> String {
    let mut out = String::new();
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
                out.push_str(&with_wordmarks(&escape(&t)));
            }
            Event::SoftBreak | Event::HardBreak => out.push('\n'),
            Event::Start(Tag::Link { dest_url, .. }) => {
                out.push_str("<a href=\"");
                out.push_str(&escape(&dest_url));
                out.push_str("\">");
            }
            Event::End(TagEnd::Link) => out.push_str("</a>"),
            Event::Rule => out.push('\n'),
            _ => {}
        }
    }
    out
}

fn prose_label(markup: &str) -> gtk::Label {
    let label = gtk::Label::new(None);
    label.set_wrap(true);
    label.set_wrap_mode(pango::WrapMode::WordChar);
    label.set_xalign(0.0);
    label.set_selectable(true);
    label.set_use_markup(true);
    label.set_markup(markup);
    label
}

fn code_block(language: Option<&str>, code: &str) -> gtk::Widget {
    let wrap = gtk::Box::new(gtk::Orientation::Vertical, 0);
    wrap.add_css_class("dn-code");
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
}
