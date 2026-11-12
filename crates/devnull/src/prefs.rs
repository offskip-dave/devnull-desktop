use std::collections::HashMap;
use std::sync::Mutex;

use chrono::{Datelike, Local, NaiveDate};
use relm4::adw;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum Theme {
    Light,
    Dark,
    #[default]
    System,
}

impl Theme {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::System => "system",
            Self::Light => "light",
            Self::Dark => "dark",
        }
    }

    fn color_scheme(self) -> adw::ColorScheme {
        match self {
            Self::System => adw::ColorScheme::Default,
            Self::Light => adw::ColorScheme::ForceLight,
            Self::Dark => adw::ColorScheme::ForceDark,
        }
    }
}

/// What the reader picked in Appearance. Auto follows the calendar where they are.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum EventThemeChoice {
    #[default]
    Auto,
    #[serde(rename = "none")]
    None,
    Halloween,
    Christmas,
    #[serde(rename = "newyear")]
    NewYear,
}

impl EventThemeChoice {
    pub fn index(self) -> u32 {
        match self {
            Self::Auto => 0,
            Self::None => 1,
            Self::Halloween => 2,
            Self::Christmas => 3,
            Self::NewYear => 4,
        }
    }

    pub fn from_index(index: u32) -> Self {
        match index {
            1 => Self::None,
            2 => Self::Halloween,
            3 => Self::Christmas,
            4 => Self::NewYear,
            _ => Self::Auto,
        }
    }

    fn forced(self) -> Option<EventTheme> {
        match self {
            Self::Halloween => Some(EventTheme::Halloween),
            Self::Christmas => Some(EventTheme::Christmas),
            Self::NewYear => Some(EventTheme::NewYear),
            Self::Auto | Self::None => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EventTheme {
    Halloween,
    Christmas,
    NewYear,
}

/// The theme to show: "none" picks no theme, a named choice forces it, auto follows the date.
/// Each theme runs from two days before the day to the day after it.
pub fn event_for(choice: EventThemeChoice, date: NaiveDate) -> Option<EventTheme> {
    if choice == EventThemeChoice::None {
        return None;
    }
    choice.forced().or_else(|| event_on(date))
}

fn event_on(date: NaiveDate) -> Option<EventTheme> {
    let month_day = date.month() * 100 + date.day();
    if (1029..=1101).contains(&month_day) {
        Some(EventTheme::Halloween)
    } else if (1223..=1226).contains(&month_day) {
        Some(EventTheme::Christmas)
    } else if month_day >= 1230 || month_day <= 102 {
        Some(EventTheme::NewYear)
    } else {
        None
    }
}

/// The year a New Year greeting wishes: the coming one through December, then the current one.
pub fn greeted_year_on(date: NaiveDate) -> i32 {
    if date.month() == 12 {
        date.year() + 1
    } else {
        date.year()
    }
}

static EVENT_CHOICE: Mutex<EventThemeChoice> = Mutex::new(EventThemeChoice::Auto);

fn publish_event(choice: EventThemeChoice) {
    if let Ok(mut slot) = EVENT_CHOICE.lock() {
        *slot = choice;
    }
}

fn published_event() -> Option<EventTheme> {
    let choice = EVENT_CHOICE.lock().map(|slot| *slot).unwrap_or_default();
    event_for(choice, Local::now().date_naive())
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Prefs {
    #[serde(default)]
    pub theme: Theme,
    #[serde(default = "default_true")]
    pub show_who_to_follow: bool,
    #[serde(default = "default_true")]
    pub highlight_on_hover: bool,
    #[serde(default)]
    pub last_visits: HashMap<String, String>,
    #[serde(default)]
    pub chat_docked: bool,
    #[serde(default)]
    pub pinned_remotes: Vec<String>,
    #[serde(default)]
    pub pinned_messages: HashMap<String, i64>,
    #[serde(default)]
    pub pin_warning_dismissed: bool,
    #[serde(default)]
    #[allow(dead_code)]
    pub message_reactions: HashMap<String, Vec<String>>,
    /// Last three search queries per user id, newest first.
    #[serde(default)]
    pub recent_searches: HashMap<String, Vec<String>>,
    /// Auto follows Halloween, Christmas, and New Year where this computer is.
    #[serde(default)]
    pub event_theme: EventThemeChoice,
    /// Domains that skip the leave-site warning for external links.
    #[serde(default)]
    pub trusted_external_domains: Vec<String>,
}

fn default_true() -> bool {
    true
}

impl Default for Prefs {
    fn default() -> Self {
        Self {
            theme: Theme::System,
            show_who_to_follow: true,
            highlight_on_hover: true,
            last_visits: HashMap::new(),
            chat_docked: false,
            pinned_remotes: Vec::new(),
            pinned_messages: HashMap::new(),
            pin_warning_dismissed: false,
            message_reactions: HashMap::new(),
            recent_searches: HashMap::new(),
            event_theme: EventThemeChoice::Auto,
            trusted_external_domains: Vec::new(),
        }
    }
}

impl Prefs {
    pub fn load() -> Self {
        let prefs: Self = std::fs::read(path())
            .ok()
            .and_then(|bytes| serde_json::from_slice(&bytes).ok())
            .unwrap_or_default();
        prefs.apply();
        prefs
    }

    pub fn save(&self) {
        self.apply();
        self.persist();
    }

    pub fn persist(&self) {
        let Some(dir) = path().parent().map(ToOwned::to_owned) else {
            return;
        };
        let _ = std::fs::create_dir_all(dir);
        if let Ok(bytes) = serde_json::to_vec_pretty(self) {
            let _ = std::fs::write(path(), bytes);
        }
    }

    pub fn visit_key(user_id: &str, log: &str) -> String {
        format!("{user_id}:{log}")
    }

    pub fn last_visit(&self, user_id: &str, log: &str) -> Option<&str> {
        self.last_visits
            .get(&Self::visit_key(user_id, log))
            .map(String::as_str)
            .filter(|value| !value.is_empty())
    }

    pub fn remember_visit(&mut self, user_id: &str, log: &str, at: String) {
        self.last_visits.insert(Self::visit_key(user_id, log), at);
        self.persist();
    }

    pub fn recent_searches(&self, user_id: &str) -> Vec<String> {
        self.recent_searches
            .get(user_id)
            .cloned()
            .unwrap_or_default()
    }

    pub fn remember_search(&mut self, user_id: &str, query: &str) {
        let term = query.trim();
        if term.is_empty() || term.len() > 100 {
            return;
        }
        let recent = self.recent_searches.entry(user_id.to_owned()).or_default();
        recent.retain(|known| !known.eq_ignore_ascii_case(term));
        recent.insert(0, term.to_owned());
        recent.truncate(3);
        self.persist();
    }

    pub fn clear_recent_searches(&mut self, user_id: &str) {
        self.recent_searches.remove(user_id);
        self.persist();
    }

    pub fn is_remote_pinned(&self, key: &str) -> bool {
        self.pinned_remotes.iter().any(|known| known == key)
    }

    pub fn toggle_remote_pin(&mut self, key: &str, message_id: Option<i64>) -> bool {
        if let Some(index) = self.pinned_remotes.iter().position(|known| known == key) {
            self.pinned_remotes.remove(index);
            self.pinned_messages.remove(key);
            self.save();
            false
        } else {
            self.pinned_remotes.push(key.to_owned());
            if let Some(id) = message_id {
                self.pinned_messages.insert(key.to_owned(), id);
            }
            self.save();
            true
        }
    }

    pub fn dismiss_pin_warning(&mut self) {
        if self.pin_warning_dismissed {
            return;
        }
        self.pin_warning_dismissed = true;
        self.persist();
    }

    pub fn trusts_external_domain(&self, domain: &str) -> bool {
        let domain = domain.to_ascii_lowercase();
        self.trusted_external_domains
            .iter()
            .any(|known| known.eq_ignore_ascii_case(&domain))
    }

    pub fn trust_external_domain(&mut self, domain: &str) {
        let domain = domain.trim();
        if domain.is_empty() || self.trusts_external_domain(domain) {
            return;
        }
        self.trusted_external_domains.push(domain.to_owned());
        self.persist();
    }

    pub fn apply(&self) {
        publish_event(self.event_theme);
        adw::StyleManager::default().set_color_scheme(self.theme.color_scheme());
        reload_css();
    }

    pub fn active_event(&self) -> Option<EventTheme> {
        event_for(self.event_theme, Local::now().date_naive())
    }

    pub fn greeted_year(&self) -> i32 {
        greeted_year_on(Local::now().date_naive())
    }
}

/// GTK `@define-color` is global and `@media (prefers-color-scheme)` follows the
/// OS, not libadwaita's `ColorScheme`. Inject dark tokens when the app is dark.
const DARK_TOKENS: &str = r#"
@define-color dn_shell #111112;
@define-color dn_surface #18181a;
@define-color dn_ink #ececec;
@define-color dn_ink_muted #a1a1a6;
@define-color dn_hairline #2a2a2d;
@define-color dn_rail #36363a;
@define-color dn_wash #232326;
@define-color dn_primary #7aa2f7;
@define-color dn_primary_hover #94b5f9;
@define-color dn_primary_soft #18233a;
@define-color dn_primary_ink #111112;
@define-color dn_destructive #ff6b6b;
@define-color dn_lane_0 #ff8a4c;
@define-color dn_lane_1 #51cf66;
@define-color dn_lane_2 #da77f2;
@define-color dn_lane_3 #3bc9db;
@define-color dn_lane_4 #f06595;
@define-color dn_lane_5 #fcc419;
@define-color window_bg_color #111112;
@define-color window_fg_color #ececec;
@define-color view_bg_color #18181a;
@define-color view_fg_color #ececec;
@define-color headerbar_bg_color #111112;
@define-color headerbar_fg_color #ececec;
@define-color headerbar_border_color #2a2a2d;
@define-color sidebar_bg_color #111112;
@define-color sidebar_fg_color #ececec;
@define-color card_bg_color #18181a;
@define-color card_fg_color #ececec;
@define-color popover_bg_color #1f1f22;
@define-color popover_fg_color #ececec;
@define-color dialog_bg_color #18181a;
@define-color dialog_fg_color #ececec;
@define-color borders #2a2a2d;
@define-color accent_bg_color #7aa2f7;
@define-color accent_fg_color #111112;
@define-color accent_color #7aa2f7;
@define-color destructive_color #ff6b6b;
"#;

pub fn reload_css() {
    let dark = adw::StyleManager::default().is_dark();
    let mut css = gtkify_css(include_str!("style.css"));
    if dark {
        css.push_str(DARK_TOKENS);
    }
    if let Some(theme) = published_event() {
        css.push_str(event_token_css(theme, dark));
    }
    relm4::set_global_css(&css);
}

/// Christmas contribution graph: green days, red for the busiest ones.
pub fn christmas_graph_hex(dark: bool) -> Option<(&'static str, &'static str)> {
    if published_event() == Some(EventTheme::Christmas) {
        if dark {
            Some(("#51cf66", "#ff6b6b"))
        } else {
            Some(("#2f9e44", "#c92a2a"))
        }
    } else {
        None
    }
}

fn event_token_css(theme: EventTheme, dark: bool) -> &'static str {
    match (theme, dark) {
        (EventTheme::Halloween, false) => {
            r#"
@define-color dn_shell #fbf5ee;
@define-color dn_surface #fffdf9;
@define-color dn_ink #1f1710;
@define-color dn_ink_muted #6b5a4a;
@define-color dn_hairline #eadccd;
@define-color dn_rail #dccbb8;
@define-color dn_wash #fdeadb;
@define-color dn_primary #c2410c;
@define-color dn_primary_hover #9a3412;
@define-color dn_primary_soft #fdeadb;
@define-color dn_primary_ink #ffffff;
@define-color window_bg_color #fbf5ee;
@define-color window_fg_color #1f1710;
@define-color view_bg_color #fffdf9;
@define-color view_fg_color #1f1710;
@define-color headerbar_bg_color #fbf5ee;
@define-color headerbar_fg_color #1f1710;
@define-color headerbar_border_color #eadccd;
@define-color sidebar_bg_color #fbf5ee;
@define-color sidebar_fg_color #1f1710;
@define-color card_bg_color #fffdf9;
@define-color card_fg_color #1f1710;
@define-color popover_bg_color #fffdf9;
@define-color popover_fg_color #1f1710;
@define-color dialog_bg_color #fffdf9;
@define-color dialog_fg_color #1f1710;
@define-color borders #eadccd;
@define-color accent_bg_color #c2410c;
@define-color accent_fg_color #ffffff;
@define-color accent_color #c2410c;
"#
        }
        (EventTheme::Halloween, true) => {
            r#"
@define-color dn_shell #120c14;
@define-color dn_surface #1a1220;
@define-color dn_ink #f1e9f5;
@define-color dn_ink_muted #ae9fb8;
@define-color dn_hairline #2c2033;
@define-color dn_rail #3a2c44;
@define-color dn_wash #2e1a12;
@define-color dn_primary #ff8a4c;
@define-color dn_primary_hover #ffa270;
@define-color dn_primary_soft #2e1a12;
@define-color dn_primary_ink #120c14;
@define-color window_bg_color #120c14;
@define-color window_fg_color #f1e9f5;
@define-color view_bg_color #1a1220;
@define-color view_fg_color #f1e9f5;
@define-color headerbar_bg_color #120c14;
@define-color headerbar_fg_color #f1e9f5;
@define-color headerbar_border_color #2c2033;
@define-color sidebar_bg_color #120c14;
@define-color sidebar_fg_color #f1e9f5;
@define-color card_bg_color #1a1220;
@define-color card_fg_color #f1e9f5;
@define-color popover_bg_color #21162a;
@define-color popover_fg_color #f1e9f5;
@define-color dialog_bg_color #1a1220;
@define-color dialog_fg_color #f1e9f5;
@define-color borders #2c2033;
@define-color accent_bg_color #ff8a4c;
@define-color accent_fg_color #120c14;
@define-color accent_color #ff8a4c;
"#
        }
        (EventTheme::Christmas, false) => {
            r#"
@define-color dn_shell #f8f6f1;
@define-color dn_surface #fffefb;
@define-color dn_ink #161a17;
@define-color dn_ink_muted #5e5a50;
@define-color dn_hairline #e3e0d6;
@define-color dn_rail #d3cfc2;
@define-color dn_wash #fbe9e9;
@define-color dn_primary #b3262c;
@define-color dn_primary_hover #962024;
@define-color dn_primary_soft #fbe9e9;
@define-color dn_primary_ink #ffffff;
@define-color window_bg_color #f8f6f1;
@define-color window_fg_color #161a17;
@define-color view_bg_color #fffefb;
@define-color view_fg_color #161a17;
@define-color headerbar_bg_color #f8f6f1;
@define-color headerbar_fg_color #161a17;
@define-color headerbar_border_color #e3e0d6;
@define-color sidebar_bg_color #f8f6f1;
@define-color sidebar_fg_color #161a17;
@define-color card_bg_color #fffefb;
@define-color card_fg_color #161a17;
@define-color popover_bg_color #fffefb;
@define-color popover_fg_color #161a17;
@define-color dialog_bg_color #fffefb;
@define-color dialog_fg_color #161a17;
@define-color borders #e3e0d6;
@define-color accent_bg_color #b3262c;
@define-color accent_fg_color #ffffff;
@define-color accent_color #b3262c;
"#
        }
        (EventTheme::Christmas, true) => {
            r#"
@define-color dn_shell #0e1512;
@define-color dn_surface #141d19;
@define-color dn_ink #ecf1ee;
@define-color dn_ink_muted #9fb0a6;
@define-color dn_hairline #243029;
@define-color dn_rail #2f3d35;
@define-color dn_wash #2b1717;
@define-color dn_primary #ff7b7b;
@define-color dn_primary_hover #ff9494;
@define-color dn_primary_soft #2b1717;
@define-color dn_primary_ink #0e1512;
@define-color window_bg_color #0e1512;
@define-color window_fg_color #ecf1ee;
@define-color view_bg_color #141d19;
@define-color view_fg_color #ecf1ee;
@define-color headerbar_bg_color #0e1512;
@define-color headerbar_fg_color #ecf1ee;
@define-color headerbar_border_color #243029;
@define-color sidebar_bg_color #0e1512;
@define-color sidebar_fg_color #ecf1ee;
@define-color card_bg_color #141d19;
@define-color card_fg_color #ecf1ee;
@define-color popover_bg_color #1a2520;
@define-color popover_fg_color #ecf1ee;
@define-color dialog_bg_color #141d19;
@define-color dialog_fg_color #ecf1ee;
@define-color borders #243029;
@define-color accent_bg_color #ff7b7b;
@define-color accent_fg_color #0e1512;
@define-color accent_color #ff7b7b;
"#
        }
        (EventTheme::NewYear, false) => {
            r#"
@define-color dn_shell #faf7ef;
@define-color dn_surface #fffdf7;
@define-color dn_ink #1c1807;
@define-color dn_ink_muted #6b5f3e;
@define-color dn_hairline #ece3c8;
@define-color dn_rail #ddd1ae;
@define-color dn_wash #fbf0d2;
@define-color dn_primary #a16207;
@define-color dn_primary_hover #854d0e;
@define-color dn_primary_soft #fbf0d2;
@define-color dn_primary_ink #ffffff;
@define-color window_bg_color #faf7ef;
@define-color window_fg_color #1c1807;
@define-color view_bg_color #fffdf7;
@define-color view_fg_color #1c1807;
@define-color headerbar_bg_color #faf7ef;
@define-color headerbar_fg_color #1c1807;
@define-color headerbar_border_color #ece3c8;
@define-color sidebar_bg_color #faf7ef;
@define-color sidebar_fg_color #1c1807;
@define-color card_bg_color #fffdf7;
@define-color card_fg_color #1c1807;
@define-color popover_bg_color #fffdf7;
@define-color popover_fg_color #1c1807;
@define-color dialog_bg_color #fffdf7;
@define-color dialog_fg_color #1c1807;
@define-color borders #ece3c8;
@define-color accent_bg_color #a16207;
@define-color accent_fg_color #ffffff;
@define-color accent_color #a16207;
"#
        }
        (EventTheme::NewYear, true) => {
            r#"
@define-color dn_shell #0b0d17;
@define-color dn_surface #12152a;
@define-color dn_ink #eef0fa;
@define-color dn_ink_muted #9aa0c4;
@define-color dn_hairline #232744;
@define-color dn_rail #31365a;
@define-color dn_wash #2a2410;
@define-color dn_primary #f5c542;
@define-color dn_primary_hover #f8d56f;
@define-color dn_primary_soft #2a2410;
@define-color dn_primary_ink #0b0d17;
@define-color window_bg_color #0b0d17;
@define-color window_fg_color #eef0fa;
@define-color view_bg_color #12152a;
@define-color view_fg_color #eef0fa;
@define-color headerbar_bg_color #0b0d17;
@define-color headerbar_fg_color #eef0fa;
@define-color headerbar_border_color #232744;
@define-color sidebar_bg_color #0b0d17;
@define-color sidebar_fg_color #eef0fa;
@define-color card_bg_color #12152a;
@define-color card_fg_color #eef0fa;
@define-color popover_bg_color #191d38;
@define-color popover_fg_color #eef0fa;
@define-color dialog_bg_color #12152a;
@define-color dialog_fg_color #eef0fa;
@define-color borders #232744;
@define-color accent_bg_color #f5c542;
@define-color accent_fg_color #0b0d17;
@define-color accent_color #f5c542;
"#
        }
    }
}

/// Map CSS custom properties to GTK named colors so GTK 4.14 can load the
/// stylesheet. Custom properties need GTK 4.16+.
/// `:root { --token: value; }` becomes `@define-color token value;`
/// and `var(--token)` becomes `@token`.
fn gtkify_css(css: &str) -> String {
    gtkify_var_refs(&gtkify_root_tokens(css))
}

fn gtkify_root_tokens(css: &str) -> String {
    let mut out = String::with_capacity(css.len());
    let mut rest = css;
    while let Some(start) = find_root_block(rest) {
        out.push_str(&rest[..start]);
        let after_root = rest[start + 5..].trim_start();
        let body_and_rest = &after_root[1..];
        match body_and_rest.find('}') {
            Some(end) => {
                emit_define_colors(&mut out, &body_and_rest[..end]);
                rest = &body_and_rest[end + 1..];
            }
            None => {
                out.push_str(&rest[start..]);
                return out;
            }
        }
    }
    out.push_str(rest);
    out
}

fn find_root_block(css: &str) -> Option<usize> {
    let mut search = css;
    let mut offset = 0;
    while let Some(i) = search.find(":root") {
        if search[i + 5..].trim_start().starts_with('{') {
            return Some(offset + i);
        }
        offset += i + 5;
        search = &search[i + 5..];
    }
    None
}

fn emit_define_colors(out: &mut String, body: &str) {
    for raw in body.lines() {
        let line = raw.trim();
        if line.is_empty() {
            continue;
        }
        let Some(spec) = line.strip_prefix("--") else {
            continue;
        };
        let Some((name, value)) = spec.split_once(':') else {
            continue;
        };
        let name = name.trim();
        let value = value.trim().trim_end_matches(';').trim();
        if name.is_empty()
            || value.is_empty()
            || !name
                .bytes()
                .all(|b| matches!(b, b'0'..=b'9' | b'A'..=b'Z' | b'a'..=b'z' | b'_' | b'-'))
        {
            continue;
        }
        out.push_str("@define-color ");
        out.push_str(name);
        out.push(' ');
        out.push_str(value);
        out.push_str(";\n");
    }
}

fn gtkify_var_refs(css: &str) -> String {
    let mut out = String::with_capacity(css.len());
    let mut rest = css;
    while let Some(start) = rest.find("var(--") {
        out.push_str(&rest[..start]);
        rest = &rest[start + 6..];
        match rest.find(')') {
            Some(end) => {
                let name = &rest[..end];
                if !name.is_empty()
                    && name
                        .bytes()
                        .all(|b| matches!(b, b'0'..=b'9' | b'A'..=b'Z' | b'a'..=b'z' | b'_' | b'-'))
                {
                    out.push('@');
                    out.push_str(name);
                    rest = &rest[end + 1..];
                } else {
                    out.push_str("var(--");
                }
            }
            None => {
                out.push_str("var(--");
            }
        }
    }
    out.push_str(rest);
    out
}

fn path() -> std::path::PathBuf {
    glib_config_dir().join("prefs.json")
}

fn glib_config_dir() -> std::path::PathBuf {
    relm4::gtk::glib::user_config_dir().join("sh.devnull.Client")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn highlight_on_hover_defaults_on() {
        let prefs: Prefs = serde_json::from_str("{}").unwrap();
        assert!(prefs.highlight_on_hover);
        assert!(prefs.show_who_to_follow);
        assert!(!prefs.chat_docked);
        assert_eq!(prefs.event_theme, EventThemeChoice::Auto);
    }

    #[test]
    fn event_themes_cover_the_days_around_each_holiday() {
        let day = |month, n| NaiveDate::from_ymd_opt(2026, month, n).unwrap();
        assert_eq!(event_for(EventThemeChoice::Auto, day(10, 28)), None);
        assert_eq!(
            event_for(EventThemeChoice::Auto, day(10, 29)),
            Some(EventTheme::Halloween)
        );
        assert_eq!(
            event_for(EventThemeChoice::Auto, day(11, 1)),
            Some(EventTheme::Halloween)
        );
        assert_eq!(event_for(EventThemeChoice::Auto, day(11, 2)), None);
        assert_eq!(
            event_for(EventThemeChoice::Auto, day(12, 23)),
            Some(EventTheme::Christmas)
        );
        assert_eq!(
            event_for(EventThemeChoice::Auto, day(12, 26)),
            Some(EventTheme::Christmas)
        );
        assert_eq!(event_for(EventThemeChoice::Auto, day(12, 27)), None);
        assert_eq!(
            event_for(EventThemeChoice::Auto, day(12, 30)),
            Some(EventTheme::NewYear)
        );
        assert_eq!(
            event_for(EventThemeChoice::Auto, day(1, 2)),
            Some(EventTheme::NewYear)
        );
        assert_eq!(event_for(EventThemeChoice::Auto, day(1, 3)), None);
        assert_eq!(event_for(EventThemeChoice::None, day(10, 31)), None);
        assert_eq!(
            event_for(EventThemeChoice::Christmas, day(6, 1)),
            Some(EventTheme::Christmas)
        );
        assert_eq!(greeted_year_on(day(12, 31)), 2027);
        assert_eq!(greeted_year_on(day(1, 1)), 2026);
    }

    #[test]
    fn gtkify_css_turns_custom_properties_into_named_colors() {
        let css = gtkify_css("color: var(--dn_primary); background: alpha(var(--dn_wash), 0.45);");
        assert_eq!(
            css,
            "color: @dn_primary; background: alpha(@dn_wash, 0.45);"
        );
        let css = gtkify_css(":root { --dn_shell: #f6f7f8; }\nwindow { color: var(--dn_shell); }");
        assert_eq!(
            css,
            "@define-color dn_shell #f6f7f8;\n\nwindow { color: @dn_shell; }"
        );
        let sheet = gtkify_css(include_str!("style.css"));
        assert!(sheet.contains("@define-color dn_shell #f6f7f8;"));
        assert!(sheet.contains("background-color: @dn_shell;"));
        assert!(!sheet.contains("var(--dn_shell)"));
        assert!(!sheet.contains(":root"));
    }
}
