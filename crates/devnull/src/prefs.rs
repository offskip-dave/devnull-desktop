use std::collections::HashMap;

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

    pub fn apply(&self) {
        adw::StyleManager::default().set_color_scheme(self.theme.color_scheme());
        reload_css();
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
    relm4::set_global_css(&css);
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
