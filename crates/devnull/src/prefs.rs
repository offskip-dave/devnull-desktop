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
    #[serde(default)]
    pub last_visits: HashMap<String, String>,
}

fn default_true() -> bool {
    true
}

impl Default for Prefs {
    fn default() -> Self {
        Self {
            theme: Theme::System,
            show_who_to_follow: true,
            last_visits: HashMap::new(),
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

    pub fn apply(&self) {
        adw::StyleManager::default().set_color_scheme(self.theme.color_scheme());
        reload_css();
    }
}

/// GTK `@define-color` is global and `@media (prefers-color-scheme)` follows the
/// OS, not libadwaita's `ColorScheme`. Inject dark tokens when the app is dark.
const DARK_TOKENS: &str = r#"
window { color-scheme: dark; }
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
    let mut css = String::from(include_str!("style.css"));
    if dark {
        css.push_str(DARK_TOKENS);
    }
    relm4::set_global_css(&css);
}

fn path() -> std::path::PathBuf {
    glib_config_dir().join("prefs.json")
}

fn glib_config_dir() -> std::path::PathBuf {
    relm4::gtk::glib::user_config_dir().join("sh.devnull.Client")
}
