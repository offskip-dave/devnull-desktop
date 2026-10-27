use chrono::{
    DateTime, Datelike, Duration as ChronoDuration, Local, NaiveDate, TimeZone, Timelike, Utc,
};
use devnull_api::{
    Achievements, Activity, ActivityCell, ApiError, Badge, Client, Commit, ConversationSummary,
    CurrentUser, Embed, Lane, LinkPreview, Message, Notification, Profile, SearchResults, Streaks,
    Upload, User,
};
use relm4::adw;
use relm4::adw::prelude::{AdwDialogExt, AlertDialogExt};
use relm4::gtk;
use relm4::gtk::gio::prelude::*;
use relm4::gtk::glib;
use relm4::gtk::pango;
use relm4::gtk::prelude::*;
use relm4::Sender;

use std::cell::{Cell, RefCell};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::rc::Rc;
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

use crate::app::{AppMsg, Nav, ProfileFeed, SettingsTab};
use crate::body;
use crate::emoji;
use crate::images;
use crate::prefs::Theme;

pub use crate::body::body_markup;

pub struct FeedCtx {
    pub sender: Sender<AppMsg>,
    pub pasteis_mode: bool,
    pub me_id: Option<String>,
    pub stars: HashMap<String, (bool, i64)>,
    pub pasteis: HashMap<String, i64>,
    pub expanded: HashSet<String>,
    /// Search and commit ancestors join posts on one line. The log and profile keep each post as its own stub.
    pub connected: bool,
    /// Dashed gray from the composer into the first log avatar.
    pub join_staged: bool,
}

pub struct ThreadState<'a> {
    pub nested: &'a HashMap<String, Vec<Commit>>,
    pub expanded: &'a HashSet<String>,
    pub replying_to: Option<&'a str>,
    pub composer: &'a gtk::Box,
}

#[derive(Clone, Copy)]
pub struct CommitChrome {
    pub depth: u8,
    pub focused: bool,
    pub last: bool,
    pub grouped: bool,
    pub connected: bool,
    pub terminator: bool,
    pub dashed_in: bool,
}

impl Default for CommitChrome {
    fn default() -> Self {
        Self {
            depth: 0,
            focused: false,
            last: false,
            grouped: false,
            connected: false,
            terminator: true,
            dashed_in: false,
        }
    }
}

impl CommitChrome {
    fn commit_kind(self, has_previous: bool) -> RailKind {
        RailKind::Commit {
            incoming: (self.connected && has_previous) || self.dashed_in,
            terminator: self.terminator,
            dashed_in: self.dashed_in,
        }
    }
}

fn pointer(widget: &impl WidgetExt) {
    widget.set_cursor_from_name(Some("pointer"));
}

fn resources_dir() -> &'static Path {
    static DIR: OnceLock<PathBuf> = OnceLock::new();
    DIR.get_or_init(|| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../resources"))
}

pub fn register_icons() {
    let Some(display) = gtk::gdk::Display::default() else {
        return;
    };
    gtk::IconTheme::for_display(&display).add_search_path(resources_dir().join("icons"));
}

pub fn mark_path(dark: bool) -> &'static str {
    if dark {
        static DARK: OnceLock<String> = OnceLock::new();
        DARK.get_or_init(|| {
            resources_dir()
                .join("icons/hicolor/scalable/apps/devnull-mark-dark.svg")
                .to_string_lossy()
                .into_owned()
        })
    } else {
        static LIGHT: OnceLock<String> = OnceLock::new();
        LIGHT.get_or_init(|| {
            resources_dir()
                .join("icons/hicolor/scalable/apps/devnull-mark.svg")
                .to_string_lossy()
                .into_owned()
        })
    }
}

pub fn wordmark_markup() -> String {
    let color = if adw_is_dark() { "#ff8a4c" } else { "#d9480f" };
    format!("/dev<span foreground=\"{color}\">/</span>null")
}

pub fn logo_menu(sender: Sender<AppMsg>) -> gtk::Popover {
    let popover = gtk::Popover::new();
    popover.set_has_arrow(true);
    popover.set_position(gtk::PositionType::Bottom);
    popover.add_css_class("dn-logo-menu");
    let list = gtk::Box::new(gtk::Orientation::Vertical, 2);
    let home = devnull_api::default_base_url();
    list.append(&logo_menu_item(
        "Open in browser",
        {
            let sender = sender.clone();
            let home = home.clone();
            move || sender.emit(AppMsg::OpenWeb(home.clone()))
        },
        &popover,
    ));
    list.append(&logo_menu_item(
        "Copy link",
        {
            let sender = sender.clone();
            move || sender.emit(AppMsg::CopyHomeLink)
        },
        &popover,
    ));
    list.append(&logo_menu_item(
        "Source code",
        {
            let sender = sender.clone();
            move || {
                sender.emit(AppMsg::OpenWeb(
                    "https://github.com/dev-null-sh/dev-null.sh".into(),
                ))
            }
        },
        &popover,
    ));
    popover.set_child(Some(&list));
    popover
}

fn logo_menu_item(
    label: &str,
    on_click: impl Fn() + 'static,
    popover: &gtk::Popover,
) -> gtk::Button {
    let btn = gtk::Button::with_label(label);
    btn.add_css_class("flat");
    btn.add_css_class("dn-nav-btn");
    pointer(&btn);
    let popover = popover.clone();
    btn.connect_clicked(move |_| {
        popover.popdown();
        on_click();
    });
    btn
}

pub struct UserMenu {
    pub popover: gtk::Popover,
    avatar: gtk::Box,
    name: gtk::Label,
    handle: gtk::Label,
}

impl UserMenu {
    pub fn new(sender: Sender<AppMsg>) -> Self {
        Self::build(sender, gtk::PositionType::Top)
    }

    pub fn for_header(sender: Sender<AppMsg>) -> Self {
        Self::build(sender, gtk::PositionType::Bottom)
    }

    fn build(sender: Sender<AppMsg>, position: gtk::PositionType) -> Self {
        let popover = gtk::Popover::new();
        popover.set_has_arrow(true);
        popover.set_position(position);
        popover.add_css_class("dn-user-popover");
        popover.set_size_request(240, -1);

        let list = gtk::Box::new(gtk::Orientation::Vertical, 2);
        let header = gtk::Box::new(gtk::Orientation::Horizontal, 10);
        header.add_css_class("dn-user-menu-header");
        let avatar = gtk::Box::new(gtk::Orientation::Horizontal, 0);
        avatar.add_css_class("dn-nav-avatar");
        avatar.set_valign(gtk::Align::Center);
        header.append(&avatar);
        let who = gtk::Box::new(gtk::Orientation::Vertical, 0);
        who.set_hexpand(true);
        let name = gtk::Label::new(None);
        name.add_css_class("dn-name");
        name.set_xalign(0.0);
        name.set_ellipsize(pango::EllipsizeMode::End);
        who.append(&name);
        let handle = gtk::Label::new(None);
        handle.add_css_class("dn-handle");
        handle.set_xalign(0.0);
        handle.set_ellipsize(pango::EllipsizeMode::End);
        who.append(&handle);
        header.append(&who);
        list.append(&header);
        list.append(&menu_separator());

        let home = devnull_api::default_base_url();
        list.append(&user_menu_item(
            "dn-bookmark-symbolic",
            "Starred",
            {
                let sender = sender.clone();
                move || sender.emit(AppMsg::Select(Nav::Starred))
            },
            &popover,
        ));
        list.append(&user_menu_item(
            "dn-user-round-symbolic",
            "Profile",
            {
                let sender = sender.clone();
                move || sender.emit(AppMsg::OpenMe)
            },
            &popover,
        ));
        list.append(&user_menu_item(
            "dn-settings-2-symbolic",
            "Settings",
            {
                let sender = sender.clone();
                move || sender.emit(AppMsg::OpenSettings)
            },
            &popover,
        ));
        list.append(&menu_separator());
        list.append(&user_menu_item(
            "dn-book-open-symbolic",
            "About",
            {
                let sender = sender.clone();
                let home = home.clone();
                move || sender.emit(AppMsg::OpenWeb(format!("{home}/about")))
            },
            &popover,
        ));
        list.append(&user_menu_item(
            "dn-layout-grid-symbolic",
            "Apps",
            {
                let sender = sender.clone();
                let home = home.clone();
                move || sender.emit(AppMsg::OpenWeb(format!("{home}/apps")))
            },
            &popover,
        ));
        list.append(&user_menu_item(
            "dn-file-text-symbolic",
            "Terms",
            {
                let sender = sender.clone();
                let home = home.clone();
                move || sender.emit(AppMsg::OpenWeb(format!("{home}/terms")))
            },
            &popover,
        ));
        list.append(&user_menu_item(
            "dn-shield-symbolic",
            "Privacy",
            {
                let sender = sender.clone();
                let home = home.clone();
                move || sender.emit(AppMsg::OpenWeb(format!("{home}/privacy")))
            },
            &popover,
        ));
        list.append(&user_menu_item(
            "dn-github-symbolic",
            "GitHub",
            {
                let sender = sender.clone();
                move || {
                    sender.emit(AppMsg::OpenWeb(
                        "https://github.com/dev-null-sh/dev-null.sh".into(),
                    ))
                }
            },
            &popover,
        ));
        list.append(&menu_separator());
        list.append(&user_menu_item(
            "dn-log-out-symbolic",
            "Log out",
            {
                let sender = sender.clone();
                move || sender.emit(AppMsg::SignOut)
            },
            &popover,
        ));
        popover.set_child(Some(&list));
        Self {
            popover,
            avatar,
            name,
            handle,
        }
    }

    pub fn set_user(&self, user: Option<&User>) {
        fill_avatar(&self.avatar, user, 36, false);
        self.name
            .set_label(user.map(User::display_name).unwrap_or_default());
        self.handle.set_label(
            &user
                .map(|user| format!("@{}", user.username))
                .unwrap_or_default(),
        );
    }
}

fn menu_separator() -> gtk::Separator {
    let sep = gtk::Separator::new(gtk::Orientation::Horizontal);
    sep.add_css_class("dn-menu-sep");
    sep
}

fn user_menu_item(
    icon: &str,
    label: &str,
    on_click: impl Fn() + 'static,
    popover: &gtk::Popover,
) -> gtk::Button {
    let btn = gtk::Button::new();
    btn.add_css_class("flat");
    btn.add_css_class("dn-nav-btn");
    pointer(&btn);
    let row = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    row.append(&named_icon(icon, 16));
    let text = gtk::Label::new(Some(label));
    text.set_xalign(0.0);
    text.set_hexpand(true);
    row.append(&text);
    btn.set_child(Some(&row));
    let popover = popover.clone();
    btn.connect_clicked(move |_| {
        popover.popdown();
        on_click();
    });
    btn
}

fn pastel_path() -> &'static str {
    static PATH: OnceLock<String> = OnceLock::new();
    PATH.get_or_init(|| {
        resources_dir()
            .join("icons/hicolor/scalable/status/pastel-de-nata.svg")
            .to_string_lossy()
            .into_owned()
    })
}

fn named_icon(name: &str, pixel_size: i32) -> gtk::Image {
    let image = gtk::Image::from_icon_name(name);
    image.set_pixel_size(pixel_size);
    image
}

fn pastel_icon(pixel_size: i32) -> gtk::Image {
    let image = gtk::Image::from_file(pastel_path());
    image.set_pixel_size(pixel_size);
    image
}

pub fn bind_window_scroll(
    bar: &gtk::Scrollbar,
    stack: &gtk::Stack,
    sidebar: &impl IsA<gtk::Widget>,
    aside: &impl IsA<gtk::Widget>,
    pages: &[(&str, gtk::ScrolledWindow)],
) {
    for (_, scroll) in pages {
        scroll.set_vscrollbar_policy(gtk::PolicyType::External);
        scroll.set_overlay_scrolling(false);
    }
    bar.set_orientation(gtk::Orientation::Vertical);
    bar.set_halign(gtk::Align::End);
    bar.set_valign(gtk::Align::Fill);
    bar.set_hexpand(false);
    bar.set_vexpand(true);
    bar.set_can_focus(false);
    bar.add_css_class("dn-window-scroll");

    let pages: Rc<Vec<(String, gtk::ScrolledWindow)>> = Rc::new(
        pages
            .iter()
            .map(|(name, scroll)| ((*name).to_string(), scroll.clone()))
            .collect(),
    );
    let current = Rc::new(RefCell::new(None::<gtk::Adjustment>));
    let sync = {
        let bar = bar.clone();
        let stack = stack.clone();
        let pages = pages.clone();
        let current = current.clone();
        Rc::new(move || {
            let name = stack.visible_child_name();
            let scroll = name.as_deref().and_then(|name| {
                pages
                    .iter()
                    .find(|(page, _)| page == name)
                    .map(|(_, scroll)| scroll)
            });
            match scroll {
                Some(scroll) => {
                    let adj = scroll.vadjustment();
                    bar.set_adjustment(Some(&adj));
                    let overflow = adj.upper() - adj.lower() > adj.page_size() + 1.0;
                    bar.set_visible(overflow);
                    current.replace(Some(adj));
                }
                None => {
                    bar.set_visible(false);
                    current.replace(None);
                }
            }
        })
    };
    for (_, scroll) in pages.iter() {
        scroll.vadjustment().connect_changed({
            let sync = sync.clone();
            move |_| sync()
        });
        scroll
            .vadjustment()
            .connect_notify_local(Some("page-size"), {
                let sync = sync.clone();
                move |_, _| sync()
            });
    }
    stack.connect_notify_local(Some("visible-child-name"), {
        let sync = sync.clone();
        move |_, _| sync()
    });
    sync();

    for pane in [sidebar.upcast_ref(), aside.upcast_ref()] {
        let controller = gtk::EventControllerScroll::new(
            gtk::EventControllerScrollFlags::VERTICAL | gtk::EventControllerScrollFlags::KINETIC,
        );
        controller.set_propagation_phase(gtk::PropagationPhase::Capture);
        let current = current.clone();
        controller.connect_scroll(move |controller, _, dy| {
            let Some(adj) = current.borrow().clone() else {
                return glib::Propagation::Proceed;
            };
            let delta = if controller.unit() == gtk::gdk::ScrollUnit::Wheel {
                dy * adj.step_increment().max(48.0)
            } else {
                dy
            };
            let upper = (adj.upper() - adj.page_size()).max(adj.lower());
            adj.set_value((adj.value() + delta).clamp(adj.lower(), upper));
            glib::Propagation::Stop
        });
        pane.add_controller(controller);
    }
}

pub fn apply_pointer_cursors(widget: &impl IsA<gtk::Widget>) {
    walk_pointer(widget.upcast_ref());
}

fn walk_pointer(widget: &gtk::Widget) {
    if widget.is::<gtk::Button>() {
        pointer(widget);
    } else if widget.is::<gtk::TextView>() || widget.is::<gtk::Entry>() {
        widget.set_cursor_from_name(Some("text"));
    }
    let mut child = widget.first_child();
    while let Some(node) = child {
        walk_pointer(&node);
        child = node.next_sibling();
    }
}

pub fn relative_time(dt: Option<DateTime<Utc>>) -> String {
    let Some(dt) = dt else {
        return String::new();
    };
    let delta = Utc::now() - dt;
    let secs = delta.num_seconds().max(0);
    if secs < 60 {
        format!("{secs}s")
    } else if secs < 3600 {
        format!("{}m", secs / 60)
    } else if secs < 86400 {
        format!("{}h", secs / 3600)
    } else {
        format!("{}d", secs / 86400)
    }
}

pub fn lane_color(user: Option<&User>, dark: bool) -> String {
    user.and_then(|u| u.lane.as_ref())
        .map(|l| l.css_color(dark))
        .unwrap_or_else(|| Lane::Palette(0).css_color(dark))
}

pub fn lane_class(user: Option<&User>) -> &'static str {
    match user.and_then(|u| u.lane.as_ref()) {
        Some(Lane::Palette(i)) => match *i % 6 {
            0 => "lane-0",
            1 => "lane-1",
            2 => "lane-2",
            3 => "lane-3",
            4 => "lane-4",
            _ => "lane-5",
        },
        _ => "lane-0",
    }
}

fn css_hex_token(hex: &str) -> String {
    let digits = hex.trim().trim_start_matches('#');
    if matches!(digits.len(), 3 | 4 | 6 | 8) && digits.bytes().all(|b| b.is_ascii_hexdigit()) {
        format!("#{digits}")
    } else {
        Lane::Palette(0).css_color(adw_is_dark())
    }
}

fn hex_tone_css(color: &str, class: &str) -> String {
    format!(
        ".dn-ring.{class} {{ box-shadow: 0 0 0 2px {color}; }}\n\
         .dn-achievements-chip.{class} {{ color: {color}; border: 1px solid alpha({color}, 0.25); background-color: alpha({color}, 0.08); }}\n\
         .dn-badge.unlocked.{class}:hover,\n\
         .dn-badge.unlocked.{class}.hover,\n\
         button.dn-badge-chip:hover .dn-badge.unlocked.{class} {{ border-color: {color}; }}\n"
    )
}

/// A CSS class for the resolved lane hex, including custom colors the palette classes miss.
fn hex_tone_class(hex: &str) -> String {
    let color = css_hex_token(hex);
    let class = format!(
        "dn-hex-{}",
        color.trim_start_matches('#').to_ascii_lowercase()
    );
    thread_local! {
        static PROVIDER: RefCell<Option<gtk::CssProvider>> = const { RefCell::new(None) };
        static SEEN: RefCell<HashSet<String>> = RefCell::new(HashSet::new());
        static SHEET: RefCell<String> = const { RefCell::new(String::new()) };
    }
    SEEN.with(|seen| {
        let mut seen = seen.borrow_mut();
        if !seen.insert(class.clone()) {
            return;
        }
        SHEET.with(|sheet| {
            let mut sheet = sheet.borrow_mut();
            sheet.push_str(&hex_tone_css(&color, &class));
            PROVIDER.with(|slot| {
                let mut slot = slot.borrow_mut();
                let provider = slot.get_or_insert_with(|| {
                    let provider = gtk::CssProvider::new();
                    if let Some(display) = gtk::gdk::Display::default() {
                        gtk::style_context_add_provider_for_display(
                            &display,
                            &provider,
                            gtk::STYLE_PROVIDER_PRIORITY_APPLICATION,
                        );
                    }
                    provider
                });
                provider.load_from_string(&sheet);
            });
        });
    });
    class
}

pub fn copy_text(text: &str) -> bool {
    let Some(display) = gtk::gdk::Display::default() else {
        return false;
    };
    display.clipboard().set_text(text);
    true
}

pub fn short_hash(commit: &Commit) -> String {
    if let Some(hash) = commit.short_hash.as_deref().filter(|s| !s.is_empty()) {
        return hash.to_owned();
    }
    let compact: String = commit.id.chars().filter(|c| *c != '-').collect();
    if compact.len() <= 7 {
        compact
    } else {
        compact[compact.len() - 7..].to_owned()
    }
}

pub fn commit_url(commit: &Commit) -> String {
    if let Some(url) = commit.html_url.as_deref().filter(|s| !s.is_empty()) {
        return url.to_owned();
    }
    let base = devnull_api::default_base_url()
        .trim_end_matches('/')
        .to_owned();
    match commit.author.as_ref() {
        Some(user) => format!("{base}/@{}/commits/{}", user.username, commit.id),
        None => format!("{base}/commits/{}", commit.id),
    }
}

pub fn commit_id_from_row(row: &gtk::ListBoxRow) -> Option<String> {
    row.widget_name()
        .strip_prefix("commit-")
        .filter(|id| !id.is_empty())
        .map(str::to_owned)
}

pub fn focused_commit_id(list: &gtk::ListBox) -> Option<String> {
    let mut index = 0;
    while let Some(row) = list.row_at_index(index) {
        if row.has_css_class("dn-commit") && holds_focus(&row) {
            return commit_id_from_row(&row);
        }
        index += 1;
    }
    None
}

pub fn move_commit_focus(list: &gtk::ListBox, step: i32) {
    let mut rows = Vec::new();
    let mut index = 0;
    while let Some(row) = list.row_at_index(index) {
        if row.has_css_class("dn-commit") {
            rows.push(row);
        }
        index += 1;
    }
    if rows.is_empty() {
        return;
    }
    let current = rows.iter().position(holds_focus);
    let next = match current {
        None if step > 0 => 0,
        None => rows.len() - 1,
        Some(at) => at.saturating_add_signed(step as isize).min(rows.len() - 1),
    };
    let row = &rows[next];
    row.grab_focus();
    scroll_row_into_view(row);
}

fn holds_focus(widget: &impl IsA<gtk::Widget>) -> bool {
    let widget = widget.upcast_ref::<gtk::Widget>();
    let Some(root) = widget.root() else {
        return false;
    };
    let Some(focus) = root.focus() else {
        return false;
    };
    &focus == widget || focus.is_ancestor(widget)
}

fn scroll_row_into_view(row: &gtk::ListBoxRow) {
    let Some(scroll) = row
        .ancestor(gtk::ScrolledWindow::static_type())
        .and_then(|widget| widget.downcast::<gtk::ScrolledWindow>().ok())
    else {
        return;
    };
    let Some(bounds) = row.compute_bounds(&scroll) else {
        return;
    };
    let adj = scroll.vadjustment();
    let y = f64::from(bounds.y());
    let height = f64::from(bounds.height());
    let value = adj.value();
    let page = adj.page_size();
    if y < 0.0 {
        adj.set_value(value + y);
    } else if y + height > page {
        adj.set_value(value + y + height - page);
    }
}

pub fn day_key(dt: DateTime<Utc>) -> String {
    dt.with_timezone(&Local).date_naive().to_string()
}

pub fn day_label(dt: DateTime<Utc>) -> String {
    let local = dt.with_timezone(&Local);
    let today = Local::now().date_naive();
    let day = local.date_naive();
    if day == today {
        "Today".into()
    } else if today.pred_opt() == Some(day) {
        "Yesterday".into()
    } else {
        local.format("%a, %b %e").to_string()
    }
}

fn rgb(hex: &str) -> (f64, f64, f64) {
    let h = hex.trim_start_matches('#');
    if h.len() < 6 {
        return (0.23, 0.44, 0.88);
    }
    let parse = |i| u8::from_str_radix(&h[i..i + 2], 16).unwrap_or(0) as f64 / 255.0;
    (parse(0), parse(2), parse(4))
}

fn rail_gray(dark: bool) -> (f64, f64, f64) {
    if dark {
        (0.21, 0.21, 0.23)
    } else {
        (0.83, 0.84, 0.87)
    }
}

fn surface(dark: bool) -> (f64, f64, f64) {
    if dark {
        (0.09, 0.09, 0.10)
    } else {
        (1.0, 1.0, 1.0)
    }
}

const RAIL_WIDTH: i32 = 64;
const RAIL_AVATAR: i32 = 40;
const RAIL_NODE_Y: f64 = 32.0;

pub fn staged_avatar_top() -> i32 {
    RAIL_NODE_Y as i32 - RAIL_AVATAR / 2
}
const RAIL_AVATAR_TOP: i32 = 10;
const RAIL_TERMINATOR_PAD: f64 = 20.0;
const RAIL_DOT_RING: f64 = 7.0;
const RAIL_DOT_FILL: f64 = 4.0;
const STAGED_RING: f64 = 22.0;

pub fn rail(color: &str, previous: Option<&str>, variant: RailKind) -> gtk::DrawingArea {
    rail_area(color, previous, variant, false)
}

fn rail_area(
    color: &str,
    previous: Option<&str>,
    variant: RailKind,
    skip_node: bool,
) -> gtk::DrawingArea {
    let area = gtk::DrawingArea::new();
    let nested = matches!(variant, RailKind::Nested { .. });
    area.set_content_width(if nested { 28 } else { RAIL_WIDTH });
    area.set_content_height(
        if matches!(
            variant,
            RailKind::Commit {
                terminator: true,
                ..
            }
        ) {
            96
        } else {
            64
        },
    );
    area.set_hexpand(false);
    area.set_vexpand(true);
    area.add_css_class("dn-rail");
    if nested {
        area.add_css_class("dn-rail-nested");
    }
    let dark = adw_is_dark();
    let has_previous = previous.is_some();
    let (cr, cg, cb) = rgb(color);
    let (pr, pg, pb) = previous.map(rgb).unwrap_or_else(|| rail_gray(dark));
    let (sr, sg, sb) = surface(dark);
    let (gr, gg, gb) = rail_gray(dark);
    area.set_draw_func(move |_, cairo, width, height| {
        let cx = f64::from(width) / 2.0;
        let h = f64::from(height);
        let node_y = if matches!(variant, RailKind::Nested { .. }) {
            18.0
        } else {
            RAIL_NODE_Y
        };
        match variant {
            RailKind::Day => {}
            RailKind::Segment | RailKind::Dashed => {
                cairo.set_line_width(2.0);
                if matches!(variant, RailKind::Dashed) {
                    cairo.set_dash(&[4.0, 4.0], 0.0);
                    cairo.set_source_rgb(gr, gg, gb);
                } else {
                    cairo.set_source_rgb(cr, cg, cb);
                }
                cairo.move_to(cx, 0.0);
                cairo.line_to(cx, h);
                cairo.stroke().ok();
                cairo.set_dash(&[], 0.0);
            }
            RailKind::Nested { first, continues } => {
                cairo.set_line_width(1.0);
                cairo.set_dash(&[3.0, 3.0], 0.0);
                cairo.set_source_rgb(pr, pg, pb);
                if first {
                    cairo.move_to(0.0, 4.0);
                    cairo.line_to(cx, 4.0);
                    cairo.line_to(cx, node_y);
                } else {
                    cairo.move_to(cx, 0.0);
                    cairo.line_to(cx, node_y);
                }
                cairo.stroke().ok();
                if continues {
                    cairo.set_source_rgb(cr, cg, cb);
                    cairo.move_to(cx, node_y);
                    cairo.line_to(cx, h);
                    cairo.stroke().ok();
                }
                cairo.set_dash(&[], 0.0);
                if !skip_node {
                    draw_rail_dot(
                        cairo,
                        (cx, node_y),
                        (cr, cg, cb),
                        (sr, sg, sb),
                        DotStyle::Nested,
                    );
                }
            }
            RailKind::Root => {
                if has_previous {
                    cairo.set_line_width(2.0);
                    cairo.set_source_rgb(gr, gg, gb);
                    cairo.move_to(cx, 0.0);
                    cairo.line_to(cx, node_y);
                    cairo.stroke().ok();
                }
                if !skip_node {
                    draw_rail_dot(
                        cairo,
                        (cx, node_y),
                        (gr, gg, gb),
                        (sr, sg, sb),
                        DotStyle::Hollow,
                    );
                }
            }
            RailKind::Commit {
                incoming,
                terminator,
                dashed_in,
            } => {
                cairo.set_line_width(2.0);
                if incoming {
                    if dashed_in {
                        cairo.set_dash(&[4.0, 4.0], 0.0);
                        cairo.set_source_rgb(gr, gg, gb);
                    } else {
                        cairo.set_source_rgb(pr, pg, pb);
                    }
                    cairo.move_to(cx, 0.0);
                    cairo.line_to(cx, node_y);
                    cairo.stroke().ok();
                    cairo.set_dash(&[], 0.0);
                }
                let tail_end = if terminator {
                    h - RAIL_TERMINATOR_PAD
                } else {
                    h
                };
                cairo.set_source_rgb(cr, cg, cb);
                cairo.move_to(cx, node_y);
                cairo.line_to(cx, tail_end);
                cairo.stroke().ok();
                if terminator {
                    draw_rail_dot(
                        cairo,
                        (cx, tail_end),
                        (cr, cg, cb),
                        (sr, sg, sb),
                        DotStyle::Filled,
                    );
                }
                if !skip_node {
                    draw_rail_dot(
                        cairo,
                        (cx, node_y),
                        (cr, cg, cb),
                        (sr, sg, sb),
                        DotStyle::Filled,
                    );
                }
            }
        }
    });
    area
}

#[derive(Clone, Copy)]
enum DotStyle {
    Filled,
    Hollow,
    Nested,
}

fn draw_rail_dot(
    cairo: &gtk::cairo::Context,
    at: (f64, f64),
    lane: (f64, f64, f64),
    surface: (f64, f64, f64),
    style: DotStyle,
) {
    let (cx, cy) = at;
    let (ring, fill) = match style {
        DotStyle::Nested => (5.0, 3.0),
        _ => (RAIL_DOT_RING, RAIL_DOT_FILL),
    };
    cairo.set_source_rgb(surface.0, surface.1, surface.2);
    cairo.arc(cx, cy, ring + 4.0, 0.0, std::f64::consts::TAU);
    cairo.fill().ok();
    cairo.set_line_width(2.0);
    cairo.set_source_rgb(lane.0, lane.1, lane.2);
    cairo.arc(cx, cy, ring, 0.0, std::f64::consts::TAU);
    cairo.stroke().ok();
    if !matches!(style, DotStyle::Hollow) {
        cairo.set_source_rgb(lane.0, lane.1, lane.2);
        cairo.arc(cx, cy, fill, 0.0, std::f64::consts::TAU);
        cairo.fill().ok();
    }
}

fn commit_rail(
    color: &str,
    previous: Option<&str>,
    variant: RailKind,
    author: Option<&User>,
    grouped: bool,
    sender: Sender<AppMsg>,
) -> gtk::Widget {
    let skip_node = author.is_some() && !grouped && matches!(variant, RailKind::Commit { .. });
    let area = rail_area(color, previous, variant, skip_node);
    if !skip_node {
        return area.upcast();
    }
    let overlay = gtk::Overlay::new();
    overlay.add_css_class("dn-rail");
    overlay.set_hexpand(false);
    overlay.set_vexpand(true);
    overlay.set_overflow(gtk::Overflow::Visible);
    overlay.set_size_request(RAIL_WIDTH, -1);
    overlay.set_child(Some(&area));
    overlay.add_overlay(&rail_avatar_node(author, sender));
    overlay.upcast()
}

fn rail_avatar_node(user: Option<&User>, sender: Sender<AppMsg>) -> gtk::Widget {
    let btn = gtk::Button::new();
    btn.add_css_class("flat");
    btn.add_css_class("dn-rail-avatar");
    btn.set_overflow(gtk::Overflow::Visible);
    pointer(&btn);
    btn.set_halign(gtk::Align::Center);
    btn.set_valign(gtk::Align::Start);
    btn.set_margin_top(RAIL_AVATAR_TOP);
    btn.set_can_focus(false);
    btn.set_child(Some(&avatar_widget(user, RAIL_AVATAR, true)));
    if let Some(username) = user.map(|u| u.username.clone()) {
        btn.connect_clicked(move |_| {
            sender.emit(AppMsg::OpenProfile(username.clone()));
        });
    }
    btn.upcast()
}

#[derive(Clone, Copy)]
pub enum RailKind {
    Commit {
        incoming: bool,
        terminator: bool,
        dashed_in: bool,
    },
    Root,
    Day,
    Segment,
    Dashed,
    Nested {
        first: bool,
        continues: bool,
    },
}

#[derive(Clone, Copy)]
pub struct StagedPaint {
    pub lane: (f64, f64, f64),
    pub fill: f64,
    pub target: f64,
    ticking: bool,
    last_us: i64,
}

impl Default for StagedPaint {
    fn default() -> Self {
        Self::new()
    }
}

impl StagedPaint {
    pub fn new() -> Self {
        let gray = rail_gray(adw_is_dark());
        Self {
            lane: gray,
            fill: 0.0,
            target: 0.0,
            ticking: false,
            last_us: 0,
        }
    }
}

pub fn bind_staged_rail(
    overlay: &gtk::Overlay,
    paint: &Rc<RefCell<StagedPaint>>,
) -> gtk::DrawingArea {
    let area = gtk::DrawingArea::new();
    area.set_content_width(RAIL_WIDTH);
    area.set_content_height(64);
    area.set_hexpand(false);
    area.set_vexpand(true);
    area.set_valign(gtk::Align::Fill);
    area.add_css_class("dn-rail");
    let paint_draw = paint.clone();
    area.set_draw_func(move |_, cairo, width, height| {
        let paint = paint_draw.borrow();
        let dark = adw_is_dark();
        let (sr, sg, sb) = surface(dark);
        let (gr, gg, gb) = rail_gray(dark);
        let (lr, lg, lb) = paint.lane;
        let cx = f64::from(width) / 2.0;
        let h = f64::from(height);
        let node_y = RAIL_NODE_Y;
        cairo.set_line_width(2.0);
        cairo.set_dash(&[4.0, 4.0], 0.0);
        cairo.set_source_rgb(gr, gg, gb);
        cairo.move_to(cx, node_y);
        cairo.line_to(cx, h);
        cairo.stroke().ok();
        cairo.set_dash(&[], 0.0);
        cairo.set_source_rgb(sr, sg, sb);
        cairo.arc(cx, node_y, STAGED_RING + 2.0, 0.0, std::f64::consts::TAU);
        cairo.fill().ok();
        draw_staged_ring(cairo, cx, node_y, lr, lg, lb, paint.fill);
    });
    overlay.set_child(Some(&area));
    overlay.set_overflow(gtk::Overflow::Visible);
    overlay.set_size_request(RAIL_WIDTH, -1);
    overlay.set_hexpand(false);
    overlay.set_vexpand(false);
    area
}

fn draw_staged_ring(
    cairo: &gtk::cairo::Context,
    cx: f64,
    cy: f64,
    r: f64,
    g: f64,
    b: f64,
    fill: f64,
) {
    let start = -std::f64::consts::FRAC_PI_2;
    cairo.set_line_width(2.0);
    cairo.set_line_cap(gtk::cairo::LineCap::Butt);
    if fill <= 0.0 {
        cairo.set_dash(&[3.5, 3.5], 0.0);
        cairo.set_source_rgb(r, g, b);
        cairo.arc(cx, cy, STAGED_RING, 0.0, std::f64::consts::TAU);
        cairo.stroke().ok();
        cairo.set_dash(&[], 0.0);
        return;
    }
    if fill < 0.999 {
        cairo.set_dash(&[3.5, 3.5], 0.0);
        cairo.set_source_rgba(r, g, b, 0.35);
        cairo.arc(cx, cy, STAGED_RING, 0.0, std::f64::consts::TAU);
        cairo.stroke().ok();
        cairo.set_dash(&[], 0.0);
        cairo.set_line_cap(gtk::cairo::LineCap::Round);
        cairo.set_source_rgb(r, g, b);
        cairo.arc(
            cx,
            cy,
            STAGED_RING,
            start,
            start + fill * std::f64::consts::TAU,
        );
        cairo.stroke().ok();
    } else {
        cairo.set_source_rgb(r, g, b);
        cairo.arc(cx, cy, STAGED_RING, 0.0, std::f64::consts::TAU);
        cairo.stroke().ok();
    }
}

pub fn set_staged_user(
    paint: &Rc<RefCell<StagedPaint>>,
    area: &gtk::DrawingArea,
    user: Option<&User>,
) {
    let dark = adw_is_dark();
    paint.borrow_mut().lane = rgb(&lane_color(user, dark));
    area.queue_draw();
}

pub fn set_staged_fill(area: &gtk::DrawingArea, paint: &Rc<RefCell<StagedPaint>>, target: f64) {
    let target = target.clamp(0.0, 1.0);
    {
        let mut p = paint.borrow_mut();
        p.target = target;
        if (p.fill - target).abs() < 0.002 {
            p.fill = target;
            area.queue_draw();
            return;
        }
        if p.ticking {
            return;
        }
        p.ticking = true;
        p.last_us = 0;
    }
    let paint = paint.clone();
    area.add_tick_callback(move |area, clock| {
        let now = clock.frame_time();
        let mut p = paint.borrow_mut();
        if p.last_us == 0 {
            p.last_us = now;
            return glib::ControlFlow::Continue;
        }
        let dt = ((now - p.last_us) as f64 / 1_000_000.0).clamp(0.0, 0.05);
        p.last_us = now;
        let step = dt / 0.2;
        let delta = p.target - p.fill;
        if delta.abs() <= step {
            p.fill = p.target;
            p.ticking = false;
            area.queue_draw();
            glib::ControlFlow::Break
        } else {
            p.fill += delta.signum() * step;
            area.queue_draw();
            glib::ControlFlow::Continue
        }
    });
}

pub fn fill_avatar(box_: &gtk::Box, user: Option<&User>, size: i32, ring: bool) {
    while let Some(child) = box_.first_child() {
        box_.remove(&child);
    }
    if user.is_some() {
        box_.append(&avatar_widget(user, size, ring));
    }
}

pub fn fill_aside_you(
    handle: &gtk::Box,
    stats: &gtk::Box,
    user: Option<&User>,
    sender: Sender<AppMsg>,
) {
    while let Some(child) = handle.first_child() {
        handle.remove(&child);
    }
    while let Some(child) = stats.first_child() {
        stats.remove(&child);
    }
    let Some(user) = user else {
        return;
    };
    handle.append(&handle_dot(Some(user)));
    let at = gtk::Label::new(Some(&format!("@{}", user.username)));
    at.add_css_class("dn-handle");
    at.set_xalign(0.0);
    handle.append(&at);

    let commits = user.commits_count.unwrap_or(0);
    let following = user.following_count.unwrap_or(0);
    let followers = user.followers_count.unwrap_or(0);
    let base = devnull_api::default_base_url();
    let username = user.username.clone();
    stats.append(&aside_stat(
        commits,
        if commits == 1 { "commit" } else { "commits" },
        sender.clone(),
        || AppMsg::OpenMe,
    ));
    stats.append(&aside_stat(following, "following", sender.clone(), {
        let url = format!("{base}/@{username}/following");
        move || AppMsg::OpenWeb(url.clone())
    }));
    stats.append(&aside_stat(
        followers,
        if followers == 1 {
            "follower"
        } else {
            "followers"
        },
        sender,
        {
            let url = format!("{base}/@{username}/followers");
            move || AppMsg::OpenWeb(url.clone())
        },
    ));
}

fn aside_stat(
    count: i64,
    label: &str,
    sender: Sender<AppMsg>,
    on_click: impl Fn() -> AppMsg + 'static,
) -> gtk::Button {
    let btn = gtk::Button::new();
    btn.add_css_class("flat");
    btn.add_css_class("dn-aside-stat");
    pointer(&btn);
    let row = gtk::Box::new(gtk::Orientation::Horizontal, 6);
    let n = gtk::Label::new(Some(&format_count(count)));
    n.add_css_class("dn-aside-stat-count");
    n.add_css_class("dn-mono");
    let word = gtk::Label::new(Some(label));
    word.add_css_class("dn-aside-stat-label");
    word.add_css_class("dn-muted");
    row.append(&n);
    row.append(&word);
    btn.set_child(Some(&row));
    btn.connect_clicked(move |_| sender.emit(on_click()));
    btn
}

pub fn commits_label(n: i64) -> String {
    format!(
        "{} {}",
        format_count(n),
        if n == 1 { "commit" } else { "commits" }
    )
}

fn format_count(n: i64) -> String {
    if n <= 1000 {
        return n.to_string();
    }
    let n = n as f64;
    if n >= 1_000_000.0 {
        compact_count(n / 1_000_000.0, "m")
    } else {
        compact_count(n / 1_000.0, "k")
    }
}

fn compact_count(value: f64, suffix: &str) -> String {
    let rounded = (value * 10.0).round() / 10.0;
    if (rounded - rounded.trunc()).abs() < f64::EPSILON {
        format!("{}{suffix}", rounded as i64)
    } else {
        format!("{rounded:.1}{suffix}")
    }
}

pub fn avatar_widget(user: Option<&User>, size: i32, ring: bool) -> gtk::Widget {
    let name = user.map(User::display_name);
    let av = adw::Avatar::new(size, name, true);
    images::bind_avatar(&av, user.and_then(|u| u.avatar_url.as_deref()));
    av.add_css_class("dn-avatar");
    if user.is_some_and(User::is_live) {
        return live_avatar_chrome(av, user);
    }
    if ring {
        with_lane_ring(av, size, &lane_color(user, adw_is_dark()))
    } else {
        av.upcast()
    }
}

fn with_lane_ring(av: adw::Avatar, size: i32, hex: &str) -> gtk::Widget {
    let pad = 8;
    let wrap = gtk::Overlay::new();
    wrap.add_css_class("dn-avatar-ring");
    wrap.set_halign(gtk::Align::Start);
    wrap.set_valign(gtk::Align::Start);
    wrap.set_overflow(gtk::Overflow::Visible);
    wrap.set_size_request(size + pad, size + pad);
    let ring = gtk::DrawingArea::new();
    let (lr, lg, lb) = rgb(hex);
    ring.set_draw_func(move |_, cairo, width, height| {
        let cx = f64::from(width) / 2.0;
        let cy = f64::from(height) / 2.0;
        cairo.set_line_width(2.0);
        cairo.set_source_rgb(lr, lg, lb);
        cairo.arc(cx, cy, cx - 1.0, 0.0, std::f64::consts::TAU);
        cairo.stroke().ok();
    });
    wrap.set_child(Some(&ring));
    av.set_halign(gtk::Align::Center);
    av.set_valign(gtk::Align::Center);
    wrap.add_overlay(&av);
    wrap.upcast()
}

fn live_avatar_chrome(av: adw::Avatar, user: Option<&User>) -> gtk::Widget {
    let overlay = gtk::Overlay::new();
    overlay.add_css_class("dn-live-avatar-wrap");
    overlay.set_overflow(gtk::Overflow::Visible);
    overlay.set_valign(gtk::Align::Start);
    overlay.set_tooltip_text(Some(&format!(
        "{} is live",
        user.map(User::display_name).unwrap_or("This user")
    )));
    let ring = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    ring.add_css_class("dn-live-ring");
    ring.set_valign(gtk::Align::Center);
    ring.append(&av);
    overlay.set_child(Some(&ring));
    let badge = live_badge();
    badge.set_halign(gtk::Align::Center);
    badge.set_valign(gtk::Align::End);
    overlay.add_overlay(&badge);
    overlay.upcast()
}

fn image_gallery<'a>(
    images: impl IntoIterator<Item = (Option<&'a str>, Option<i32>, Option<i32>, Option<&'a str>)>,
    sender: &Sender<AppMsg>,
) -> gtk::Widget {
    let images: Vec<(Option<&str>, Option<i32>, Option<i32>, Option<&str>)> =
        images.into_iter().take(3).collect();
    let wrap = gtk::Box::new(gtk::Orientation::Vertical, 0);
    wrap.add_css_class("dn-images");
    wrap.set_overflow(gtk::Overflow::Hidden);
    wrap.set_hexpand(true);
    wrap.set_halign(gtk::Align::Fill);
    if images.len() == 1 {
        wrap.add_css_class("dn-images-single");
        wrap.append(&single_image(
            images[0].0,
            images[0].1,
            images[0].2,
            images[0].3,
            sender,
        ));
        return wrap.upcast();
    }
    wrap.add_css_class("dn-images-grid");
    wrap.set_size_request(-1, 288);
    let grid = gtk::Grid::new();
    grid.set_column_homogeneous(true);
    grid.set_row_homogeneous(true);
    grid.set_column_spacing(2);
    grid.set_row_spacing(2);
    grid.set_hexpand(true);
    grid.set_vexpand(true);
    grid.set_halign(gtk::Align::Fill);
    grid.set_valign(gtk::Align::Fill);
    match images.as_slice() {
        [first, second] => {
            grid.attach(&cover_tile(first.0, first.3, sender), 0, 0, 1, 1);
            grid.attach(&cover_tile(second.0, second.3, sender), 1, 0, 1, 1);
        }
        [first, second, third, ..] => {
            grid.attach(&cover_tile(first.0, first.3, sender), 0, 0, 1, 2);
            grid.attach(&cover_tile(second.0, second.3, sender), 1, 0, 1, 1);
            grid.attach(&cover_tile(third.0, third.3, sender), 1, 1, 1, 1);
        }
        _ => {}
    }
    wrap.append(&grid);
    wrap.upcast()
}

fn single_image(
    url: Option<&str>,
    width: Option<i32>,
    height: Option<i32>,
    description: Option<&str>,
    sender: &Sender<AppMsg>,
) -> gtk::AspectFrame {
    let ratio = images::capped_ratio(width.unwrap_or(16), height.unwrap_or(9));
    let frame = gtk::AspectFrame::new(0.5, 0.5, ratio, false);
    frame.set_obey_child(false);
    frame.set_hexpand(true);
    frame.set_halign(gtk::Align::Fill);
    let pic = cover_picture();
    images::bind_fitted_picture(&pic, url, Some(&frame));
    if let Some(description) = description.filter(|value| !value.is_empty()) {
        pic.set_tooltip_text(Some(description));
    }
    open_on_click(&pic, url, sender);
    frame.set_child(Some(&pic));
    frame
}

fn cover_tile(
    url: Option<&str>,
    description: Option<&str>,
    sender: &Sender<AppMsg>,
) -> gtk::Picture {
    let pic = cover_picture();
    images::bind_picture(&pic, url);
    if let Some(description) = description.filter(|value| !value.is_empty()) {
        pic.set_tooltip_text(Some(description));
    }
    open_on_click(&pic, url, sender);
    pic
}

fn open_on_click(widget: &impl IsA<gtk::Widget>, url: Option<&str>, sender: &Sender<AppMsg>) {
    let Some(url) = url.and_then(images::absolute_url) else {
        return;
    };
    let click = gtk::GestureClick::new();
    click.set_button(1);
    click.set_propagation_phase(gtk::PropagationPhase::Capture);
    let sender = sender.clone();
    click.connect_pressed(move |gesture, _, _, _| {
        gesture.set_state(gtk::EventSequenceState::Claimed);
        sender.emit(AppMsg::OpenWeb(url.clone()));
    });
    widget.as_ref().add_controller(click);
    pointer(widget.as_ref());
}

fn cover_picture() -> gtk::Picture {
    let pic = gtk::Picture::new();
    pic.add_css_class("dn-image");
    pic.set_content_fit(gtk::ContentFit::Cover);
    pic.set_can_shrink(true);
    pic.set_hexpand(true);
    pic.set_vexpand(true);
    pic.set_halign(gtk::Align::Fill);
    pic.set_valign(gtk::Align::Fill);
    pic
}

fn handle_dot(user: Option<&User>) -> gtk::DrawingArea {
    let dot = gtk::DrawingArea::new();
    dot.set_content_width(8);
    dot.set_content_height(8);
    dot.set_valign(gtk::Align::Center);
    let (lr, lg, lb) = rgb(&lane_color(user, adw_is_dark()));
    dot.set_draw_func(move |_, cairo, width, height| {
        cairo.set_source_rgb(lr, lg, lb);
        cairo.arc(
            f64::from(width) / 2.0,
            f64::from(height) / 2.0,
            4.0,
            0.0,
            std::f64::consts::TAU,
        );
        let _ = cairo.fill();
    });
    dot
}

pub fn commit_row(
    commit: &Commit,
    previous_lane: Option<&str>,
    ctx: &FeedCtx,
    chrome: CommitChrome,
) -> gtk::ListBoxRow {
    let sender = ctx.sender.clone();
    let row = gtk::ListBoxRow::new();
    row.add_css_class("dn-commit-row");
    row.add_css_class("dn-commit");
    row.set_widget_name(&format!("commit-{}", commit.id));
    row.set_activatable(true);
    row.set_focusable(true);
    pointer(&row);
    let depth = chrome.depth.min(3);
    row.set_margin_start(i32::from(depth) * 28);

    let dark = adw_is_dark();
    let color = lane_color(commit.author.as_ref(), dark);
    let outer = gtk::Box::new(gtk::Orientation::Horizontal, 0);

    if commit.is_deleted() {
        outer.append(&rail(
            "#d3d7dd",
            previous_lane,
            if depth > 0 {
                RailKind::Nested {
                    first: true,
                    continues: false,
                }
            } else {
                chrome.commit_kind(previous_lane.is_some())
            },
        ));
        let content = gtk::Box::new(gtk::Orientation::Vertical, 4);
        content.add_css_class("dn-commit-content");
        content.set_hexpand(true);
        let label = gtk::Label::new(Some("rm → /dev/null  This commit was deleted."));
        label.add_css_class("dn-muted");
        label.add_css_class("dn-mono");
        label.set_xalign(0.0);
        content.append(&label);
        if commit.replies_count.unwrap_or(0) > 0 {
            content.append(&replies_button(
                commit,
                sender.clone(),
                ctx.expanded.contains(&commit.id),
            ));
        }
        outer.append(&content);
        row.set_child(Some(&outer));
        return row;
    }

    let kind = if depth > 0 {
        RailKind::Nested {
            first: chrome.depth == 1,
            continues: !chrome.last,
        }
    } else {
        chrome.commit_kind(previous_lane.is_some())
    };
    if depth > 0 {
        outer.append(&rail(&color, previous_lane, kind));
    } else {
        outer.append(&commit_rail(
            &color,
            previous_lane,
            kind,
            commit.author.as_ref(),
            chrome.grouped,
            sender.clone(),
        ));
    }

    let content = gtk::Box::new(gtk::Orientation::Vertical, 6);
    content.add_css_class("dn-commit-content");
    content.set_hexpand(true);

    let header = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    if depth > 0 {
        header.append(&avatar_widget(commit.author.as_ref(), 40, false));
    }

    let names = gtk::Box::new(gtk::Orientation::Horizontal, 6);
    names.set_hexpand(true);
    if let Some(author) = &commit.author {
        let name = gtk::Label::new(Some(author.display_name()));
        name.set_xalign(0.0);
        name.add_css_class("dn-name");
        names.append(&name);
        names.append(&handle_dot(Some(author)));
        let handle = gtk::Label::new(Some(&format!("@{}", author.username)));
        handle.add_css_class("dn-handle");
        names.append(&handle);
        let username = author.username.clone();
        let profile_sender = sender.clone();
        let click = gtk::GestureClick::new();
        click.connect_released(move |_, _, _, _| {
            profile_sender.emit(AppMsg::OpenProfile(username.clone()));
        });
        names.add_controller(click);
    }
    let time = gtk::Label::new(Some(&relative_time(commit.created_at)));
    time.add_css_class("dn-mono");
    time.add_css_class("dn-muted");
    time.set_tooltip_text(
        commit
            .created_at
            .map(|d| d.with_timezone(&Local).to_rfc2822())
            .as_deref(),
    );
    names.append(&time);
    if let Some(hash) = &commit.short_hash {
        let h = gtk::Label::new(Some(hash));
        h.add_css_class("dn-mono");
        h.add_css_class("dn-muted");
        h.set_hexpand(true);
        h.set_xalign(1.0);
        names.append(&h);
    }
    header.append(&names);
    content.append(&header);

    if let Some(parent) = commit.parent.as_ref() {
        let reply_to = gtk::Label::new(Some(&format!(
            "↳ replying to @{}",
            parent
                .author
                .as_ref()
                .map(|u| u.username.as_str())
                .unwrap_or("deleted")
        )));
        reply_to.add_css_class("dn-mono");
        reply_to.add_css_class("dn-muted");
        reply_to.set_xalign(0.0);
        content.append(&reply_to);
    }

    if let Some(body) = &commit.body {
        let class = if chrome.focused {
            "dn-body dn-body-focus"
        } else {
            "dn-body"
        };
        content.append(&body::body_widget(
            body,
            class,
            commit.mentions.as_ref(),
            Some(&sender),
        ));
    }

    if !commit.images.is_empty() {
        content.append(&image_gallery(
            commit.images.iter().map(|image| {
                (
                    image.url.as_deref(),
                    image.width,
                    image.height,
                    image.description.as_deref(),
                )
            }),
            &sender,
        ));
    }

    if let Some(preview) = &commit.link_preview {
        content.append(&link_preview_widget(
            preview,
            Some(commit.id.as_str()),
            sender.clone(),
        ));
    }

    if commit.quoted_commit_id.is_some() {
        content.append(&quoted_commit_widget(
            commit.quoted_commit.as_deref(),
            sender.clone(),
        ));
    }

    content.append(&commit_actions(commit, ctx, chrome.focused));

    outer.append(&content);
    row.set_child(Some(&outer));

    let id = commit.id.clone();
    let open_sender = sender.clone();
    row.connect_activate(move |_| {
        open_sender.emit(AppMsg::OpenCommit(id.clone()));
    });
    row
}

fn action_btn(icon: &str, label: &str) -> gtk::Button {
    icon_button(Some(icon), false, label, 16, &["flat", "dn-action"])
}

fn pastel_btn(count: i64) -> gtk::Button {
    icon_button(
        None,
        true,
        &count.to_string(),
        16,
        &["flat", "dn-action", "dn-pastel"],
    )
}

fn icon_button(
    icon: Option<&str>,
    pastel: bool,
    label: &str,
    icon_size: i32,
    classes: &[&str],
) -> gtk::Button {
    let btn = gtk::Button::new();
    for class in classes {
        btn.add_css_class(class);
    }
    pointer(&btn);
    btn.set_child(Some(&icon_label(icon, pastel, label, icon_size)));
    btn
}

fn icon_label(icon: Option<&str>, pastel: bool, label: &str, icon_size: i32) -> gtk::Box {
    let row = gtk::Box::new(gtk::Orientation::Horizontal, 6);
    row.set_halign(gtk::Align::Center);
    if pastel {
        row.append(&pastel_icon(icon_size));
    } else if let Some(name) = icon {
        row.append(&named_icon(name, icon_size));
    }
    if !label.is_empty() {
        row.append(&gtk::Label::new(Some(label)));
    }
    row
}

fn star_child(starred: bool, count: i64) -> gtk::Box {
    let icon = if starred {
        "dn-star-filled-symbolic"
    } else {
        "dn-star-symbolic"
    };
    let text = if count > 0 {
        count.to_string()
    } else {
        String::new()
    };
    icon_label(Some(icon), false, &text, 16)
}

fn paint_star(btn: &gtk::Button, starred: bool, count: i64) {
    if starred {
        btn.add_css_class("starred");
    } else {
        btn.remove_css_class("starred");
    }
    btn.set_child(Some(&star_child(starred, count)));
}

fn commit_actions(commit: &Commit, ctx: &FeedCtx, focused: bool) -> gtk::Box {
    let meta = gtk::Box::new(gtk::Orientation::Horizontal, 4);
    meta.add_css_class("dn-actions");
    let (starred, count) = ctx.stars.get(&commit.id).copied().unwrap_or((
        commit.starred.unwrap_or(false),
        commit.stars_count.unwrap_or(0),
    ));
    let star = gtk::Button::new();
    star.add_css_class("flat");
    star.add_css_class("dn-action");
    star.add_css_class("dn-star");
    pointer(&star);
    paint_star(&star, starred, count);
    let id = commit.id.clone();
    let star_sender = ctx.sender.clone();
    let starred_cell = Rc::new(Cell::new(starred));
    let count_cell = Rc::new(Cell::new(count));
    star.connect_clicked(move |btn| {
        let next = !starred_cell.get();
        let next_count = if next {
            count_cell.get() + 1
        } else {
            count_cell.get().saturating_sub(1)
        };
        starred_cell.set(next);
        count_cell.set(next_count);
        paint_star(btn, next, next_count);
        star_sender.emit(AppMsg::ToggleStar(id.clone(), next, next_count));
    });
    meta.append(&star);

    if ctx.pasteis_mode {
        let n = ctx
            .pasteis
            .get(&commit.id)
            .copied()
            .or(commit.pasteis_count)
            .unwrap_or(0);
        let pastel = pastel_btn(n);
        pastel.set_tooltip_text(Some(&if n == 1 {
            "1 pastel".into()
        } else {
            format!("{n} pastéis")
        }));
        let own = commit.is_own(ctx.me_id.as_deref());
        let read_only = own
            || commit
                .status
                .as_deref()
                .is_some_and(|status| status != "approved")
            || commit.author_id().is_none();
        if read_only {
            pastel.set_sensitive(false);
        } else {
            let id = commit.id.clone();
            let sender = ctx.sender.clone();
            let count = Rc::new(Cell::new(n));
            pastel.connect_clicked(move |btn| {
                let next = count.get() + 1;
                count.set(next);
                btn.set_child(Some(&icon_label(None, true, &next.to_string(), 16)));
                sender.emit(AppMsg::GivePastel(id.clone()));
            });
        }
        meta.append(&pastel);
    }

    let quote = action_btn("dn-quote-symbolic", "Quote");
    let quoted = commit.clone();
    let quote_sender = ctx.sender.clone();
    quote.connect_clicked(move |_| {
        quote_sender.emit(AppMsg::StartQuote(quoted.clone()));
    });
    meta.append(&quote);

    let reply = action_btn("dn-message-square-reply-symbolic", "Reply");
    let id = commit.id.clone();
    let reply_sender = ctx.sender.clone();
    reply.connect_clicked(move |_| {
        reply_sender.emit(AppMsg::ReplyTo(id.clone()));
    });
    meta.append(&reply);

    if commit.body.as_ref().is_some_and(|b| !b.is_empty()) {
        let translate = action_btn("dn-languages-symbolic", "Translate");
        let body = commit.body.clone().unwrap_or_default();
        let translate_sender = ctx.sender.clone();
        translate.connect_clicked(move |_| {
            translate_sender.emit(AppMsg::Translate(body.clone()));
        });
        meta.append(&translate);
    }

    let replies = commit.replies_count.unwrap_or(0);
    if replies > 0 {
        meta.append(&replies_button(
            commit,
            ctx.sender.clone(),
            ctx.expanded.contains(&commit.id),
        ));
    } else if focused {
        let none = gtk::Label::new(Some("No replies yet"));
        none.add_css_class("dn-muted");
        none.add_css_class("dn-mono");
        meta.append(&none);
    }

    let spacer = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    spacer.set_hexpand(true);
    meta.append(&spacer);

    let link = icon_only_button("dn-link-symbolic", "Copy link", false);
    link.add_css_class("dn-action");
    let id = commit.id.clone();
    let link_sender = ctx.sender.clone();
    link.connect_clicked(move |_| {
        link_sender.emit(AppMsg::CopyPermalink(id.clone()));
    });
    meta.append(&link);

    if can_report(commit, ctx.me_id.as_deref()) {
        let report = icon_only_button("dn-flag-symbolic", "Report commit", false);
        report.add_css_class("dn-action");
        let reported = commit.clone();
        let report_sender = ctx.sender.clone();
        report.connect_clicked(move |_| {
            report_sender.emit(AppMsg::ReportCommit(reported.clone()));
        });
        meta.append(&report);
    }
    meta
}

fn can_report(commit: &Commit, me_id: Option<&str>) -> bool {
    me_id.is_some()
        && !commit.is_own(me_id)
        && !commit.is_deleted()
        && commit.status.as_deref().unwrap_or("approved") == "approved"
}

fn replies_button(commit: &Commit, sender: Sender<AppMsg>, expanded: bool) -> gtk::Button {
    let n = commit.replies_count.unwrap_or(0);
    let label = if expanded {
        "Hide replies".into()
    } else if n == 1 {
        "1 reply".into()
    } else {
        format!("{n} replies")
    };
    let btn = action_btn("dn-corner-down-right-symbolic", &label);
    btn.add_css_class("dn-replies");
    let id = commit.id.clone();
    btn.connect_clicked(move |_| {
        sender.emit(AppMsg::ToggleReplies(id.clone()));
    });
    btn
}

fn quoted_commit_widget(quoted: Option<&Commit>, sender: Sender<AppMsg>) -> gtk::Widget {
    let card = gtk::Box::new(gtk::Orientation::Vertical, 4);
    card.add_css_class("dn-quote");
    match quoted.filter(|c| !c.is_deleted()) {
        None => {
            let label = gtk::Label::new(Some(if quoted.is_none() {
                "[unavailable]  This commit isn't available."
            } else {
                "[deleted]  This commit was deleted."
            }));
            label.add_css_class("dn-muted");
            label.add_css_class("dn-mono");
            label.set_xalign(0.0);
            card.append(&label);
        }
        Some(commit) => {
            let head = gtk::Box::new(gtk::Orientation::Horizontal, 6);
            if let Some(author) = &commit.author {
                head.append(&avatar_widget(Some(author), 20, false));
                let name = gtk::Label::new(Some(author.display_name()));
                name.add_css_class("dn-name");
                head.append(&name);
                let handle = gtk::Label::new(Some(&format!("@{}", author.username)));
                handle.add_css_class("dn-handle");
                head.append(&handle);
            }
            card.append(&head);
            if let Some(body) = &commit.body {
                card.append(&body::body_widget(
                    body,
                    "dn-body",
                    commit.mentions.as_ref(),
                    Some(&sender),
                ));
            }
            if !commit.images.is_empty() {
                card.append(&image_gallery(
                    commit.images.iter().map(|image| {
                        (
                            image.url.as_deref(),
                            image.width,
                            image.height,
                            image.description.as_deref(),
                        )
                    }),
                    &sender,
                ));
            }
            let id = commit.id.clone();
            let click = gtk::GestureClick::new();
            click.connect_released(move |_, _, _, _| {
                sender.emit(AppMsg::OpenCommit(id.clone()));
            });
            card.add_controller(click);
            pointer(&card);
        }
    }
    card.upcast()
}

pub fn day_marker(
    label: &str,
    previous_lane: Option<&str>,
    connected: bool,
    dashed: bool,
) -> gtk::ListBoxRow {
    feed_separator(label, previous_lane, false, connected, dashed)
}

pub fn last_visit_marker(
    previous_lane: Option<&str>,
    connected: bool,
    dashed: bool,
) -> gtk::ListBoxRow {
    feed_separator("HEAD@{last visit}", previous_lane, true, connected, dashed)
}

fn feed_separator(
    label: &str,
    previous_lane: Option<&str>,
    last_visit: bool,
    connected: bool,
    dashed: bool,
) -> gtk::ListBoxRow {
    let row = gtk::ListBoxRow::new();
    row.set_activatable(false);
    row.add_css_class("dn-commit-row");
    let outer = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    outer.append(&rail(
        previous_lane.unwrap_or("#d3d7dd"),
        previous_lane,
        if dashed {
            RailKind::Dashed
        } else if connected && previous_lane.is_some() {
            RailKind::Segment
        } else {
            RailKind::Day
        },
    ));
    let line = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    line.add_css_class("dn-day");
    if last_visit {
        line.add_css_class("dn-last-visit");
    }
    line.set_hexpand(true);
    let text = gtk::Label::new(Some(label));
    text.add_css_class("dn-mono");
    text.add_css_class("dn-muted");
    ellipsize_label(&text);
    line.append(&text);
    if last_visit {
        let dash = gtk::Box::new(gtk::Orientation::Horizontal, 0);
        dash.add_css_class("dn-last-visit-rule");
        dash.set_hexpand(true);
        dash.set_valign(gtk::Align::Center);
        line.append(&dash);
    }
    outer.append(&line);
    row.set_child(Some(&outer));
    row
}

pub fn last_visit_id(
    commits: &[Commit],
    last_visit: Option<&str>,
    me_id: Option<&str>,
) -> Option<String> {
    let stamp = last_visit?;
    let seen_at = DateTime::parse_from_rfc3339(stamp)
        .ok()?
        .with_timezone(&Utc);
    let first_seen = commits
        .iter()
        .position(|commit| commit.created_at.is_some_and(|created| created <= seen_at))?;
    commits[..first_seen]
        .iter()
        .any(|commit| commit.author_id() != me_id)
        .then(|| commits[first_seen].id.clone())
}

pub fn paint_new_posts(button: &gtk::Button, count: u32, authors: &[User], away: bool) {
    button.set_visible(count > 0 && away);
    if count == 0 {
        button.set_child(None::<&gtk::Widget>);
        return;
    }
    let row = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    row.set_valign(gtk::Align::Center);
    row.append(&named_icon("dn-arrow-up-symbolic", 16));
    if !authors.is_empty() {
        let stack = gtk::Box::new(gtk::Orientation::Horizontal, 0);
        stack.add_css_class("dn-new-posts-avatars");
        for (index, author) in authors.iter().take(3).enumerate() {
            let avatar = avatar_widget(Some(author), 22, false);
            avatar.add_css_class("dn-new-posts-avatar");
            if index > 0 {
                avatar.set_margin_start(-8);
            }
            stack.append(&avatar);
        }
        row.append(&stack);
    }
    let label = if count == 1 {
        "1 new commit".to_owned()
    } else {
        format!("{count} new commits")
    };
    row.append(&gtk::Label::new(Some(&label)));
    button.set_child(Some(&row));
    button.set_tooltip_text(Some("Jump to newest"));
}

pub fn root_row(detail: &str) -> gtk::ListBoxRow {
    history_end_row(detail, false)
}

fn history_end_row(detail: &str, incoming: bool) -> gtk::ListBoxRow {
    let row = gtk::ListBoxRow::new();
    row.set_activatable(false);
    let outer = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    let gray = if adw_is_dark() { "#36363a" } else { "#d3d7dd" };
    outer.append(&rail(gray, incoming.then_some(gray), RailKind::Root));
    let box_ = gtk::Box::new(gtk::Orientation::Vertical, 4);
    box_.add_css_class("dn-root");
    box_.set_hexpand(true);
    let label = gtk::Label::new(Some(detail));
    label.add_css_class("dn-muted");
    shrink_label(&label);
    box_.append(&label);
    outer.append(&box_);
    row.set_child(Some(&outer));
    row
}

pub fn empty_row(fatal: &str, detail: &str) -> gtk::ListBoxRow {
    let row = gtk::ListBoxRow::new();
    row.set_activatable(false);
    let box_ = gtk::Box::new(gtk::Orientation::Vertical, 8);
    box_.set_margin_top(32);
    box_.set_margin_bottom(32);
    box_.set_margin_start(24);
    box_.set_margin_end(24);
    box_.set_halign(gtk::Align::Center);
    let f = gtk::Label::new(Some(fatal));
    f.add_css_class("dn-fatal");
    shrink_label(&f);
    f.set_justify(gtk::Justification::Center);
    f.set_xalign(0.5);
    box_.append(&f);
    let d = gtk::Label::new(Some(detail));
    d.add_css_class("dn-muted");
    shrink_label(&d);
    d.set_justify(gtk::Justification::Center);
    d.set_xalign(0.5);
    d.set_max_width_chars(40);
    box_.append(&d);
    row.set_child(Some(&box_));
    row
}

const GITHUB_README_HOWTO: &str = "https://docs.github.com/en/account-and-profile/setting-up-and-managing-your-github-profile/customizing-your-profile/managing-your-profile-readme";

pub fn profile_readme_row(
    result: &Result<Option<String>, ApiError>,
    handle: &str,
    github_login: &str,
    own: bool,
    sender: Sender<AppMsg>,
) -> gtk::ListBoxRow {
    match result {
        Err(_) => readme_empty(
            "fatal: unable to access 'https://github.com/'",
            "GitHub didn't answer. Try again in a few minutes.",
            None,
            sender,
        ),
        Ok(None) => {
            let detail = if own {
                format!(
                    "Add a README.md to a GitHub repository named {github_login}, and it shows up here."
                )
            } else {
                format!("@{handle} doesn't have a profile README on GitHub.")
            };
            readme_empty(
                "fatal: README.md: No such file or directory",
                &detail,
                own.then_some(("How to", GITHUB_README_HOWTO)),
                sender,
            )
        }
        Ok(Some(source)) => readme_content(source, github_login, sender),
    }
}

fn readme_empty(
    fatal: &str,
    detail: &str,
    extra: Option<(&str, &str)>,
    sender: Sender<AppMsg>,
) -> gtk::ListBoxRow {
    let row = gtk::ListBoxRow::new();
    row.set_activatable(false);
    let box_ = gtk::Box::new(gtk::Orientation::Vertical, 8);
    box_.set_margin_top(32);
    box_.set_margin_bottom(32);
    box_.set_margin_start(24);
    box_.set_margin_end(24);
    box_.set_halign(gtk::Align::Center);
    let f = gtk::Label::new(Some(fatal));
    f.add_css_class("dn-fatal");
    f.set_wrap(true);
    f.set_justify(gtk::Justification::Center);
    box_.append(&f);
    let d = gtk::Label::new(Some(detail));
    d.add_css_class("dn-muted");
    d.set_wrap(true);
    d.set_max_width_chars(48);
    d.set_justify(gtk::Justification::Center);
    box_.append(&d);
    if let Some((label, url)) = extra {
        let link = gtk::Button::with_label(label);
        link.add_css_class("flat");
        link.add_css_class("dn-profile-link");
        pointer(&link);
        let url = url.to_owned();
        link.connect_clicked(move |_| sender.emit(AppMsg::OpenWeb(url.clone())));
        box_.append(&link);
    }
    row.set_child(Some(&box_));
    row
}

fn readme_content(source: &str, github_login: &str, sender: Sender<AppMsg>) -> gtk::ListBoxRow {
    let row = gtk::ListBoxRow::new();
    row.set_activatable(false);
    let wrap = gtk::Box::new(gtk::Orientation::Vertical, 12);
    wrap.add_css_class("dn-readme");
    wrap.set_hexpand(true);
    let markdown = readme_markdown(source);
    for part in readme_parts(&markdown, github_login) {
        match part {
            ReadmePart::Markdown(text) => {
                if text.trim().is_empty() {
                    continue;
                }
                let body = body::body_widget(&text, "dn-readme-body", None, Some(&sender));
                wrap.append(&body);
            }
            ReadmePart::Image { url, alt } => {
                wrap.append(&readme_image(&url, &alt, &sender));
            }
        }
    }
    let source_url = format!("https://github.com/{github_login}/{github_login}");
    let source_btn = gtk::Button::with_label(&format!("{github_login}/README.md on GitHub"));
    source_btn.add_css_class("flat");
    source_btn.add_css_class("dn-readme-source");
    source_btn.set_halign(gtk::Align::Start);
    pointer(&source_btn);
    source_btn.connect_clicked(move |_| sender.emit(AppMsg::OpenWeb(source_url.clone())));
    wrap.append(&source_btn);
    row.set_child(Some(&wrap));
    row
}

fn readme_image(url: &str, alt: &str, sender: &Sender<AppMsg>) -> gtk::Widget {
    let pic = gtk::Picture::new();
    pic.add_css_class("dn-readme-image");
    pic.set_content_fit(gtk::ContentFit::Contain);
    pic.set_hexpand(true);
    pic.set_can_shrink(true);
    if !alt.is_empty() {
        pic.set_tooltip_text(Some(alt));
    }
    images::bind_picture(&pic, Some(url));
    open_on_click(&pic, Some(url), sender);
    pic.upcast()
}

#[derive(Debug, PartialEq, Eq)]
enum ReadmePart {
    Markdown(String),
    Image { url: String, alt: String },
}

fn readme_markdown(source: &str) -> String {
    let trimmed = source.trim();
    if trimmed.starts_with('<') {
        htmlish_to_markdown(trimmed)
    } else {
        trimmed.to_owned()
    }
}

fn htmlish_to_markdown(input: &str) -> String {
    let mut out = String::new();
    let mut rest = input;
    while let Some(start) = rest.find('<') {
        out.push_str(&rest[..start]);
        let after = &rest[start..];
        let Some(end) = after.find('>') else {
            out.push_str(after);
            break;
        };
        let tag = &after[..=end];
        let lower = tag.to_ascii_lowercase();
        if lower.starts_with("<img") {
            if let Some(src) = html_attr(tag, "src") {
                let alt = html_attr(tag, "alt").unwrap_or_default();
                out.push_str(&format!("![{alt}]({src})"));
            }
        } else if lower.starts_with("<br") {
            out.push('\n');
        } else if lower.starts_with("<hr") {
            out.push_str("\n\n---\n\n");
        } else if lower.starts_with("</p")
            || lower.starts_with("</div")
            || lower.starts_with("</h")
            || lower.starts_with("</li")
            || lower.starts_with("</tr")
        {
            out.push('\n');
        }
        rest = &after[end + 1..];
    }
    out.push_str(rest);
    out
}

fn html_attr(tag: &str, name: &str) -> Option<String> {
    let lower = tag.to_ascii_lowercase();
    let needle = format!("{name}=");
    let i = lower.find(&needle)?;
    let rest = tag[i + needle.len()..].trim_start();
    let quote = rest.chars().next()?;
    if quote == '"' || quote == '\'' {
        let rest = &rest[1..];
        let end = rest.find(quote)?;
        Some(rest[..end].to_owned())
    } else {
        let end = rest
            .find(|c: char| c.is_whitespace() || c == '>')
            .unwrap_or(rest.len());
        Some(rest[..end].to_owned())
    }
}

fn github_asset_url(login: &str, src: &str) -> String {
    let src = src.trim();
    if src.starts_with("https://") || src.starts_with("http://") || src.starts_with("data:") {
        return src.to_owned();
    }
    let path = src.trim_start_matches("./").trim_start_matches('/');
    format!("https://raw.githubusercontent.com/{login}/{login}/HEAD/{path}")
}

fn readme_parts(markdown: &str, login: &str) -> Vec<ReadmePart> {
    let mut parts = Vec::new();
    let mut rest = markdown;
    loop {
        let md_at = rest.find("![");
        let html_at = rest
            .to_ascii_lowercase()
            .find("<img")
            .and_then(|i| rest.get(i..).map(|_| i));
        match (md_at, html_at) {
            (None, None) => {
                if !rest.is_empty() {
                    parts.push(ReadmePart::Markdown(rest.to_owned()));
                }
                break;
            }
            (Some(md), Some(html)) if html < md => {
                rest = take_html_image(rest, html, login, &mut parts);
            }
            (None, Some(html)) => {
                rest = take_html_image(rest, html, login, &mut parts);
            }
            (Some(md), _) => {
                rest = take_md_image(rest, md, login, &mut parts);
            }
        }
    }
    parts
}

fn take_md_image<'a>(
    rest: &'a str,
    at: usize,
    login: &str,
    parts: &mut Vec<ReadmePart>,
) -> &'a str {
    if at > 0 {
        parts.push(ReadmePart::Markdown(rest[..at].to_owned()));
    }
    let after = &rest[at + 2..];
    let Some(alt_end) = after.find("](") else {
        parts.push(ReadmePart::Markdown(rest[at..].to_owned()));
        return "";
    };
    let alt = after[..alt_end].to_owned();
    let url_part = &after[alt_end + 2..];
    let Some(url_end) = url_part.find(')') else {
        parts.push(ReadmePart::Markdown(rest[at..].to_owned()));
        return "";
    };
    let raw = url_part[..url_end].trim();
    let url = raw.split_whitespace().next().unwrap_or(raw);
    parts.push(ReadmePart::Image {
        url: github_asset_url(login, url),
        alt,
    });
    &url_part[url_end + 1..]
}

fn take_html_image<'a>(
    rest: &'a str,
    at: usize,
    login: &str,
    parts: &mut Vec<ReadmePart>,
) -> &'a str {
    if at > 0 {
        parts.push(ReadmePart::Markdown(rest[..at].to_owned()));
    }
    let after = &rest[at..];
    let Some(end) = after.find('>') else {
        parts.push(ReadmePart::Markdown(after.to_owned()));
        return "";
    };
    let tag = &after[..=end];
    if let Some(src) = html_attr(tag, "src") {
        parts.push(ReadmePart::Image {
            url: github_asset_url(login, &src),
            alt: html_attr(tag, "alt").unwrap_or_default(),
        });
    }
    &after[end + 1..]
}

pub fn append_notifications(list: &gtk::ListBox, items: &[Notification], sender: Sender<AppMsg>) {
    let gray = if adw_is_dark() { "#36363a" } else { "#d3d7dd" };
    let mut last_day = None;
    let mut previous = false;
    let mut prev_new = false;
    for item in items {
        if previous && prev_new && !item.is_unread() {
            list.append(&last_visit_marker(Some(gray), true, false));
        }
        if let Some(created) = item.created_at {
            let key = day_key(created);
            if last_day.as_ref() != Some(&key) {
                list.append(&day_marker(
                    &day_label(created),
                    previous.then_some(gray),
                    previous,
                    false,
                ));
                last_day = Some(key);
            }
        }
        list.append(&notification_row(item, previous, sender.clone()));
        previous = true;
        prev_new = item.is_unread();
    }
    list.append(&history_end_row(
        "That's the oldest notification. There's nothing before it.",
        true,
    ));
}

fn notification_row(
    item: &Notification,
    has_previous: bool,
    sender: Sender<AppMsg>,
) -> gtk::ListBoxRow {
    let row = gtk::ListBoxRow::new();
    row.add_css_class("dn-commit-row");
    row.add_css_class("dn-commit");
    let kind = item.kind();
    let follow = matches!(kind, "follow" | "followed");
    row.set_activatable(item.commit.is_some() || follow);
    if item.commit.is_some() || follow {
        pointer(&row);
    }
    let outer = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    outer.set_hexpand(true);
    outer.append(&notification_rail(item, has_previous));
    let content = gtk::Box::new(gtk::Orientation::Vertical, 6);
    content.add_css_class("dn-commit-content");
    content.set_hexpand(true);

    let header = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    header.set_hexpand(true);
    header.set_valign(gtk::Align::Center);
    let moderation = matches!(kind, "approved" | "rejected");
    if moderation {
        let chip = gtk::Label::new(Some(kind));
        chip.add_css_class("dn-mono");
        chip.add_css_class("dn-notif-chip");
        if kind == "rejected" {
            chip.add_css_class("dn-over");
        } else {
            chip.add_css_class("dn-muted");
        }
        header.append(&chip);
    } else {
        header.append(&notification_avatars(item));
        header.append(&notification_who(item));
    }
    let time = gtk::Label::new(Some(&relative_time(item.created_at)));
    time.add_css_class("dn-muted");
    time.add_css_class("dn-mono");
    time.set_ellipsize(pango::EllipsizeMode::End);
    time.set_width_chars(1);
    time.set_hexpand(true);
    time.set_xalign(1.0);
    time.set_valign(gtk::Align::Center);
    header.append(&time);
    content.append(&header);

    let title = gtk::Label::new(None);
    title.set_use_markup(true);
    title.set_markup(&notification_headline(item));
    shrink_label(&title);
    content.append(&title);
    if let Some(body) = item.commit.as_ref().and_then(|c| c.body.as_deref()) {
        if !matches!(kind, "follow" | "followed") {
            let excerpt = gtk::Label::new(Some(body));
            excerpt.add_css_class("dn-muted");
            excerpt.set_xalign(0.0);
            excerpt.set_wrap(true);
            excerpt.set_wrap_mode(pango::WrapMode::WordChar);
            excerpt.set_ellipsize(pango::EllipsizeMode::End);
            excerpt.set_lines(4);
            excerpt.set_width_chars(1);
            if matches!(kind, "starred" | "star" | "approved" | "rejected") {
                let card = gtk::Box::new(gtk::Orientation::Vertical, 0);
                card.add_css_class("dn-notif-excerpt");
                card.append(&excerpt);
                content.append(&card);
            } else {
                content.append(&excerpt);
            }
        }
    }
    outer.append(&content);
    row.set_child(Some(&outer));
    if let Some(commit) = &item.commit {
        let id = commit.id.clone();
        row.connect_activate(move |_| {
            sender.emit(AppMsg::OpenCommit(id.clone()));
        });
    } else if follow {
        if let Some(username) = item
            .actor
            .as_ref()
            .or(item.actors.first())
            .map(|user| user.username.clone())
        {
            row.connect_activate(move |_| {
                sender.emit(AppMsg::OpenProfile(username.clone()));
            });
        }
    }
    row
}

fn notification_rail(item: &Notification, has_previous: bool) -> gtk::Widget {
    let gray = if adw_is_dark() { "#36363a" } else { "#d3d7dd" };
    let area = rail_area(
        gray,
        has_previous.then_some(gray),
        RailKind::Commit {
            incoming: has_previous,
            terminator: false,
            dashed_in: false,
        },
        true,
    );
    let overlay = gtk::Overlay::new();
    overlay.add_css_class("dn-rail");
    overlay.set_hexpand(false);
    overlay.set_vexpand(true);
    overlay.set_overflow(gtk::Overflow::Visible);
    overlay.set_size_request(RAIL_WIDTH, -1);
    overlay.set_child(Some(&area));

    let node = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    node.add_css_class("dn-notif-node");
    if item.is_unread() {
        node.add_css_class("unread");
    }
    node.set_halign(gtk::Align::Center);
    node.set_valign(gtk::Align::Start);
    node.set_margin_top(14);
    node.set_size_request(36, 36);
    let kind = item.kind();
    let icon = named_icon(notification_icon(kind), 16);
    icon.set_halign(gtk::Align::Center);
    icon.set_valign(gtk::Align::Center);
    icon.set_hexpand(true);
    let moderation = matches!(kind, "approved" | "rejected");
    if moderation {
        icon.add_css_class("dn-muted");
    } else {
        icon.add_css_class("dn-notif-icon");
        icon.add_css_class(lane_class(item.actor.as_ref().or(item.actors.first())));
    }
    node.append(&icon);
    overlay.add_overlay(&node);
    overlay.upcast()
}

fn notification_icon(kind: &str) -> &'static str {
    match kind {
        "follow" | "followed" => "dn-user-plus-symbolic",
        "reply" | "replied" => "dn-message-square-reply-symbolic",
        "quote" | "quoted" => "dn-quote-symbolic",
        "star" | "starred" => "dn-star-filled-symbolic",
        "mention" | "mentioned" => "dn-at-sign-symbolic",
        "approved" => "dn-check-symbolic",
        "rejected" => "dn-x-symbolic",
        _ => "dn-bell-symbolic",
    }
}

fn notification_who(item: &Notification) -> gtk::Widget {
    let names = gtk::Box::new(gtk::Orientation::Horizontal, 6);
    names.set_hexpand(true);
    names.set_valign(gtk::Align::Center);
    if let Some(actor) = item.actor.as_ref().or(item.actors.first()) {
        let name = gtk::Label::new(Some(actor.display_name()));
        name.add_css_class("dn-name");
        ellipsize_label(&name);
        names.append(&name);
        let handle = gtk::Label::new(Some(&format!("@{}", actor.username)));
        handle.add_css_class("dn-handle");
        ellipsize_label(&handle);
        names.append(&handle);
    }
    names.upcast()
}

fn notification_headline(item: &Notification) -> String {
    let kind = item.kind();
    if kind == "approved" {
        return "Your commit was approved.".into();
    }
    if kind == "rejected" {
        return "Your commit was rejected.".into();
    }
    let action = match kind {
        "follow" | "followed" => "followed you",
        "star" | "starred" => "starred your commit",
        "reply" | "replied" => "replied to your commit",
        "quote" | "quoted" => "quoted your commit",
        "mention" | "mentioned" => "mentioned you",
        other if !other.is_empty() => other,
        _ => "notified you",
    };
    let actor = item.actor.as_ref().or(item.actors.first());
    let name = glib::markup_escape_text(actor.map(User::display_name).unwrap_or("someone"));
    let others = if matches!(kind, "follow" | "followed" | "star" | "starred") {
        item.actors_count.unwrap_or(1).saturating_sub(1)
    } else {
        0
    };
    if others > 0 {
        let noun = if others == 1 { "other" } else { "others" };
        format!("<span weight=\"600\">{name}</span> and {others} {noun} {action}")
    } else {
        let username =
            glib::markup_escape_text(actor.map(|user| user.username.as_str()).unwrap_or(""));
        format!(
            "<span weight=\"600\">{name}</span> <span font_family=\"monospace\" size=\"small\" alpha=\"70%\">@{username}</span> {action}"
        )
    }
}

fn notification_avatars(item: &Notification) -> gtk::Widget {
    let stack = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    stack.add_css_class("dn-notif-avatars");
    let mut people: Vec<&User> = Vec::new();
    if let Some(actor) = item.actor.as_ref() {
        people.push(actor);
    }
    for actor in &item.actors {
        if people.iter().all(|seen| seen.id != actor.id) {
            people.push(actor);
        }
    }
    let count = item
        .actors_count
        .unwrap_or(people.len() as i64)
        .max(people.len() as i64);
    let shown: Vec<&User> = people.into_iter().take(3).collect();
    let extra = count.saturating_sub(shown.len() as i64);
    for (index, actor) in shown.into_iter().enumerate() {
        let avatar = avatar_widget(Some(actor), 32, false);
        if index > 0 {
            avatar.set_margin_start(-8);
        }
        stack.append(&avatar);
    }
    if extra > 0 {
        let more = gtk::Label::new(Some(&format!("+{extra}")));
        more.add_css_class("dn-mono");
        more.add_css_class("dn-muted");
        more.add_css_class("dn-notif-more");
        more.set_margin_start(-8);
        stack.append(&more);
    }
    stack.upcast()
}

pub fn inbox_row(item: &ConversationSummary, sender: Sender<AppMsg>) -> gtk::ListBoxRow {
    let row = gtk::ListBoxRow::new();
    row.add_css_class("dn-inbox-row");
    row.set_activatable(true);
    pointer(&row);
    let box_ = gtk::Box::new(gtk::Orientation::Horizontal, 10);
    let user = item.partner.as_ref();
    box_.append(&avatar_widget(user, 36, true));
    let text = gtk::Box::new(gtk::Orientation::Vertical, 2);
    text.set_hexpand(true);
    let title = gtk::Label::new(Some(&item.title()));
    title.add_css_class("dn-name");
    ellipsize_label(&title);
    text.append(&title);
    let preview = gtk::Label::new(Some(&item.preview()));
    preview.add_css_class("dn-muted");
    ellipsize_label(&preview);
    text.append(&preview);
    box_.append(&text);
    if item.ahead > 0 {
        let badge = gtk::Label::new(Some(&format!("{} ahead", item.ahead)));
        badge.add_css_class("dn-ahead");
        badge.set_valign(gtk::Align::Center);
        box_.append(&badge);
    }
    row.set_child(Some(&box_));
    let reference = devnull_api::ThreadRef::from_summary(item);
    row.connect_activate(move |_| {
        sender.emit(AppMsg::OpenThread(reference.clone()));
    });
    row
}

pub fn message_row(message: &Message, me: Option<&str>, sender: Sender<AppMsg>) -> gtk::ListBoxRow {
    let row = gtk::ListBoxRow::new();
    row.set_activatable(false);
    let box_ = gtk::Box::new(gtk::Orientation::Vertical, 2);
    box_.add_css_class("dn-message");
    box_.set_hexpand(true);
    if message.reply_to_id.is_some() {
        box_.set_margin_start(28);
        box_.add_css_class("dn-nested");
        let hint = gtk::Label::new(Some("↳ reply"));
        hint.add_css_class("dn-mono");
        hint.add_css_class("dn-muted");
        hint.set_xalign(0.0);
        box_.append(&hint);
    }
    let mine = message.user_id.as_deref() == me;
    if mine {
        box_.add_css_class("mine");
    }
    let header = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    header.append(&avatar_widget(message.user.as_ref(), 24, false));
    if let Some(user) = &message.user {
        let who = gtk::Label::new(Some(&format!("@{}", user.username)));
        who.add_css_class("dn-handle");
        ellipsize_label(&who);
        header.append(&who);
    }
    let time = gtk::Label::new(Some(&relative_time(message.created_at)));
    time.add_css_class("dn-mono");
    time.add_css_class("dn-muted");
    header.append(&time);
    box_.append(&header);
    if let Some(body) = &message.body {
        box_.append(&body::body_widget(
            body,
            "dn-body",
            message.mentions.as_ref(),
            Some(&sender),
        ));
    }
    if !message.images.is_empty() {
        box_.append(&image_gallery(
            message.images.iter().map(|image| {
                (
                    Some(image.url.as_str()),
                    image.width,
                    image.height,
                    image.description.as_deref(),
                )
            }),
            &sender,
        ));
    }
    if let Some(preview) = &message.link_preview {
        box_.append(&link_preview_widget(preview, None, sender));
    }
    row.set_child(Some(&box_));
    row
}

fn link_preview_widget(
    preview: &LinkPreview,
    commit_id: Option<&str>,
    sender: Sender<AppMsg>,
) -> gtk::Widget {
    let wrap = gtk::Box::new(gtk::Orientation::Vertical, 0);
    wrap.set_hexpand(true);
    fill_link_preview(&wrap, preview, sender.clone());
    if preview.is_pending.unwrap_or(false) {
        if let Some(id) = commit_id.map(ToOwned::to_owned) {
            poll_link_preview(id, wrap.clone(), sender);
        }
    }
    wrap.upcast()
}

fn poll_link_preview(commit_id: String, wrap: gtk::Box, sender: Sender<AppMsg>) {
    let weak = glib::SendWeakRef::from(wrap.downgrade());
    relm4::spawn(async move {
        let Ok(client) = Client::unsigned() else {
            return;
        };
        for delay in [1000_u64, 2000, 3000, 5000, 8000, 13000] {
            tokio::time::sleep(std::time::Duration::from_millis(delay)).await;
            let Ok(Some(preview)) = client.commit_link_preview(&commit_id).await else {
                continue;
            };
            if preview.is_pending.unwrap_or(false) {
                continue;
            }
            let sender = sender.clone();
            let weak = weak.clone();
            glib::idle_add_once(move || {
                if let Some(wrap) = weak.upgrade() {
                    fill_link_preview(&wrap, &preview, sender);
                }
            });
            return;
        }
        let weak = weak.clone();
        glib::idle_add_once(move || {
            if let Some(wrap) = weak.upgrade() {
                while let Some(child) = wrap.first_child() {
                    wrap.remove(&child);
                }
            }
        });
    });
}

fn fill_link_preview(wrap: &gtk::Box, preview: &LinkPreview, sender: Sender<AppMsg>) {
    while let Some(child) = wrap.first_child() {
        wrap.remove(&child);
    }
    if let Some(embed) = preview.embed.as_ref() {
        wrap.append(&embed_card(preview, embed, sender));
        return;
    }
    if preview.is_pending.unwrap_or(false) {
        wrap.append(&link_skeleton());
        return;
    }
    if preview.title.as_deref().is_none_or(str::is_empty) {
        return;
    }
    wrap.append(&og_card(preview, sender));
}

fn link_skeleton() -> gtk::Widget {
    let row = gtk::Box::new(gtk::Orientation::Horizontal, 12);
    row.add_css_class("dn-link-card");
    let thumb = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    thumb.add_css_class("dn-link-thumb");
    thumb.add_css_class("dn-skeleton-block");
    thumb.add_css_class("dn-skeleton-pulse");
    thumb.set_size_request(80, 80);
    row.append(&thumb);
    let col = gtk::Box::new(gtk::Orientation::Vertical, 6);
    col.set_valign(gtk::Align::Center);
    col.set_hexpand(true);
    for class in ["dn-skeleton-line", "dn-skeleton-line", "dn-skeleton-line"] {
        let line = gtk::Box::new(gtk::Orientation::Horizontal, 0);
        line.add_css_class(class);
        line.add_css_class("dn-skeleton-pulse");
        line.set_size_request(-1, 10);
        line.set_hexpand(true);
        col.append(&line);
    }
    row.append(&col);
    row.upcast()
}

fn og_card(preview: &LinkPreview, sender: Sender<AppMsg>) -> gtk::Widget {
    let btn = gtk::Button::new();
    btn.add_css_class("flat");
    btn.add_css_class("dn-link-card");
    pointer(&btn);
    let url = preview.url.clone();
    btn.connect_clicked(move |_| sender.emit(AppMsg::OpenWeb(url.clone())));
    let row = gtk::Box::new(gtk::Orientation::Horizontal, 12);
    if let Some(image) = preview.image_url.as_deref() {
        let pic = gtk::Picture::new();
        pic.add_css_class("dn-link-thumb");
        pic.set_content_fit(gtk::ContentFit::Cover);
        pic.set_size_request(80, 80);
        images::bind_picture(&pic, Some(image));
        row.append(&pic);
    }
    row.append(&link_details(
        preview
            .site_name
            .as_deref()
            .unwrap_or(&hostname(&preview.url)),
        preview.title.as_deref(),
        preview.description.as_deref(),
    ));
    btn.set_child(Some(&row));
    btn.upcast()
}

fn embed_card(preview: &LinkPreview, embed: &Embed, sender: Sender<AppMsg>) -> gtk::Widget {
    let card = gtk::Box::new(gtk::Orientation::Vertical, 0);
    card.add_css_class("dn-embed");
    card.set_hexpand(true);
    card.set_halign(gtk::Align::Fill);
    card.set_overflow(gtk::Overflow::Hidden);
    pointer(&card);
    open_on_click(&card, Some(&preview.url), &sender);
    let poster = preview.image_url.as_deref().or(embed.poster.as_deref());
    let is_video = embed.kind.as_deref() == Some("video")
        || embed
            .aspect_ratio
            .as_deref()
            .is_some_and(|ratio| !ratio.is_empty());
    if is_video {
        let ratio = parse_aspect(embed.aspect_ratio.as_deref()).unwrap_or(16.0 / 9.0);
        let frame = gtk::AspectFrame::new(0.5, 0.5, ratio, false);
        frame.set_obey_child(false);
        frame.add_css_class("dn-embed-frame");
        frame.set_hexpand(true);
        frame.set_halign(gtk::Align::Fill);
        frame.set_child(Some(&play_poster(poster, true)));
        if ratio < 1.0 {
            frame.add_css_class("dn-embed-tall");
            frame.set_halign(gtk::Align::Center);
            frame.set_hexpand(false);
            let clamp = adw::Clamp::new();
            clamp.set_maximum_size(320);
            clamp.set_tightening_threshold(320);
            clamp.set_hexpand(true);
            clamp.set_child(Some(&frame));
            card.append(&clamp);
        } else {
            card.append(&frame);
        }
    } else {
        card.add_css_class("dn-embed-audio");
        let row = gtk::Box::new(gtk::Orientation::Horizontal, 0);
        row.append(&play_poster(poster, false));
        let details = embed_details(preview, embed);
        details.add_css_class("dn-embed-details");
        row.append(&details);
        card.append(&row);
        return card.upcast();
    }
    let details = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    details.add_css_class("dn-embed-details");
    details.append(&embed_details(preview, embed));
    card.append(&details);
    card.upcast()
}

fn embed_details(preview: &LinkPreview, embed: &Embed) -> gtk::Box {
    link_details(
        preview
            .site_name
            .as_deref()
            .or(embed.provider.as_deref())
            .unwrap_or(&hostname(&preview.url)),
        preview.title.as_deref(),
        None,
    )
}

fn play_poster(url: Option<&str>, fill: bool) -> gtk::Widget {
    let overlay = gtk::Overlay::new();
    overlay.add_css_class("dn-embed-poster");
    overlay.set_overflow(gtk::Overflow::Hidden);
    overlay.set_can_target(false);
    if fill {
        overlay.set_hexpand(true);
        overlay.set_vexpand(true);
        overlay.set_halign(gtk::Align::Fill);
        overlay.set_valign(gtk::Align::Fill);
    } else {
        overlay.set_size_request(80, 80);
        overlay.set_halign(gtk::Align::Start);
        overlay.set_valign(gtk::Align::Center);
        overlay.add_css_class("dn-embed-audio-thumb");
    }
    let pic = cover_picture();
    pic.set_can_target(false);
    images::bind_picture(&pic, url);
    overlay.set_child(Some(&pic));
    let play = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    play.add_css_class("dn-embed-play");
    play.set_halign(gtk::Align::Center);
    play.set_valign(gtk::Align::Center);
    play.set_can_target(false);
    let icon = gtk::Image::from_icon_name("dn-play-symbolic");
    icon.set_pixel_size(if fill { 28 } else { 22 });
    play.append(&icon);
    overlay.add_overlay(&play);
    overlay.upcast()
}

fn link_details(site: &str, title: Option<&str>, description: Option<&str>) -> gtk::Box {
    let col = gtk::Box::new(gtk::Orientation::Vertical, 2);
    col.set_hexpand(true);
    col.set_valign(gtk::Align::Center);
    let site_l = gtk::Label::new(Some(site));
    site_l.add_css_class("dn-mono");
    site_l.add_css_class("dn-muted");
    site_l.set_xalign(0.0);
    site_l.set_ellipsize(pango::EllipsizeMode::End);
    col.append(&site_l);
    if let Some(title) = title.filter(|t| !t.is_empty()) {
        let title_l = gtk::Label::new(Some(title));
        title_l.add_css_class("dn-link-title");
        title_l.set_xalign(0.0);
        title_l.set_wrap(true);
        title_l.set_ellipsize(pango::EllipsizeMode::End);
        title_l.set_lines(2);
        col.append(&title_l);
    }
    if let Some(description) = description.filter(|t| !t.is_empty()) {
        let desc = gtk::Label::new(Some(description));
        desc.add_css_class("dn-muted");
        desc.set_xalign(0.0);
        desc.set_wrap(true);
        desc.set_ellipsize(pango::EllipsizeMode::End);
        desc.set_lines(2);
        col.append(&desc);
    }
    col
}

fn hostname(url: &str) -> String {
    url::Url::parse(url)
        .ok()
        .and_then(|parsed| parsed.host_str().map(str::to_owned))
        .unwrap_or_else(|| url.to_owned())
}

fn parse_aspect(value: Option<&str>) -> Option<f32> {
    let value = value?;
    let mut parts = value.split('/');
    let width: f32 = parts.next()?.trim().parse().ok()?;
    let height: f32 = parts.next()?.trim().parse().ok()?;
    (height > 0.0).then_some(width / height)
}

pub fn fill_profile_hero(
    hero: &gtk::Box,
    profile: &Profile,
    me: Option<&CurrentUser>,
    follow_label: &str,
    tab: ProfileFeed,
    sender: Sender<AppMsg>,
) {
    while let Some(child) = hero.first_child() {
        hero.remove(&child);
    }
    let user = &profile.user;
    let own = me.is_some_and(|current| current.username() == user.username);
    let dark = adw_is_dark();
    let lane_hex = lane_color(Some(user), dark);

    hero.add_css_class("dn-profile-hero");
    hero.set_hexpand(true);
    hero.set_overflow(gtk::Overflow::Hidden);

    let header = gtk::Box::new(gtk::Orientation::Vertical, 12);
    header.add_css_class("dn-profile-header");
    header.set_hexpand(true);

    let top = gtk::Box::new(gtk::Orientation::Horizontal, 16);
    top.set_valign(gtk::Align::Start);
    top.set_hexpand(true);
    top.append(&avatar_widget(Some(user), 80, true));
    let spacer = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    spacer.set_hexpand(true);
    top.append(&spacer);
    top.append(&profile_actions(
        own,
        follow_label,
        me.is_some_and(CurrentUser::remotes_on),
        sender.clone(),
    ));
    header.append(&top);

    let name = gtk::Label::new(Some(user.display_name()));
    name.add_css_class("dn-title");
    shrink_label(&name);
    header.append(&name);
    let handle_row = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    handle_row.add_css_class("dn-profile-handle");
    handle_row.set_hexpand(true);
    let handle_id = gtk::Box::new(gtk::Orientation::Horizontal, 6);
    handle_id.set_hexpand(true);
    handle_id.append(&handle_dot(Some(user)));
    let handle = gtk::Label::new(Some(&format!("@{}", user.username)));
    handle.add_css_class("dn-handle");
    ellipsize_label(&handle);
    handle_id.append(&handle);
    if profile.follows_you {
        let chip = gtk::Label::new(Some("Follows you"));
        chip.add_css_class("dn-profile-chip");
        handle_id.append(&chip);
    }
    if profile.is_muting {
        let chip = gtk::Label::new(Some("Muted"));
        chip.add_css_class("dn-profile-chip");
        handle_id.append(&chip);
    }
    handle_row.append(&handle_id);
    if let Some(achievements) = &profile.achievements {
        let chip = gtk::Button::with_label(&achievements.label());
        chip.add_css_class("flat");
        chip.add_css_class("dn-achievements-chip");
        chip.add_css_class(&hex_tone_class(&lane_hex));
        chip.set_tooltip_text(Some("View achievements and Unix rank"));
        chip.set_halign(gtk::Align::Start);
        chip.set_hexpand(false);
        pointer(&chip);
        let inspect = sender.clone();
        chip.connect_clicked(move |_| inspect.emit(AppMsg::InspectAchievements));
        handle_row.append(&chip);
    }
    header.append(&handle_row);
    if let Some(bio) = user.bio.as_deref().filter(|s| !s.is_empty()) {
        let bio_l = gtk::Label::new(Some(bio));
        shrink_label(&bio_l);
        header.append(&bio_l);
    }
    header.append(&profile_meta(user, sender.clone()));
    header.append(&profile_counts(user, sender.clone()));
    if me.is_some_and(|current| current.pasteis_on()) {
        if let Some(received) = profile.pasteis_received {
            let pastel = gtk::Box::new(gtk::Orientation::Horizontal, 6);
            pastel.append(&pastel_icon(20));
            let count = gtk::Label::new(Some(&received.to_string()));
            count.add_css_class("dn-mono");
            pastel.append(&count);
            let caption = gtk::Label::new(Some(&format!(
                "{} received",
                if received == 1 { "pastel" } else { "pastéis" }
            )));
            caption.add_css_class("dn-mono");
            caption.add_css_class("dn-muted");
            pastel.append(&caption);
            header.append(&pastel);
        }
    }
    hero.append(&header);

    hero.append(&activity_graph(&profile.activity, &lane_hex));
    if profile.streaks.longest > 0 {
        hero.append(&streaks_row(&profile.streaks, &lane_hex));
    }
    if let Some(achievements) = &profile.achievements {
        hero.append(&achievements_shelf(achievements, &lane_hex, sender.clone()));
    }

    let tabs = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    tabs.add_css_class("dn-tabs");
    tabs.add_css_class("dn-profile-tabs");
    tabs.set_hexpand(true);
    tabs.set_homogeneous(true);
    tabs.append(&profile_tab(
        "Commits",
        tab == ProfileFeed::Commits,
        ProfileFeed::Commits,
        sender.clone(),
    ));
    tabs.append(&profile_tab(
        "Replies",
        tab == ProfileFeed::Replies,
        ProfileFeed::Replies,
        sender.clone(),
    ));
    tabs.append(&profile_tab(
        "README",
        tab == ProfileFeed::Readme,
        ProfileFeed::Readme,
        sender,
    ));
    hero.append(&tabs);
}

fn profile_actions(
    own: bool,
    follow_label: &str,
    remotes: bool,
    sender: Sender<AppMsg>,
) -> gtk::Box {
    let actions = gtk::Box::new(gtk::Orientation::Horizontal, 6);
    actions.set_valign(gtk::Align::Start);
    actions.set_halign(gtk::Align::End);

    let share = outline_icon_button("dn-share-2-symbolic", "Share profile");
    let share_sender = sender.clone();
    share.connect_clicked(move |_| share_sender.emit(AppMsg::ShareProfile));
    actions.append(&share);

    if own {
        actions.append(&profile_actions_menu(sender.clone()));
        actions.append(&edit_profile_menu(sender.clone()));
    } else {
        if remotes {
            let remote = outline_button(Some("dn-messages-square-symbolic"), false, "Remote");
            let remote_sender = sender.clone();
            remote.connect_clicked(move |_| remote_sender.emit(AppMsg::StartRemote));
            actions.append(&remote);
        }
        if !follow_label.is_empty() {
            let following = follow_label != "Follow";
            let follow = gtk::Button::with_label(if following { "Following" } else { "Follow" });
            follow.add_css_class("dn-follow-btn");
            pointer(&follow);
            if following {
                follow.add_css_class("dn-outline-btn");
                follow.add_css_class("dn-unfollow");
                let hover = gtk::EventControllerMotion::new();
                let label = follow.clone();
                hover.connect_enter(move |_, _, _| label.set_label("Unfollow"));
                let label = follow.clone();
                hover.connect_leave(move |_| label.set_label("Following"));
                follow.add_controller(hover);
            } else {
                follow.add_css_class("suggested-action");
            }
            let follow_sender = sender.clone();
            follow.connect_clicked(move |_| follow_sender.emit(AppMsg::ToggleFollowCurrent));
            actions.append(&follow);
        }
    }
    actions
}

fn profile_actions_menu(sender: Sender<AppMsg>) -> gtk::MenuButton {
    let more = gtk::MenuButton::new();
    more.add_css_class("dn-outline-btn");
    more.add_css_class("dn-icon-btn");
    more.set_icon_name("dn-ellipsis-symbolic");
    more.set_tooltip_text(Some("More"));
    more.set_always_show_arrow(false);
    pointer(&more);

    let popover = gtk::Popover::new();
    popover.set_has_arrow(false);
    popover.set_position(gtk::PositionType::Bottom);
    let list = gtk::Box::new(gtk::Orientation::Vertical, 2);
    list.set_margin_top(4);
    list.set_margin_bottom(4);
    list.set_margin_start(4);
    list.set_margin_end(4);
    list.append(&user_menu_item(
        "dn-arrow-up-right-symbolic",
        "Go live",
        {
            let sender = sender.clone();
            move || sender.emit(AppMsg::GoLive)
        },
        &popover,
    ));
    popover.set_child(Some(&list));
    more.set_popover(Some(&popover));
    more
}

fn edit_profile_menu(sender: Sender<AppMsg>) -> gtk::MenuButton {
    let edit = gtk::MenuButton::new();
    edit.add_css_class("dn-outline-btn");
    edit.set_label("Edit profile");
    edit.set_always_show_arrow(true);
    pointer(&edit);

    let popover = gtk::Popover::new();
    popover.set_has_arrow(false);
    popover.set_position(gtk::PositionType::Bottom);
    let list = gtk::Box::new(gtk::Orientation::Vertical, 2);
    list.set_margin_top(4);
    list.set_margin_bottom(4);
    list.set_margin_start(4);
    list.set_margin_end(4);
    list.append(&user_menu_item(
        "dn-github-symbolic",
        "Edit on GitHub",
        {
            let sender = sender.clone();
            move || sender.emit(AppMsg::EditOnGitHub)
        },
        &popover,
    ));
    list.append(&user_menu_item(
        "dn-refresh-cw-symbolic",
        "Sync from GitHub",
        {
            let sender = sender.clone();
            move || sender.emit(AppMsg::SyncProfile)
        },
        &popover,
    ));
    popover.set_child(Some(&list));
    edit.set_popover(Some(&popover));
    edit
}

fn outline_icon_button(icon: &str, tooltip: &str) -> gtk::Button {
    let btn = gtk::Button::new();
    btn.add_css_class("dn-outline-btn");
    btn.add_css_class("dn-icon-btn");
    btn.set_tooltip_text(Some(tooltip));
    btn.set_icon_name(icon);
    pointer(&btn);
    btn
}

fn outline_button(icon: Option<&str>, icon_after: bool, label: &str) -> gtk::Button {
    let btn = gtk::Button::new();
    btn.add_css_class("dn-outline-btn");
    pointer(&btn);
    let row = gtk::Box::new(gtk::Orientation::Horizontal, 6);
    row.set_halign(gtk::Align::Center);
    let text = gtk::Label::new(Some(label));
    if let Some(name) = icon {
        if icon_after {
            row.append(&text);
            row.append(&named_icon(name, 16));
        } else {
            row.append(&named_icon(name, 16));
            row.append(&text);
        }
    } else {
        row.append(&text);
    }
    btn.set_child(Some(&row));
    btn
}

fn action_button(
    icon: Option<&str>,
    label: &str,
    suggested: bool,
    on_click: impl Fn() + 'static,
) -> gtk::Button {
    let classes: &[&str] = if suggested {
        &["suggested-action"]
    } else {
        &[]
    };
    let button = icon_button(icon, false, label, 16, classes);
    button.connect_clicked(move |_| on_click());
    button
}

fn icon_only_button(icon: &str, tooltip: &str, suggested: bool) -> gtk::Button {
    let btn = gtk::Button::new();
    btn.add_css_class("flat");
    btn.add_css_class("dn-icon-btn");
    if suggested {
        btn.add_css_class("suggested-action");
    }
    btn.set_tooltip_text(Some(tooltip));
    btn.set_icon_name(icon);
    pointer(&btn);
    btn
}

fn profile_tab(label: &str, active: bool, tab: ProfileFeed, sender: Sender<AppMsg>) -> gtk::Button {
    let button = gtk::Button::with_label(label);
    pointer(&button);
    button.set_hexpand(true);
    button.set_halign(gtk::Align::Fill);
    button.set_css_classes(if active {
        &["flat", "dn-tab", "active"]
    } else {
        &["flat", "dn-tab", "inactive"]
    });
    button.connect_clicked(move |_| sender.emit(AppMsg::ProfileTab(tab)));
    button
}

fn shrink_box(spacing: i32) -> gtk::Box {
    let row = gtk::Box::new(gtk::Orientation::Horizontal, spacing);
    row.set_hexpand(true);
    row.add_css_class("dn-shrink");
    row
}

fn shrink_label(label: &gtk::Label) {
    label.set_wrap(true);
    label.set_wrap_mode(pango::WrapMode::WordChar);
    label.set_hexpand(true);
    label.set_xalign(0.0);
    label.set_width_chars(1);
}

fn ellipsize_label(label: &gtk::Label) {
    label.set_ellipsize(pango::EllipsizeMode::End);
    label.set_hexpand(true);
    label.set_xalign(0.0);
    label.set_width_chars(1);
}

fn profile_meta(user: &User, sender: Sender<AppMsg>) -> gtk::Box {
    let meta = shrink_box(16);
    meta.add_css_class("dn-profile-meta");
    if let Some(company) = user.company.as_deref().filter(|s| !s.is_empty()) {
        meta.append(&meta_item("dn-building-2-symbolic", company));
    }
    if let Some(location) = user.location.as_deref().filter(|s| !s.is_empty()) {
        meta.append(&meta_item("dn-map-pin-symbolic", location));
    }
    if let Some(website) = user.website.as_deref().filter(|s| !s.is_empty()) {
        meta.append(&meta_link(
            "dn-link-2-symbolic",
            strip_url_scheme(website),
            absolute_web_url(website),
            sender.clone(),
        ));
    }
    if let Some((href, label)) = github_profile_link(user) {
        meta.append(&meta_link(
            "dn-github-symbolic",
            label,
            href,
            sender.clone(),
        ));
    }
    if user.has_sponsors_listing.unwrap_or(false) {
        meta.append(&meta_link(
            "dn-heart-symbolic",
            "Sponsor on GitHub",
            format!("https://github.com/sponsors/{}", user.username),
            sender,
        ));
    }
    if let Some(created) = user.created_at {
        meta.append(&meta_item(
            "dn-calendar-days-symbolic",
            &format!("Joined {}", created.format("%B %Y")),
        ));
    }
    meta
}

fn github_profile_link(user: &User) -> Option<(String, String)> {
    if let Some(url) = user.github_url.as_deref().filter(|s| !s.is_empty()) {
        return Some((absolute_web_url(url), strip_url_scheme(url).to_owned()));
    }
    if user.has_github_account == Some(false) {
        return None;
    }
    Some((
        format!("https://github.com/{}", user.username),
        format!("github.com/{}", user.username),
    ))
}

fn strip_url_scheme(url: &str) -> &str {
    url.trim_start_matches("https://")
        .trim_start_matches("http://")
}

fn absolute_web_url(url: &str) -> String {
    let url = url.trim();
    if url.starts_with("https://") || url.starts_with("http://") {
        url.to_owned()
    } else {
        format!("https://{url}")
    }
}

fn meta_item(icon: &str, text: &str) -> gtk::Box {
    let row = gtk::Box::new(gtk::Orientation::Horizontal, 6);
    row.set_hexpand(true);
    let image = named_icon(icon, 16);
    image.add_css_class("dn-muted");
    row.append(&image);
    let label = muted_label(text);
    row.append(&label);
    row
}

fn meta_link(
    icon: &str,
    text: impl AsRef<str>,
    url: String,
    sender: Sender<AppMsg>,
) -> gtk::Button {
    let btn = gtk::Button::new();
    btn.add_css_class("flat");
    btn.add_css_class("dn-profile-link");
    pointer(&btn);
    btn.set_tooltip_text(Some(&url));
    btn.set_halign(gtk::Align::Start);
    btn.set_valign(gtk::Align::Center);
    btn.set_hexpand(true);
    let row = gtk::Box::new(gtk::Orientation::Horizontal, 6);
    let image = named_icon(icon, 16);
    row.append(&image);
    let label = gtk::Label::new(Some(text.as_ref()));
    label.add_css_class("dn-mono");
    label.set_ellipsize(pango::EllipsizeMode::End);
    label.set_width_chars(1);
    label.set_hexpand(true);
    label.set_xalign(0.0);
    row.append(&label);
    btn.set_child(Some(&row));
    btn.connect_clicked(move |_| sender.emit(AppMsg::OpenWeb(url.clone())));
    btn
}

fn profile_counts(user: &User, sender: Sender<AppMsg>) -> gtk::Box {
    let row = shrink_box(16);
    row.add_css_class("dn-profile-counts");
    let base = devnull_api::default_base_url();
    let username = user.username.clone();
    let commits = user.commits_count.unwrap_or(0);
    let following = user.following_count.unwrap_or(0);
    let followers = user.followers_count.unwrap_or(0);
    row.append(&profile_stat(
        commits,
        if commits == 1 { "commit" } else { "commits" },
        None,
        sender.clone(),
    ));
    row.append(&profile_stat(
        following,
        "following",
        Some(format!("{base}/@{username}/following")),
        sender.clone(),
    ));
    row.append(&profile_stat(
        followers,
        if followers == 1 {
            "follower"
        } else {
            "followers"
        },
        Some(format!("{base}/@{username}/followers")),
        sender,
    ));
    row
}

fn profile_stat(
    count: i64,
    label: &str,
    url: Option<String>,
    sender: Sender<AppMsg>,
) -> gtk::Widget {
    let n = gtk::Label::new(Some(&count.to_string()));
    n.add_css_class("dn-stat-count");
    n.add_css_class("dn-mono");
    let word = gtk::Label::new(Some(label));
    word.add_css_class("dn-stat-label");
    word.add_css_class("dn-muted");
    let inner = gtk::Box::new(gtk::Orientation::Horizontal, 6);
    inner.append(&n);
    inner.append(&word);
    let Some(url) = url else {
        return inner.upcast();
    };
    let btn = gtk::Button::new();
    btn.add_css_class("flat");
    btn.add_css_class("dn-profile-stat");
    pointer(&btn);
    btn.set_child(Some(&inner));
    btn.connect_clicked(move |_| sender.emit(AppMsg::OpenWeb(url.clone())));
    btn.upcast()
}

fn muted_label(text: &str) -> gtk::Label {
    let label = gtk::Label::new(Some(text));
    label.add_css_class("dn-muted");
    label.add_css_class("dn-mono");
    ellipsize_label(&label);
    label
}

pub fn lane_picker(
    me: Option<&CurrentUser>,
    draft: Option<Option<Lane>>,
    save_button: bool,
    sender: Sender<AppMsg>,
) -> gtk::Box {
    let dark = adw_is_dark();
    let automatic = automatic_lane(me);
    let saved = me.and_then(|user| user.preferred_lane.clone());
    let selected = draft.clone().unwrap_or_else(|| saved.clone());
    let preview_lane = selected.clone().unwrap_or_else(|| automatic.clone());
    let preview_hex = preview_lane.css_color(dark);

    let body = gtk::Box::new(gtk::Orientation::Vertical, 20);
    body.append(&lane_preview(me, &preview_hex));

    let options = gtk::Box::new(gtk::Orientation::Horizontal, 4);
    options.add_css_class("dn-lane-picker");
    options.set_hexpand(true);
    let auto_hex = automatic.css_color(dark);
    options.append(&lane_option(
        "Auto",
        Some(&format!(
            "Automatic ({})",
            lane_name(match automatic {
                Lane::Palette(i) => i,
                Lane::Custom(_) => 0,
            })
            .to_lowercase()
        )),
        &auto_hex,
        true,
        selected.is_none(),
        {
            let sender = sender.clone();
            move || {
                if save_button {
                    sender.emit(AppMsg::DraftLane(None));
                } else {
                    sender.emit(AppMsg::SetLane(None));
                }
            }
        },
    ));
    let names = ["Orange", "Green", "Violet", "Teal", "Rose", "Ochre"];
    for (index, name) in names.iter().enumerate() {
        let palette = index as u8;
        let hex = Lane::Palette(palette).css_color(dark);
        let active = matches!(selected, Some(Lane::Palette(i)) if i == palette);
        options.append(&lane_option(name, None, &hex, false, active, {
            let sender = sender.clone();
            move || {
                if save_button {
                    sender.emit(AppMsg::DraftLane(Some(Lane::Palette(palette))));
                } else {
                    sender.emit(AppMsg::SetLane(Some(Lane::Palette(palette))));
                }
            }
        }));
    }
    let custom_hex = match &selected {
        Some(Lane::Custom(hex)) => Some(hex.clone()),
        _ => None,
    };
    options.append(&custom_lane_option(
        custom_hex.as_deref(),
        matches!(selected, Some(Lane::Custom(_))),
        save_button,
        sender.clone(),
    ));
    body.append(&options);

    if save_button {
        let save = gtk::Button::with_label("Save color");
        save.add_css_class("suggested-action");
        save.set_halign(gtk::Align::Start);
        pointer(&save);
        save.set_sensitive(draft.is_some() && draft.as_ref() != Some(&saved));
        let to_save = selected.clone();
        save.connect_clicked(move |_| sender.emit(AppMsg::SetLane(to_save.clone())));
        body.append(&save);
    }
    body
}

fn automatic_lane(me: Option<&CurrentUser>) -> Lane {
    me.and_then(|user| user.automatic_lane)
        .map(Lane::Palette)
        .or_else(|| me.and_then(|user| user.user.lane.clone()))
        .unwrap_or(Lane::Palette(0))
}

fn lane_name(index: u8) -> &'static str {
    ["Orange", "Green", "Violet", "Teal", "Rose", "Ochre"][(index as usize) % 6]
}

fn lane_preview(me: Option<&CurrentUser>, hex: &str) -> gtk::Box {
    let preview = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    preview.add_css_class("dn-lane-preview");
    preview.set_overflow(gtk::Overflow::Hidden);
    let overlay = gtk::Overlay::new();
    overlay.set_size_request(RAIL_WIDTH, 64);
    overlay.set_hexpand(false);
    overlay.set_child(Some(&rail(
        hex,
        Some(hex),
        RailKind::Commit {
            incoming: true,
            terminator: false,
            dashed_in: false,
        },
    )));
    let avatar = avatar_widget(me.map(|user| &user.user), 40, false);
    avatar.set_halign(gtk::Align::Center);
    avatar.set_valign(gtk::Align::Start);
    avatar.set_margin_top(staged_avatar_top());
    overlay.add_overlay(&avatar);
    preview.append(&overlay);
    let copy = gtk::Box::new(gtk::Orientation::Vertical, 2);
    copy.set_valign(gtk::Align::Center);
    copy.set_hexpand(true);
    copy.set_margin_end(16);
    let name = gtk::Label::new(Some(
        me.map(|user| user.user.display_name()).unwrap_or("You"),
    ));
    name.add_css_class("dn-name");
    name.set_xalign(0.0);
    name.set_ellipsize(pango::EllipsizeMode::End);
    copy.append(&name);
    let handle = gtk::Box::new(gtk::Orientation::Horizontal, 6);
    handle.append(&color_dot(hex));
    let at = gtk::Label::new(Some(&format!(
        "@{}",
        me.map(|user| user.username()).unwrap_or("you")
    )));
    at.add_css_class("dn-handle");
    at.set_xalign(0.0);
    at.set_ellipsize(pango::EllipsizeMode::End);
    handle.append(&at);
    copy.append(&handle);
    preview.append(&copy);
    preview
}

fn color_dot(hex: &str) -> gtk::DrawingArea {
    let (r, g, b) = rgb(hex);
    let area = gtk::DrawingArea::new();
    area.set_content_width(8);
    area.set_content_height(8);
    area.set_size_request(8, 8);
    area.set_valign(gtk::Align::Center);
    area.set_draw_func(move |_, cairo, width, height| {
        cairo.set_source_rgb(r, g, b);
        cairo.arc(
            f64::from(width) / 2.0,
            f64::from(height) / 2.0,
            4.0,
            0.0,
            std::f64::consts::TAU,
        );
        cairo.fill().ok();
    });
    area
}

fn lane_option(
    label: &str,
    aria: Option<&str>,
    hex: &str,
    automatic: bool,
    active: bool,
    on_pick: impl Fn() + 'static,
) -> gtk::Button {
    let btn = gtk::Button::new();
    pointer(&btn);
    btn.add_css_class("flat");
    btn.add_css_class("dn-lane-option");
    if active {
        btn.add_css_class("active");
    }
    btn.set_tooltip_text(aria.or(Some(label)));
    let col = gtk::Box::new(gtk::Orientation::Vertical, 6);
    col.set_halign(gtk::Align::Center);
    col.append(&lane_swatch(hex, automatic, active));
    let text = gtk::Label::new(Some(label));
    text.add_css_class("dn-lane-option-label");
    if active {
        text.add_css_class("active");
    }
    col.append(&text);
    btn.set_child(Some(&col));
    btn.connect_clicked(move |_| on_pick());
    btn
}

fn lane_swatch(hex: &str, automatic: bool, active: bool) -> gtk::DrawingArea {
    let (r, g, b) = rgb(hex);
    let area = gtk::DrawingArea::new();
    area.set_content_width(38);
    area.set_content_height(38);
    area.set_size_request(38, 38);
    area.set_draw_func(move |_, cairo, width, height| {
        let cx = f64::from(width) / 2.0;
        let cy = f64::from(height) / 2.0;
        cairo.set_source_rgb(r, g, b);
        if automatic {
            cairo.set_line_width(2.0);
            cairo.set_dash(&[3.0, 3.0], 0.0);
            cairo.arc(cx, cy, 13.0, 0.0, std::f64::consts::TAU);
            cairo.stroke().ok();
            cairo.set_dash(&[], 0.0);
            cairo.arc(cx, cy, 5.0, 0.0, std::f64::consts::TAU);
            cairo.fill().ok();
        } else {
            cairo.arc(cx, cy, 14.0, 0.0, std::f64::consts::TAU);
            cairo.fill().ok();
        }
        if active {
            cairo.set_line_width(2.0);
            cairo.arc(cx, cy, 17.0, 0.0, std::f64::consts::TAU);
            cairo.stroke().ok();
        }
    });
    area
}

fn custom_lane_option(
    hex: Option<&str>,
    active: bool,
    save_button: bool,
    sender: Sender<AppMsg>,
) -> gtk::Button {
    let btn = gtk::Button::new();
    pointer(&btn);
    btn.add_css_class("flat");
    btn.add_css_class("dn-lane-option");
    if active {
        btn.add_css_class("active");
    }
    let tooltip = match hex {
        Some(color) => format!("Custom color ({color})"),
        None => "Custom color".into(),
    };
    btn.set_tooltip_text(Some(&tooltip));
    let col = gtk::Box::new(gtk::Orientation::Vertical, 6);
    col.set_halign(gtk::Align::Center);
    let swatch = gtk::DrawingArea::new();
    swatch.set_content_width(38);
    swatch.set_content_height(38);
    swatch.set_size_request(38, 38);
    let custom = hex.map(str::to_owned);
    swatch.set_draw_func(move |_, cairo, width, height| {
        let cx = f64::from(width) / 2.0;
        let cy = f64::from(height) / 2.0;
        if let Some(color) = &custom {
            let (r, g, b) = rgb(color);
            cairo.set_source_rgb(r, g, b);
            cairo.arc(cx, cy, 14.0, 0.0, std::f64::consts::TAU);
            cairo.fill().ok();
            if active {
                cairo.set_line_width(2.0);
                cairo.arc(cx, cy, 17.0, 0.0, std::f64::consts::TAU);
                cairo.stroke().ok();
            }
        } else {
            draw_rainbow_swatch(cairo, cx, cy, 14.0);
        }
    });
    col.append(&swatch);
    let text = gtk::Label::new(Some("Custom"));
    text.add_css_class("dn-lane-option-label");
    if active {
        text.add_css_class("active");
    }
    col.append(&text);
    btn.set_child(Some(&col));
    let initial = hex.map(str::to_owned);
    btn.connect_clicked(move |_| {
        let dialog = gtk::ColorDialog::new();
        dialog.set_with_alpha(false);
        let current = initial
            .as_deref()
            .and_then(|value| gtk::gdk::RGBA::parse(value).ok())
            .unwrap_or(gtk::gdk::RGBA::BLACK);
        let sender = sender.clone();
        dialog.choose_rgba(
            toplevel().as_ref(),
            Some(&current),
            None::<&gtk::gio::Cancellable>,
            move |result| {
                if let Ok(rgba) = result {
                    let lane = Some(Lane::Custom(rgba_to_hex(rgba)));
                    if save_button {
                        sender.emit(AppMsg::DraftLane(lane));
                    } else {
                        sender.emit(AppMsg::SetLane(lane));
                    }
                }
            },
        );
    });
    btn
}

fn draw_rainbow_swatch(cairo: &gtk::cairo::Context, cx: f64, cy: f64, radius: f64) {
    let stops = [
        (1.00, 0.24, 0.24),
        (0.99, 0.49, 0.08),
        (0.99, 0.77, 0.10),
        (0.32, 0.81, 0.40),
        (0.13, 0.72, 0.81),
        (0.30, 0.43, 0.96),
        (0.68, 0.24, 0.79),
        (1.00, 0.24, 0.24),
    ];
    let slice = std::f64::consts::TAU / (stops.len() - 1) as f64;
    for (index, window) in stops.windows(2).enumerate() {
        let start = f64::from(index as u32) * slice - std::f64::consts::FRAC_PI_2;
        cairo.set_source_rgb(window[0].0, window[0].1, window[0].2);
        cairo.move_to(cx, cy);
        cairo.arc(cx, cy, radius, start, start + slice);
        cairo.close_path();
        cairo.fill().ok();
    }
}

fn rgba_to_hex(rgba: gtk::gdk::RGBA) -> String {
    format!(
        "#{:02x}{:02x}{:02x}",
        (rgba.red() * 255.0).round() as u8,
        (rgba.green() * 255.0).round() as u8,
        (rgba.blue() * 255.0).round() as u8,
    )
}

pub struct SettingsCtx<'a> {
    pub me: Option<&'a CurrentUser>,
    pub tab: SettingsTab,
    pub theme: Theme,
    pub show_who_to_follow: bool,
    pub muted: &'a [User],
    pub blocked: &'a [User],
    pub draft_lane: Option<Option<Lane>>,
    pub device_name: String,
    pub fingerprint: Option<String>,
    pub sender: Sender<AppMsg>,
}

pub fn settings_panel(ctx: SettingsCtx<'_>) -> gtk::Box {
    let panel = gtk::Box::new(gtk::Orientation::Vertical, 0);
    panel.add_css_class("dn-settings");
    match ctx.tab {
        SettingsTab::Profile => fill_profile_settings(&panel, ctx),
        SettingsTab::Appearance => fill_appearance_settings(&panel, ctx),
        SettingsTab::Terminal => fill_terminal_settings(&panel, ctx.sender),
        SettingsTab::Safety => fill_safety_settings(&panel, ctx),
        SettingsTab::Devices => fill_devices_settings(&panel, ctx),
    }
    panel
}

pub fn save_export(bytes: Vec<u8>, sender: Sender<AppMsg>) {
    let dialog = gtk::FileDialog::new();
    dialog.set_initial_name(Some(&format!(
        "devnull-export-{}.json",
        Local::now().format("%Y-%m-%d-%H%M%S")
    )));
    let filter = gtk::FileFilter::new();
    filter.set_name(Some("JSON"));
    filter.add_mime_type("application/json");
    let filters = gtk::gio::ListStore::new::<gtk::FileFilter>();
    filters.append(&filter);
    dialog.set_filters(Some(&filters));
    dialog.save(
        toplevel().as_ref(),
        None::<&gtk::gio::Cancellable>,
        move |result| {
            let Ok(file) = result else {
                return;
            };
            match file.replace_contents(
                &bytes,
                None,
                false,
                gtk::gio::FileCreateFlags::REPLACE_DESTINATION,
                None::<&gtk::gio::Cancellable>,
            ) {
                Ok(_) => sender.emit(AppMsg::Toast("Export saved.".into())),
                Err(_) => sender.emit(AppMsg::Toast("fatal: could not write the export".into())),
            }
        },
    );
}

pub fn confirm_delete_account(sender: Sender<AppMsg>) {
    let dialog = adw::AlertDialog::new(
        Some("Delete your account?"),
        Some(
            "Your profile, commits, replies, and follows go to /dev/null, along with every reply others left under your commits. Replies under your replies stay, under a [deleted] tombstone. This can't be undone. You can sign in with GitHub again later, but you'll start from zero.",
        ),
    );
    dialog.add_response("cancel", "Keep my account");
    dialog.add_response("delete", "Delete account");
    dialog.set_response_appearance("delete", adw::ResponseAppearance::Destructive);
    dialog.set_default_response(Some("cancel"));
    dialog.set_close_response("cancel");
    dialog.connect_response(None, move |_, response| {
        if response == "delete" {
            sender.emit(AppMsg::ConfirmDeleteAccount);
        }
    });
    dialog.present(toplevel().as_ref());
}

fn fill_profile_settings(panel: &gtk::Box, ctx: SettingsCtx<'_>) {
    let me = ctx.me;
    let sender = ctx.sender;
    let profile = gtk::Box::new(gtk::Orientation::Vertical, 20);
    profile.append(&github_profile(me));
    let actions = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    actions.add_css_class("dn-settings-actions");
    actions.set_hexpand(true);
    actions.append(&action_button(
        Some("dn-arrow-up-right-symbolic"),
        "Edit on GitHub",
        false,
        {
            let sender = sender.clone();
            move || sender.emit(AppMsg::EditOnGitHub)
        },
    ));
    actions.append(&action_button(
        Some("dn-refresh-cw-symbolic"),
        "Sync from GitHub",
        false,
        {
            let sender = sender.clone();
            move || sender.emit(AppMsg::SyncProfile)
        },
    ));
    actions.append(&action_button(None, "View profile", false, {
        let sender = sender.clone();
        move || sender.emit(AppMsg::OpenMe)
    }));
    profile.append(&actions);
    let synced = gtk::Label::new(Some(&synced_label(me)));
    synced.add_css_class("dn-muted");
    synced.set_xalign(0.0);
    synced.set_wrap(true);
    profile.append(&synced);
    panel.append(&settings_section(
        "Profile information",
        "Mirrored from GitHub when you sign in. To change any of it, change it on GitHub, then sync it here.",
        &profile,
    ));

    panel.append(&settings_section(
        "Color",
        "Your lane on the graph: the rail and nodes beside your commits, the ring around your avatar, and the dot before your handle. Automatic picks one from your account id.",
        &lane_picker(me, ctx.draft_lane.clone(), true, sender.clone()),
    ));
    panel.append(&timezone_picker(me, sender.clone()));
    panel.append(&toggle_setting(
        "Pastéis mode",
        "A little appreciation, fresh daily. Get 10 pastéis to give to other people’s commits, one per click. Your allowance resets at 00:00 UTC.\n\nWhen off, all pastéis controls and counters are hidden. You still receive pastéis from others.",
        me.is_some_and(CurrentUser::pasteis_on),
        {
            let sender = sender.clone();
            move |on| sender.emit(AppMsg::SetPasteis(on))
        },
    ));
    panel.append(&toggle_setting(
        "Remotes",
        "One-to-one messages with other developers. People you follow can push to you, and you can push to people who follow you. Whether you share pulls (read receipts) is up to you, from Remotes.\n\nWhen off, nobody can message you and Remotes is hidden. Your conversations are kept for when you turn it back on.",
        me.is_some_and(CurrentUser::remotes_on),
        {
            let sender = sender.clone();
            move |on| sender.emit(AppMsg::SetRemotes(on))
        },
    ));
    if let Some(user) = me {
        panel.append(&activity_graph_embed(user, sender.clone()));
    }
    let export = gtk::Box::new(gtk::Orientation::Vertical, 12);
    export.append(&action_button(
        Some("dn-download-symbolic"),
        "Download JSON",
        false,
        {
            let sender = sender.clone();
            move || sender.emit(AppMsg::ExportData)
        },
    ));
    panel.append(&settings_section(
        "Export your data",
        "Download everything we store about you: your profile, commits and replies (including pending, rejected, and deleted ones), stars, saved commits, follows, mutes, blocks, the messages you sent, notifications, SSH key fingerprints, and active sessions. Other people's replies are excluded. Your export is a JSON file. Your profile is still managed on GitHub.",
        &export,
    ));
    let delete = gtk::Box::new(gtk::Orientation::Vertical, 12);
    let danger = action_button(None, "Delete account", false, move || {
        sender.emit(AppMsg::DeleteAccount)
    });
    danger.add_css_class("dn-danger");
    delete.append(&danger);
    panel.append(&settings_section(
        "Delete account",
        "Removes your /dev/null account, every commit and reply you've made, and everyone you follow. Replies others left under your commits go too. Replies under your replies stay, hanging off a [deleted] tombstone. Your GitHub account is not touched.",
        &delete,
    ));
}

fn fill_appearance_settings(panel: &gtk::Box, ctx: SettingsCtx<'_>) {
    let sender = ctx.sender;
    let theme = ctx.theme;
    let show = ctx.show_who_to_follow;
    panel.append(&settings_section(
        "Theme",
        "System follows your operating system, light by day and dark by night if that's how you have it set.",
        &segmented(
            &[
                ("Light", "dn-sun-symbolic", Theme::Light as u8),
                ("Dark", "dn-moon-symbolic", Theme::Dark as u8),
                ("System", "dn-monitor-symbolic", Theme::System as u8),
            ],
            theme as u8,
            {
                let sender = sender.clone();
                move |value| {
                    let theme = match value {
                        1 => Theme::Dark,
                        2 => Theme::System,
                        _ => Theme::Light,
                    };
                    sender.emit(AppMsg::SetTheme(theme));
                }
            },
        ),
    ));
    panel.append(&settings_section(
        "Who to follow",
        "Three people you might want to follow, in the right column, or in your log on smaller screens.",
        &segmented(
            &[
                ("Show", "dn-eye-symbolic", 1),
                ("Hide", "dn-eye-off-symbolic", 0),
            ],
            u8::from(show),
            move |value| sender.emit(AppMsg::SetWhoToFollow(value == 1)),
        ),
    ));
}

fn fill_terminal_settings(panel: &gtk::Box, sender: Sender<AppMsg>) {
    let base = devnull_api::default_base_url();
    let commands = gtk::Box::new(gtk::Orientation::Vertical, 8);
    commands.append(&copyable_code(
        "command",
        &format!("curl -fsSL {base}/commit | bash -s -- 'It was DNS.'"),
        sender.clone(),
    ));
    commands.append(&copyable_code(
        "reply command",
        &format!("curl -fsSL {base}/commit | bash -s -- --reply 0be0130 'Agreed!'"),
        sender.clone(),
    ));
    commands.append(&copyable_code(
        "image command",
        &format!("curl -fsSL {base}/commit | bash -s -- --image shipped.png 'Shipped!'"),
        sender.clone(),
    ));
    panel.append(&settings_section(
        "Commit from your terminal",
        "No login, no token: the script signs your commit with an SSH key that is on your GitHub account, and /dev/null checks it against the keys GitHub publishes for you.",
        &commands,
    ));

    let steps = gtk::Box::new(gtk::Orientation::Vertical, 16);
    steps.append(&terminal_step(
        1,
        "Generate a key for CI only.",
        Some((
            "key command",
            "ssh-keygen -t ed25519 -N '' -C devnull-ci -f devnull-ci",
        )),
        sender.clone(),
    ));
    let step2 = gtk::Label::new(None);
    step2.set_markup(
        "2. Add <span font_family='monospace' size='small'>devnull-ci.pub</span> to GitHub as a <a href=\"https://github.com/settings/ssh/new\">signing key</a>. It can sign /dev/null commits, but never push code or log in.",
    );
    step2.set_wrap(true);
    step2.set_xalign(0.0);
    step2.set_use_markup(true);
    steps.append(&step2);
    let step3 = gtk::Label::new(None);
    step3.set_markup(
        "3. Save <span font_family='monospace' size='small'>devnull-ci</span>, the private key, as a repository secret named <span font_family='monospace' size='small'>DEVNULL_SSH_KEY</span>, then delete both files.",
    );
    step3.set_wrap(true);
    step3.set_xalign(0.0);
    steps.append(&step3);
    steps.append(&terminal_step(
        4,
        "Add the workflow, in .github/workflows/devnull.yml.",
        Some(("workflow", TERMINAL_WORKFLOW)),
        sender,
    ));
    let inputs = [
        ("body", "The commit, 3 to 4,000 characters."),
        (
            "ssh-key",
            "The private key, from a secret. Ed25519 or RSA, without a passphrase.",
        ),
        (
            "reply-to",
            "Optional. The hash or link of a commit to reply to.",
        ),
        (
            "images",
            "Optional. Paths of JPEG, PNG or WebP files to attach, one per line, up to 3.",
        ),
    ];
    steps.append(&field_list(
        &inputs
            .iter()
            .map(|(k, v)| (*k, Some((*v).to_string())))
            .collect::<Vec<_>>(),
    ));
    let note = gtk::Label::new(Some(
        "The job prints the link to your commit, and whether it is pending review. When /dev/null refuses it, the job fails with the reason.",
    ));
    note.add_css_class("dn-muted");
    note.set_wrap(true);
    note.set_xalign(0.0);
    steps.append(&note);
    panel.append(&settings_section(
        "Commit from GitHub Actions",
        "Ship release notes when you publish a release, or anything else a workflow knows. Same signature, same moderation, same 10 commits a minute.",
        &steps,
    ));
}

fn fill_devices_settings(panel: &gtk::Box, ctx: SettingsCtx<'_>) {
    let sender = ctx.sender;
    let fields = gtk::Box::new(gtk::Orientation::Vertical, 0);
    fields.append(&field_list(&[
        ("name", Some(ctx.device_name.clone())),
        ("fingerprint", ctx.fingerprint.clone()),
        ("remote", Some("Linux".into())),
    ]));
    let actions = gtk::Box::new(gtk::Orientation::Vertical, 12);
    actions.append(&fields);
    actions.append(&action_button(
        Some("dn-monitor-symbolic"),
        "Your devices on dev-null.sh",
        false,
        {
            let sender = sender.clone();
            move || {
                sender.emit(AppMsg::OpenWeb(format!(
                    "{}/settings/devices",
                    devnull_api::default_base_url()
                )));
            }
        },
    ));
    panel.append(&settings_section(
        "This computer",
        "Signed in with a key that stays on this computer. dev-null.sh lists it by this fingerprint.",
        &actions,
    ));
    let sign_out = gtk::Box::new(gtk::Orientation::Vertical, 8);
    sign_out.append(&action_button(None, "Sign out", false, {
        let sender = sender.clone();
        move || sender.emit(AppMsg::SignOut)
    }));
    let hint = gtk::Label::new(Some(
        "Deletes the key from this computer and signs you out. The computer stays listed on dev-null.sh until you remove it there, and signing in again makes a new key.",
    ));
    hint.add_css_class("dn-muted");
    hint.set_wrap(true);
    hint.set_xalign(0.0);
    sign_out.append(&hint);
    panel.append(&settings_section(
        "Sign out",
        "This only affects this computer.",
        &sign_out,
    ));
}

fn fill_safety_settings(panel: &gtk::Box, ctx: SettingsCtx<'_>) {
    panel.append(&settings_section(
        "Muted",
        "Their commits and replies stay out of your logs, threads, search, and suggestions, and they can't notify you. They aren't told, and you can still visit their profile.",
        &people_list(
            ctx.muted,
            "You haven't muted anyone.",
            "Unmute",
            {
                let sender = ctx.sender.clone();
                move |username| sender.emit(AppMsg::Unmute(username))
            },
            ctx.sender.clone(),
        ),
    ));
    panel.append(&settings_section(
        "Blocked",
        "Neither of you can message, follow, reply to, or star the other, or see each other's commits. Unblocking doesn't bring back follows.",
        &people_list(
            ctx.blocked,
            "You haven't blocked anyone.",
            "Unblock",
            {
                let sender = ctx.sender.clone();
                move |username| sender.emit(AppMsg::Unblock(username))
            },
            ctx.sender,
        ),
    ));
}

const TERMINAL_WORKFLOW: &str = "name: /dev/null\n\non:\n  release:\n    types: [published]\n\npermissions: {}\n\njobs:\n  commit:\n    runs-on: ubuntu-latest\n    steps:\n      - uses: nunomaduro/dev-null.sh/.github/actions/commit@production\n        with:\n          body: |\n            Released ${{ github.event.release.name }}.\n            ${{ github.event.release.html_url }}\n          ssh-key: ${{ secrets.DEVNULL_SSH_KEY }}";

fn settings_section(title: &str, description: &str, child: &impl IsA<gtk::Widget>) -> gtk::Box {
    let section = gtk::Box::new(gtk::Orientation::Vertical, 0);
    section.add_css_class("dn-section");
    let heading = gtk::Label::new(Some(title));
    heading.add_css_class("dn-section-title");
    heading.set_xalign(0.0);
    section.append(&heading);
    let blurb = gtk::Label::new(Some(description));
    blurb.add_css_class("dn-muted");
    blurb.add_css_class("dn-section-blurb");
    blurb.set_wrap(true);
    blurb.set_max_width_chars(65);
    blurb.set_xalign(0.0);
    section.append(&blurb);
    let body = gtk::Box::new(gtk::Orientation::Vertical, 0);
    body.add_css_class("dn-section-body");
    body.append(child);
    section.append(&body);
    section
}

fn github_profile(me: Option<&CurrentUser>) -> gtk::Box {
    let wrap = gtk::Box::new(gtk::Orientation::Vertical, 20);
    let Some(user) = me.map(|current| &current.user) else {
        wrap.append(&muted_label("Sign in to see your profile."));
        return wrap;
    };
    let hero = gtk::Box::new(gtk::Orientation::Horizontal, 16);
    hero.append(&avatar_widget(Some(user), 56, false));
    let names = gtk::Box::new(gtk::Orientation::Vertical, 2);
    let name = gtk::Label::new(Some(user.display_name()));
    name.add_css_class("dn-name");
    name.set_xalign(0.0);
    names.append(&name);
    let handle = gtk::Label::new(Some(&format!("@{}", user.username)));
    handle.add_css_class("dn-handle");
    handle.set_xalign(0.0);
    names.append(&handle);
    hero.append(&names);
    wrap.append(&hero);
    let sponsors = user
        .has_sponsors_listing
        .unwrap_or(false)
        .then(|| format!("github.com/sponsors/{}", user.username));
    wrap.append(&field_list(&[
        ("name", user.name.clone()),
        ("username", Some(format!("@{}", user.username))),
        ("bio", user.bio.clone()),
        ("company", user.company.clone()),
        ("location", user.location.clone()),
        ("website", user.website.clone()),
        ("sponsors", sponsors),
    ]));
    wrap
}

fn field_list(fields: &[(&str, Option<String>)]) -> gtk::Box {
    let list = gtk::Box::new(gtk::Orientation::Vertical, 0);
    list.add_css_class("dn-fields");
    for (index, (key, value)) in fields.iter().enumerate() {
        let row = gtk::Box::new(gtk::Orientation::Horizontal, 12);
        row.add_css_class("dn-field-row");
        if index + 1 == fields.len() {
            row.add_css_class("last");
        }
        let dt = gtk::Label::new(Some(key));
        dt.add_css_class("dn-field-key");
        dt.set_xalign(0.0);
        row.append(&dt);
        let dd = gtk::Label::new(Some(value.as_deref().unwrap_or("null")));
        dd.set_xalign(0.0);
        dd.set_wrap(true);
        dd.set_hexpand(true);
        if value.is_none() {
            dd.add_css_class("dn-mono");
            dd.add_css_class("dn-muted");
        }
        row.append(&dd);
        list.append(&row);
    }
    list
}

fn timezone_picker(me: Option<&CurrentUser>, sender: Sender<AppMsg>) -> gtk::Box {
    let current = me.and_then(CurrentUser::timezone).map(str::to_owned);
    let zones = iana_timezones();
    let mut labels = vec!["Not set, using this computer's timezone".to_owned()];
    labels.extend(zones.iter().cloned());
    let refs: Vec<&str> = labels.iter().map(String::as_str).collect();
    let dropdown = gtk::DropDown::from_strings(&refs);
    dropdown.set_enable_search(true);
    let selected_index = current
        .as_ref()
        .and_then(|name| zones.iter().position(|zone| zone == name))
        .map(|index| index + 1)
        .unwrap_or(0);
    dropdown.set_selected(selected_index as u32);
    let picked = Rc::new(RefCell::new(current.clone()));
    dropdown.connect_selected_notify({
        let picked = picked.clone();
        let zones = zones.clone();
        move |drop| {
            let index = drop.selected() as usize;
            *picked.borrow_mut() = if index == 0 {
                None
            } else {
                zones.get(index - 1).cloned()
            };
        }
    });
    let body = gtk::Box::new(gtk::Orientation::Vertical, 8);
    body.append(&dropdown);
    let links = gtk::Box::new(gtk::Orientation::Horizontal, 12);
    if let Some(local) = local_timezone() {
        if current.as_deref() != Some(local.as_str()) {
            let use_local = gtk::Button::with_label(&format!("Use {local} from this computer"));
            use_local.add_css_class("flat");
            use_local.add_css_class("dn-text-btn");
            pointer(&use_local);
            let drop = dropdown.clone();
            let zones = zones.clone();
            let picked = picked.clone();
            use_local.connect_clicked(move |_| {
                *picked.borrow_mut() = Some(local.clone());
                if let Some(index) = zones.iter().position(|zone| zone == &local) {
                    drop.set_selected((index + 1) as u32);
                }
            });
            links.append(&use_local);
        }
    }
    let unset = gtk::Button::with_label("Unset");
    unset.add_css_class("flat");
    unset.add_css_class("dn-text-btn");
    pointer(&unset);
    let drop = dropdown.clone();
    let picked_unset = picked.clone();
    unset.connect_clicked(move |_| {
        *picked_unset.borrow_mut() = None;
        drop.set_selected(0);
    });
    links.append(&unset);
    body.append(&links);
    let save = gtk::Button::with_label("Save timezone");
    save.add_css_class("suggested-action");
    pointer(&save);
    save.connect_clicked(move |_| {
        sender.emit(AppMsg::SetTimezone(picked.borrow().clone()));
    });
    body.append(&save);
    settings_section(
        "Timezone",
        "Hover a commit or reply time to see it in your timezone, and in the author's when they have set one. Others see yours the same way. Leave it unset to use this computer's timezone without sharing it.",
        &body,
    )
}

fn toggle_setting(
    title: &str,
    description: &str,
    on: bool,
    on_change: impl Fn(bool) + 'static,
) -> gtk::Box {
    let row = gtk::Box::new(gtk::Orientation::Horizontal, 16);
    let copy = gtk::Box::new(gtk::Orientation::Vertical, 8);
    copy.set_hexpand(true);
    let heading = gtk::Label::new(Some(title));
    heading.add_css_class("dn-section-title");
    heading.set_xalign(0.0);
    copy.append(&heading);
    let blurb = gtk::Label::new(Some(description));
    blurb.add_css_class("dn-muted");
    blurb.set_wrap(true);
    blurb.set_xalign(0.0);
    copy.append(&blurb);
    row.append(&copy);
    let toggle = gtk::Switch::new();
    toggle.set_valign(gtk::Align::Start);
    toggle.set_active(on);
    pointer(&toggle);
    toggle.connect_state_set(move |_, enabled| {
        on_change(enabled);
        gtk::glib::Propagation::Proceed
    });
    row.append(&toggle);
    let section = gtk::Box::new(gtk::Orientation::Vertical, 0);
    section.add_css_class("dn-section");
    section.append(&row);
    section
}

fn activity_graph_embed(user: &CurrentUser, sender: Sender<AppMsg>) -> gtk::Box {
    let base = devnull_api::default_base_url();
    let profile = user
        .user
        .html_url
        .clone()
        .unwrap_or_else(|| format!("{base}/@{}", user.username()));
    let graph = format!("{base}/@{}/graph.svg", user.username());
    let lane = lane_query(&user.user);
    let snippet = Rc::new(RefCell::new(graph_snippet(&graph, &profile, 0)));
    let picture = gtk::Picture::new();
    picture.set_can_shrink(true);
    picture.set_content_fit(gtk::ContentFit::Contain);
    picture.set_hexpand(true);
    picture.set_size_request(-1, 96);
    picture.add_css_class("dn-activity-preview-img");
    images::bind_picture(&picture, Some(&graph_preview_url(&graph, 0, &lane)));
    let frame = gtk::Box::new(gtk::Orientation::Vertical, 0);
    frame.add_css_class("dn-activity-preview");
    frame.append(&picture);
    let hint = gtk::Label::new(Some(graph_theme_hint(0)));
    hint.add_css_class("dn-muted");
    hint.set_wrap(true);
    hint.set_xalign(0.0);
    let preview = gtk::Label::new(Some(&snippet.borrow().clone()));
    preview.add_css_class("dn-code");
    preview.set_wrap(true);
    preview.set_xalign(0.0);
    preview.set_selectable(true);
    let copy = action_button(Some("dn-copy-symbolic"), "Copy Markdown", false, {
        let snippet = snippet.clone();
        let sender = sender.clone();
        move || {
            if copy_text(&snippet.borrow()) {
                sender.emit(AppMsg::Toast(
                    "Copied the Markdown. Paste it into your README.".into(),
                ));
            } else {
                sender.emit(AppMsg::Toast("fatal: could not copy the Markdown".into()));
            }
        }
    });
    let body = gtk::Box::new(gtk::Orientation::Vertical, 12);
    body.append(&segmented(
        &[
            ("Light", "dn-sun-symbolic", 0),
            ("Dark", "dn-moon-symbolic", 1),
            ("Auto", "dn-monitor-symbolic", 2),
        ],
        0,
        {
            let snippet = snippet.clone();
            let preview = preview.clone();
            let picture = picture.clone();
            let hint = hint.clone();
            let graph = graph.clone();
            let profile = profile.clone();
            let lane = lane.clone();
            move |value| {
                let next = graph_snippet(&graph, &profile, value);
                preview.set_label(&next);
                *snippet.borrow_mut() = next;
                hint.set_label(graph_theme_hint(value));
                images::bind_picture(&picture, Some(&graph_preview_url(&graph, value, &lane)));
            }
        },
    ));
    body.append(&hint);
    body.append(&frame);
    body.append(&preview);
    body.append(&copy);
    settings_section(
        "Embed your activity graph",
        "Your commit graph as an SVG for your GitHub profile README, or anywhere Markdown renders. It only shows approved commits and refreshes about once an hour.",
        &body,
    )
}

fn lane_query(user: &User) -> String {
    match user.lane.as_ref() {
        Some(Lane::Palette(index)) => index.to_string(),
        Some(Lane::Custom(hex)) => hex.clone(),
        None => "0".into(),
    }
}

fn graph_preview_url(graph: &str, theme: u8, lane: &str) -> String {
    let encoded: String = url::form_urlencoded::byte_serialize(lane.as_bytes()).collect();
    match theme {
        1 => format!("{graph}?theme=dark&lane={encoded}"),
        2 => format!("{graph}?theme=auto&lane={encoded}"),
        _ => format!("{graph}?lane={encoded}"),
    }
}

fn graph_theme_hint(theme: u8) -> &'static str {
    match theme {
        1 => "Always dark, whatever the reader prefers.",
        2 => "Follows the color scheme of whoever is reading.",
        _ => "Always light, whatever the reader prefers.",
    }
}

fn graph_snippet(graph: &str, profile: &str, theme: u8) -> String {
    let url = match theme {
        1 => format!("{graph}?theme=dark"),
        2 => format!("{graph}?theme=auto"),
        _ => graph.to_owned(),
    };
    format!("[![{}]({url})]({profile})", "/dev/null activity")
}

fn segmented(
    options: &[(&str, &str, u8)],
    selected: u8,
    on_change: impl Fn(u8) + 'static,
) -> gtk::Box {
    let row = gtk::Box::new(gtk::Orientation::Horizontal, 4);
    row.add_css_class("dn-segmented");
    let on_change = Rc::new(on_change);
    for (label, icon, value) in options {
        let button = gtk::Button::new();
        pointer(&button);
        button.add_css_class("flat");
        button.add_css_class("dn-segment");
        if *value == selected {
            button.add_css_class("active");
        }
        let inner = gtk::Box::new(gtk::Orientation::Horizontal, 6);
        inner.set_halign(gtk::Align::Center);
        inner.append(&named_icon(icon, 16));
        inner.append(&gtk::Label::new(Some(label)));
        button.set_child(Some(&inner));
        let on_change = on_change.clone();
        let value = *value;
        button.connect_clicked(move |_| on_change(value));
        row.append(&button);
    }
    row
}

fn copyable_code(label: &str, code: &str, sender: Sender<AppMsg>) -> gtk::Box {
    let wrap = gtk::Box::new(gtk::Orientation::Vertical, 4);
    let cap = gtk::Label::new(Some(label));
    cap.add_css_class("dn-field-key");
    cap.set_xalign(0.0);
    wrap.append(&cap);
    let row = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    row.add_css_class("dn-code");
    row.set_overflow(gtk::Overflow::Hidden);
    let text = gtk::Label::new(Some(code));
    text.add_css_class("dn-mono");
    text.set_xalign(0.0);
    text.set_wrap(true);
    text.set_hexpand(true);
    text.set_selectable(true);
    row.append(&text);
    let copy = gtk::Button::from_icon_name("dn-copy-symbolic");
    copy.add_css_class("flat");
    copy.set_tooltip_text(Some(&format!("Copy {label}")));
    pointer(&copy);
    let code = code.to_owned();
    let label = label.to_owned();
    copy.connect_clicked(move |_| {
        if copy_text(&code) {
            sender.emit(AppMsg::Toast(format!("Copied {label}")));
        } else {
            sender.emit(AppMsg::Toast(format!("fatal: could not copy {label}")));
        }
    });
    row.append(&copy);
    wrap.append(&row);
    wrap
}

fn terminal_step(
    number: u8,
    text: &str,
    code: Option<(&str, &str)>,
    sender: Sender<AppMsg>,
) -> gtk::Box {
    let wrap = gtk::Box::new(gtk::Orientation::Vertical, 8);
    let label = gtk::Label::new(Some(&format!("{number}. {text}")));
    label.set_wrap(true);
    label.set_xalign(0.0);
    wrap.append(&label);
    if let Some((name, snippet)) = code {
        wrap.append(&copyable_code(name, snippet, sender));
    }
    wrap
}

fn people_list(
    people: &[User],
    empty: &str,
    undo: &str,
    on_undo: impl Fn(String) + 'static,
    sender: Sender<AppMsg>,
) -> gtk::Box {
    let wrap = gtk::Box::new(gtk::Orientation::Vertical, 0);
    if people.is_empty() {
        let label = gtk::Label::new(Some(empty));
        label.add_css_class("dn-empty");
        label.add_css_class("dn-muted");
        wrap.append(&label);
        return wrap;
    }
    wrap.add_css_class("dn-fields");
    let on_undo = Rc::new(on_undo);
    for (index, user) in people.iter().enumerate() {
        let row = gtk::Box::new(gtk::Orientation::Horizontal, 12);
        row.add_css_class("dn-field-row");
        if index + 1 == people.len() {
            row.add_css_class("last");
        }
        let open = gtk::Button::new();
        pointer(&open);
        open.add_css_class("flat");
        open.set_hexpand(true);
        let inner = gtk::Box::new(gtk::Orientation::Horizontal, 10);
        inner.append(&avatar_widget(Some(user), 36, true));
        let col = gtk::Box::new(gtk::Orientation::Vertical, 0);
        let name = gtk::Label::new(Some(user.display_name()));
        name.add_css_class("dn-name");
        name.set_xalign(0.0);
        col.append(&name);
        let handle = gtk::Label::new(Some(&format!("@{}", user.username)));
        handle.add_css_class("dn-handle");
        handle.set_xalign(0.0);
        col.append(&handle);
        inner.append(&col);
        open.set_child(Some(&inner));
        let username = user.username.clone();
        let open_sender = sender.clone();
        open.connect_clicked(move |_| {
            open_sender.emit(AppMsg::OpenProfile(username.clone()));
        });
        row.append(&open);
        let action = gtk::Button::with_label(undo);
        pointer(&action);
        let username = user.username.clone();
        let on_undo = on_undo.clone();
        action.connect_clicked(move |_| on_undo(username.clone()));
        row.append(&action);
        wrap.append(&row);
    }
    wrap
}

fn synced_label(me: Option<&CurrentUser>) -> String {
    match me.and_then(|user| user.github_synced_at) {
        Some(at) => format!("Last synced {}", relative_time(Some(at))),
        None => "Last synced when you signed in".into(),
    }
}

fn iana_timezones() -> Vec<String> {
    let mut zones = Vec::new();
    if let Ok(text) = std::fs::read_to_string("/usr/share/zoneinfo/zone.tab") {
        for line in text.lines() {
            if line.starts_with('#') || line.is_empty() {
                continue;
            }
            if let Some(name) = line.split_whitespace().nth(2) {
                zones.push(name.to_owned());
            }
        }
        zones.sort();
        zones.dedup();
    }
    if zones.is_empty() {
        zones = vec![
            "UTC".into(),
            "America/New_York".into(),
            "America/Los_Angeles".into(),
            "Europe/London".into(),
            "Europe/Paris".into(),
            "Asia/Tokyo".into(),
        ];
    }
    zones
}

fn local_timezone() -> Option<String> {
    std::fs::read_link("/etc/localtime")
        .ok()
        .and_then(|path| {
            path.to_str()
                .and_then(|value| value.split("zoneinfo/").nth(1))
                .map(str::to_owned)
        })
        .or_else(|| std::env::var("TZ").ok().filter(|value| !value.is_empty()))
}

fn toplevel() -> Option<gtk::Window> {
    gtk::Window::list_toplevels()
        .into_iter()
        .find_map(|widget| widget.downcast::<gtk::Window>().ok())
}

fn activity_graph(activity: &Activity, lane_hex: &str) -> gtk::Box {
    let wrap = gtk::Box::new(gtk::Orientation::Vertical, 12);
    wrap.add_css_class("dn-activity");
    wrap.set_hexpand(true);
    wrap.set_overflow(gtk::Overflow::Hidden);

    let total: i64 = activity.buckets.iter().map(|b| b.count).sum();
    let empty = total == 0 && activity.scale != "year";
    let head = shrink_box(12);
    let title = gtk::Label::new(Some("Commit activity"));
    title.add_css_class("dn-activity-title");
    title.set_xalign(0.0);
    title.set_hexpand(true);
    head.append(&title);
    head.append(&activity_summary_label(total, empty, activity.window()));
    wrap.append(&head);

    let cells = activity.cells();
    let year = activity.scale == "year";
    let area = gtk::DrawingArea::new();
    area.set_hexpand(true);
    area.set_halign(gtk::Align::Fill);
    area.set_content_height(if year { 118 } else { 64 });
    area.set_has_tooltip(true);
    let (lr, lg, lb) = rgb(lane_hex);
    let dark = adw_is_dark();
    let (sr, sg, sb) = surface(dark);
    let (wr, wg, wb) = if dark {
        (0.18, 0.18, 0.19)
    } else {
        (0.93, 0.94, 0.95)
    };
    let (mr, mg, mb) = if dark {
        (0.63, 0.63, 0.65)
    } else {
        (0.36, 0.38, 0.42)
    };
    let n_cells = cells.len();
    area.connect_resize(move |area, width, _height| {
        if width <= 1 {
            return;
        }
        let height = activity_layout(year, n_cells, f64::from(width)).height();
        if area.content_height() != height {
            area.set_content_height(height);
        }
    });
    let draw_cells = cells.clone();
    area.set_draw_func(move |_area, cairo, width, _height| {
        let layout = activity_layout(year, draw_cells.len(), width as f64);
        for (index, cell) in draw_cells.iter().enumerate() {
            let (x, y, w, h) = layout.cell_at(index);
            let t = match cell.level {
                0 => 0.0,
                1 => 0.32,
                2 => 0.56,
                3 => 0.80,
                _ => 1.0,
            };
            if cell.level == 0 {
                cairo.set_source_rgb(wr, wg, wb);
            } else {
                cairo.set_source_rgb(sr + (lr - sr) * t, sg + (lg - sg) * t, sb + (lb - sb) * t);
            }
            rounded_cell(cairo, x, y, w, h, 2.0);
            let _ = cairo.fill();
        }
        cairo.set_source_rgb(mr, mg, mb);
        cairo.set_font_size(11.0);
        if year {
            cairo_month_labels(cairo, &layout, &draw_cells);
            cairo_weekday_labels(cairo, &layout);
        } else {
            cairo_strip_labels(cairo, &layout, &draw_cells);
        }
    });
    let tips = cells.clone();
    let scale = activity.scale.clone();
    area.connect_query_tooltip(move |area, x, y, _, tooltip| {
        let layout = activity_layout(year, tips.len(), f64::from(area.width()));
        let Some(index) = layout.hit(f64::from(x), f64::from(y)) else {
            return false;
        };
        tooltip.set_text(Some(&describe_bucket(&tips[index], &scale)));
        true
    });

    if year {
        let scroll = gtk::ScrolledWindow::new();
        scroll.set_policy(gtk::PolicyType::Automatic, gtk::PolicyType::Never);
        scroll.set_hexpand(true);
        scroll.set_propagate_natural_width(false);
        scroll.set_min_content_width(0);
        scroll.set_child(Some(&area));
        wrap.append(&scroll);
    } else {
        wrap.append(&area);
    }
    wrap.append(&activity_legend(lane_hex, dark));
    wrap
}

fn activity_summary_copy(total: i64, window: &str) -> (String, String) {
    (
        total.to_string(),
        format!("{} {window}", if total == 1 { "commit" } else { "commits" }),
    )
}

fn activity_summary_label(total: i64, empty: bool, window: &str) -> gtk::Box {
    let summary = gtk::Box::new(gtk::Orientation::Horizontal, 4);
    summary.set_halign(gtk::Align::End);
    if empty {
        let none = gtk::Label::new(Some("No commits yet"));
        none.add_css_class("dn-muted");
        summary.append(&none);
        return summary;
    }
    let (count, rest) = activity_summary_copy(total, window);
    let number = gtk::Label::new(Some(&count));
    number.add_css_class("dn-activity-count");
    number.add_css_class("dn-mono");
    summary.append(&number);
    let caption = gtk::Label::new(Some(&rest));
    caption.add_css_class("dn-muted");
    shrink_label(&caption);
    summary.append(&caption);
    summary
}

struct ActivityLayout {
    cols: usize,
    rows: usize,
    n: usize,
    cell_w: f64,
    cell_h: f64,
    gap: f64,
    origin_x: f64,
    origin_y: f64,
}

fn activity_layout(year: bool, n: usize, width: f64) -> ActivityLayout {
    let gap = 3.0;
    if year {
        let cols = n.div_ceil(7).max(1);
        let origin_x = 28.0;
        let origin_y = 16.0;
        let avail = (width - origin_x - gap * cols.saturating_sub(1) as f64).max(56.0);
        let cell = (avail / cols as f64).clamp(8.0, 13.0);
        ActivityLayout {
            cols,
            rows: 7,
            n,
            cell_w: cell,
            cell_h: cell,
            gap,
            origin_x,
            origin_y,
        }
    } else {
        let cols = n.max(1);
        let avail = (width - gap * cols.saturating_sub(1) as f64).max(24.0);
        ActivityLayout {
            cols,
            rows: 1,
            n,
            cell_w: avail / cols as f64,
            cell_h: 44.0,
            gap,
            origin_x: 0.0,
            origin_y: 0.0,
        }
    }
}

impl ActivityLayout {
    fn height(&self) -> i32 {
        let labels = if self.rows == 1 { 18.0 } else { 0.0 };
        (self.origin_y
            + self.rows as f64 * self.cell_h
            + self.rows.saturating_sub(1) as f64 * self.gap
            + labels)
            .ceil() as i32
    }

    fn cell_at(&self, index: usize) -> (f64, f64, f64, f64) {
        let (col, row) = if self.rows == 7 {
            (index / 7, index % 7)
        } else {
            (index, 0)
        };
        (
            self.origin_x + col as f64 * (self.cell_w + self.gap),
            self.origin_y + row as f64 * (self.cell_h + self.gap),
            self.cell_w,
            self.cell_h,
        )
    }

    fn hit(&self, x: f64, y: f64) -> Option<usize> {
        (0..self.n).find(|&index| {
            let (cx, cy, w, h) = self.cell_at(index);
            x >= cx && x < cx + w && y >= cy && y < cy + h
        })
    }
}

fn cairo_month_labels(
    cairo: &gtk::cairo::Context,
    layout: &ActivityLayout,
    cells: &[ActivityCell],
) {
    let mut last_month = None;
    for col in 0..layout.cols {
        let Some(cell) = cells.get(col * 7) else {
            continue;
        };
        let Some(day) = bucket_day(&cell.date) else {
            continue;
        };
        let month = day.month();
        let next_is_close = col == 0
            && (1..3).any(|ahead| {
                cells
                    .get((col + ahead) * 7)
                    .and_then(|next| bucket_day(&next.date))
                    .is_some_and(|next| next.month() != month)
            });
        if last_month == Some(month) || next_is_close {
            continue;
        }
        last_month = Some(month);
        let (x, _, _, _) = layout.cell_at(col * 7);
        cairo.move_to(x, 12.0);
        let _ = cairo.show_text(&day.format("%b").to_string());
    }
}

fn cairo_weekday_labels(cairo: &gtk::cairo::Context, layout: &ActivityLayout) {
    for (row, label) in ["", "Mon", "", "Wed", "", "Fri", ""]
        .into_iter()
        .enumerate()
    {
        if label.is_empty() {
            continue;
        }
        let (_, y, _, h) = layout.cell_at(row);
        cairo.move_to(0.0, y + h * 0.78);
        let _ = cairo.show_text(label);
    }
}

fn cairo_strip_labels(
    cairo: &gtk::cairo::Context,
    layout: &ActivityLayout,
    cells: &[ActivityCell],
) {
    let n = cells.len();
    for (index, cell) in cells.iter().enumerate() {
        let Some(label) = strip_label(cell, index, n) else {
            continue;
        };
        let (x, y, _, h) = layout.cell_at(index);
        cairo.move_to(x, y + h + 13.0);
        let _ = cairo.show_text(&label);
    }
}

fn strip_label(cell: &ActivityCell, index: usize, n: usize) -> Option<String> {
    let day = bucket_day(&cell.date)?;
    if cell.date.contains('T') {
        let local = day.with_timezone(&Local);
        return (local.hour() % 6 == 0 && index + 2 < n).then(|| local.format("%H:%M").to_string());
    }
    if n <= 7 {
        return Some(format!("{} {}", day.format("%a"), day.format("%-d")));
    }
    (day.weekday().num_days_from_sunday() == 1 && index + 3 < n)
        .then(|| format!("{} {}", day.format("%b"), day.format("%-d")))
}

fn bucket_day(date: &str) -> Option<DateTime<Utc>> {
    if let Ok(parsed) = DateTime::parse_from_rfc3339(date) {
        return Some(parsed.with_timezone(&Utc));
    }
    NaiveDate::parse_from_str(date, "%Y-%m-%d")
        .ok()?
        .and_hms_opt(0, 0, 0)
        .map(|naive| naive.and_utc())
}

fn describe_bucket(cell: &ActivityCell, scale: &str) -> String {
    let commits = if cell.count == 0 {
        "No commits".to_owned()
    } else if cell.count == 1 {
        "1 commit".to_owned()
    } else {
        format!("{} commits", cell.count)
    };
    let Some(day) = bucket_day(&cell.date) else {
        return commits;
    };
    if scale != "day" {
        let formatted = if scale == "year" {
            day.format("%b %-d, %Y").to_string()
        } else {
            day.format("%a, %b %-d").to_string()
        };
        return format!("{commits} on {formatted}");
    }
    let start = day.with_timezone(&Local);
    let end = start + ChronoDuration::hours(1);
    let today = Local::now().date_naive();
    let when = if start.date_naive() == today {
        "today"
    } else {
        "yesterday"
    };
    format!(
        "{commits} between {} and {} {when}",
        start.format("%H:%M"),
        end.format("%H:%M")
    )
}

fn activity_legend(lane_hex: &str, dark: bool) -> gtk::Box {
    let row = gtk::Box::new(gtk::Orientation::Horizontal, 4);
    row.set_halign(gtk::Align::End);
    let less = gtk::Label::new(Some("Less"));
    less.add_css_class("dn-muted");
    less.add_css_class("dn-activity-legend-label");
    row.append(&less);
    let (lr, lg, lb) = rgb(lane_hex);
    let (sr, sg, sb) = surface(dark);
    let (wr, wg, wb) = if dark {
        (0.18, 0.18, 0.19)
    } else {
        (0.93, 0.94, 0.95)
    };
    for level in 0..=4 {
        let swatch = gtk::DrawingArea::new();
        swatch.set_content_width(10);
        swatch.set_content_height(10);
        let t = match level {
            0 => 0.0,
            1 => 0.32,
            2 => 0.56,
            3 => 0.80,
            _ => 1.0,
        };
        swatch.set_draw_func(move |_, cairo, width, height| {
            if level == 0 {
                cairo.set_source_rgb(wr, wg, wb);
            } else {
                cairo.set_source_rgb(sr + (lr - sr) * t, sg + (lg - sg) * t, sb + (lb - sb) * t);
            }
            rounded_cell(cairo, 0.0, 0.0, f64::from(width), f64::from(height), 2.0);
            let _ = cairo.fill();
        });
        row.append(&swatch);
    }
    let more = gtk::Label::new(Some("More"));
    more.add_css_class("dn-muted");
    more.add_css_class("dn-activity-legend-label");
    row.append(&more);
    row
}

fn rounded_cell(cairo: &gtk::cairo::Context, x: f64, y: f64, w: f64, h: f64, r: f64) {
    cairo.new_sub_path();
    cairo.arc(x + w - r, y + r, r, -std::f64::consts::FRAC_PI_2, 0.0);
    cairo.arc(x + w - r, y + h - r, r, 0.0, std::f64::consts::FRAC_PI_2);
    cairo.arc(
        x + r,
        y + h - r,
        r,
        std::f64::consts::FRAC_PI_2,
        std::f64::consts::PI,
    );
    cairo.arc(
        x + r,
        y + r,
        r,
        std::f64::consts::PI,
        3.0 * std::f64::consts::FRAC_PI_2,
    );
    cairo.close_path();
}

fn streaks_row(streaks: &Streaks, lane_hex: &str) -> gtk::Box {
    let row = shrink_box(8);
    row.add_css_class("dn-streaks");
    row.append(&streak_phrase(
        "longest streak",
        streaks.longest,
        if streaks.longest == 1 { "day" } else { "days" },
    ));
    if streaks.current > 0 {
        let current = gtk::Box::new(gtk::Orientation::Horizontal, 6);
        let dot = gtk::DrawingArea::new();
        dot.set_content_width(8);
        dot.set_content_height(8);
        dot.set_valign(gtk::Align::Center);
        let (lr, lg, lb) = rgb(lane_hex);
        dot.set_draw_func(move |_, cairo, width, height| {
            cairo.set_source_rgb(lr, lg, lb);
            cairo.arc(
                f64::from(width) / 2.0,
                f64::from(height) / 2.0,
                4.0,
                0.0,
                std::f64::consts::TAU,
            );
            let _ = cairo.fill();
        });
        current.append(&dot);
        current.append(&streak_phrase("current", streaks.current, ""));
        row.append(&current);
    }
    row
}

fn streak_phrase(prefix: &str, value: i64, suffix: &str) -> gtk::Box {
    let row = gtk::Box::new(gtk::Orientation::Horizontal, 4);
    let lead = gtk::Label::new(Some(prefix));
    lead.add_css_class("dn-mono");
    lead.add_css_class("dn-muted");
    row.append(&lead);
    let number = gtk::Label::new(Some(&value.to_string()));
    number.add_css_class("dn-mono");
    number.add_css_class("dn-name");
    row.append(&number);
    if !suffix.is_empty() {
        let rest = gtk::Label::new(Some(suffix));
        rest.add_css_class("dn-mono");
        rest.add_css_class("dn-muted");
        row.append(&rest);
    }
    row
}

fn achievements_shelf(
    achievements: &Achievements,
    lane_hex: &str,
    sender: Sender<AppMsg>,
) -> gtk::Box {
    let wrap = gtk::Box::new(gtk::Orientation::Vertical, 10);
    wrap.add_css_class("dn-achievements");
    wrap.set_hexpand(true);

    let head = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    let open = gtk::Button::new();
    pointer(&open);
    open.add_css_class("flat");
    open.add_css_class("dn-achievements-open");
    let inner = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    inner.append(&named_icon("dn-award-symbolic", 16));
    let title = gtk::Label::new(Some(&achievements.label()));
    title.add_css_class("dn-name");
    title.add_css_class("dn-mono");
    ellipsize_label(&title);
    inner.append(&title);
    let xp = gtk::Label::new(Some(&format!("{} XP", achievements.xp)));
    xp.add_css_class("dn-mono");
    xp.add_css_class("dn-muted");
    ellipsize_label(&xp);
    inner.append(&xp);
    open.set_child(Some(&inner));
    let inspect = sender.clone();
    open.connect_clicked(move |_| inspect.emit(AppMsg::InspectAchievements));
    head.append(&open);
    let spacer = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    spacer.set_hexpand(true);
    head.append(&spacer);
    let more = gtk::Button::new();
    pointer(&more);
    more.add_css_class("flat");
    more.add_css_class("dn-achievements-more");
    let more_inner = gtk::Box::new(gtk::Orientation::Horizontal, 4);
    let unlocked = gtk::Label::new(Some(&format!(
        "{} / {} unlocked",
        achievements.unlocked_count(),
        achievements.badges.len()
    )));
    unlocked.add_css_class("dn-mono");
    unlocked.add_css_class("dn-muted");
    more_inner.append(&unlocked);
    more_inner.append(&named_icon("dn-chevron-right-symbolic", 14));
    more.set_child(Some(&more_inner));
    let more_sender = sender.clone();
    more.connect_clicked(move |_| more_sender.emit(AppMsg::InspectAchievements));
    head.append(&more);
    wrap.append(&head);

    wrap.append(&xp_progress(achievements, lane_hex));
    wrap.append(&achievements_badge_row(achievements, lane_hex, sender));
    wrap
}

fn achievements_badge_row(
    achievements: &Achievements,
    lane_hex: &str,
    sender: Sender<AppMsg>,
) -> gtk::MenuButton {
    let unlocked: Vec<&Badge> = achievements.unlocked_badges().collect();
    let row = gtk::MenuButton::new();
    pointer(&row);
    row.add_css_class("flat");
    row.add_css_class("dn-badge-row");
    row.set_always_show_arrow(false);
    row.set_hexpand(true);
    row.set_tooltip_text(Some("Show all achievements"));

    let inner = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    inner.set_hexpand(true);
    inner.set_overflow(gtk::Overflow::Hidden);
    inner.set_halign(gtk::Align::Fill);
    inner.set_valign(gtk::Align::Center);

    let mut chips = Vec::new();
    for badge in &unlocked {
        let chip = badge_face(badge, lane_hex);
        chip.set_hexpand(false);
        chip.set_halign(gtk::Align::Start);
        chip.set_visible(false);
        inner.append(&chip);
        chips.push(chip.upcast::<gtk::Widget>());
    }

    let more = gtk::Label::new(Some(&format!("+{} more", unlocked.len())));
    more.add_css_class("dn-badge-more");
    more.add_css_class("dn-mono");
    more.set_hexpand(false);
    more.set_width_chars(6 + unlocked.len().max(1).to_string().len() as i32);
    more.set_visible(false);
    inner.append(&more);

    if unlocked.is_empty() {
        let empty = gtk::Label::new(Some("View achievements"));
        empty.add_css_class("dn-mono");
        empty.add_css_class("dn-muted");
        empty.set_xalign(0.0);
        empty.set_width_chars(1);
        empty.set_ellipsize(pango::EllipsizeMode::End);
        inner.append(&empty);
    }

    let overlay = gtk::Overlay::new();
    overlay.set_hexpand(true);
    overlay.set_overflow(gtk::Overflow::Hidden);
    let sizer = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    sizer.set_hexpand(true);
    sizer.set_height_request(32);
    overlay.set_child(Some(&sizer));
    overlay.add_overlay(&inner);
    row.set_child(Some(&overlay));

    let popover = gtk::Popover::new();
    popover.set_has_arrow(false);
    popover.set_position(gtk::PositionType::Bottom);
    let body = gtk::Box::new(gtk::Orientation::Vertical, 12);
    body.add_css_class("dn-achievements-popover");
    let title = gtk::Label::new(Some("All achievements"));
    title.add_css_class("dn-achievements-popover-title");
    title.set_xalign(0.0);
    body.append(&title);
    let grid = gtk::FlowBox::new();
    grid.set_selection_mode(gtk::SelectionMode::None);
    grid.set_min_children_per_line(2);
    grid.set_max_children_per_line(8);
    grid.set_column_spacing(8);
    grid.set_row_spacing(8);
    grid.set_homogeneous(false);
    grid.add_css_class("dn-badge-flow");
    for badge in &achievements.badges {
        grid.insert(
            &badge_chip(badge, lane_hex, sender.clone(), Some(popover.clone())),
            -1,
        );
    }
    body.append(&grid);
    popover.set_child(Some(&body));
    row.set_popover(Some(&popover));

    if !chips.is_empty() {
        bind_badge_overflow(overlay.upcast(), chips, more);
    }
    row
}

const BADGE_ROW_GAP: i32 = 8;

fn visible_badge_count(widths: &[i32], more_width: i32, available: i32) -> usize {
    if widths.is_empty() || available <= 1 {
        return 0;
    }
    let total =
        widths.iter().sum::<i32>() + BADGE_ROW_GAP * (widths.len().saturating_sub(1) as i32);
    let mut used = 0;
    let mut count = 0;
    for width in widths {
        let next = if count == 0 {
            *width
        } else {
            used + BADGE_ROW_GAP + *width
        };
        if total > available && next + BADGE_ROW_GAP + more_width > available {
            break;
        }
        used = next;
        count += 1;
    }
    count
}

fn badge_more_label(hidden: usize) -> String {
    format!("+{hidden} more")
}

fn bind_badge_overflow(watch: gtk::Widget, chips: Vec<gtk::Widget>, more: gtk::Label) {
    let last_width = Rc::new(Cell::new(-1i32));
    let last_count = Rc::new(Cell::new(usize::MAX));
    watch.add_tick_callback(move |watch, _| {
        let available = watch.width();
        if available <= 1 {
            return glib::ControlFlow::Continue;
        }
        if available == last_width.get() {
            return glib::ControlFlow::Continue;
        }
        last_width.set(available);
        fit_badge_overflow(chips.as_slice(), &more, available, &last_count);
        glib::ControlFlow::Continue
    });
}

fn fit_badge_overflow(
    chips: &[gtk::Widget],
    more: &gtk::Label,
    available: i32,
    last_count: &Cell<usize>,
) {
    let more_w = more.measure(gtk::Orientation::Horizontal, -1).1.max(48);
    let widths: Vec<i32> = chips
        .iter()
        .map(|chip| chip.measure(gtk::Orientation::Horizontal, -1).1)
        .collect();
    let count = visible_badge_count(&widths, more_w, available);
    if count == last_count.get() {
        return;
    }
    last_count.set(count);
    for (index, chip) in chips.iter().enumerate() {
        let show = index < count;
        if chip.is_visible() != show {
            chip.set_visible(show);
        }
    }
    let hidden = chips.len().saturating_sub(count);
    let more_visible = hidden > 0;
    if more.is_visible() != more_visible {
        more.set_visible(more_visible);
    }
    let label = badge_more_label(hidden);
    if more.text().as_str() != label {
        more.set_text(&label);
    }
}

fn xp_progress(achievements: &Achievements, lane_hex: &str) -> gtk::Box {
    let col = gtk::Box::new(gtk::Orientation::Vertical, 4);
    let meta = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    let left = gtk::Label::new(Some(&achievements.xp_label()));
    left.add_css_class("dn-mono");
    left.add_css_class("dn-muted");
    left.add_css_class("dn-xp-meta");
    ellipsize_label(&left);
    meta.append(&left);
    let right = gtk::Label::new(Some(&achievements.next_rank_label()));
    right.add_css_class("dn-mono");
    right.add_css_class("dn-muted");
    right.add_css_class("dn-xp-meta");
    ellipsize_label(&right);
    right.set_xalign(1.0);
    meta.append(&right);
    col.append(&meta);
    col.append(&xp_bar(achievements.level_progress, lane_hex));
    col
}

fn xp_bar(progress: i64, lane_hex: &str) -> gtk::DrawingArea {
    let area = gtk::DrawingArea::new();
    area.add_css_class("dn-xp-bar");
    area.set_content_height(6);
    area.set_hexpand(true);
    let t = (progress as f64 / 100.0).clamp(0.0, 1.0);
    let (lr, lg, lb) = rgb(lane_hex);
    let dark = adw_is_dark();
    let (wr, wg, wb) = if dark {
        (0.18, 0.18, 0.19)
    } else {
        (0.93, 0.94, 0.95)
    };
    area.set_draw_func(move |_, cairo, width, height| {
        let w = f64::from(width);
        let h = f64::from(height);
        cairo.set_source_rgb(wr, wg, wb);
        rounded_cell(cairo, 0.0, 0.0, w, h, h / 2.0);
        let _ = cairo.fill();
        let fill = w * t;
        if fill > 0.0 {
            cairo.set_source_rgb(lr, lg, lb);
            rounded_cell(cairo, 0.0, 0.0, fill.max(h), h, h / 2.0);
            let _ = cairo.fill();
        }
    });
    area
}

fn badge_face(badge: &Badge, lane_hex: &str) -> gtk::Box {
    let row = gtk::Box::new(gtk::Orientation::Horizontal, 6);
    row.add_css_class("dn-badge");
    if badge.unlocked {
        row.add_css_class("unlocked");
        row.add_css_class(&hex_tone_class(lane_hex));
        let motion = gtk::EventControllerMotion::new();
        let hover = row.clone();
        motion.connect_enter(move |_, _, _| hover.add_css_class("hover"));
        let hover = row.clone();
        motion.connect_leave(move |_| hover.remove_css_class("hover"));
        row.add_controller(motion);
    } else {
        row.add_css_class("locked");
    }
    row.append(&named_icon(badge_icon_name(&badge.icon), 14));
    let name = gtk::Label::new(Some(&badge.name));
    name.add_css_class("dn-mono");
    name.set_ellipsize(pango::EllipsizeMode::End);
    name.set_width_chars(1);
    name.set_max_width_chars(16);
    row.append(&name);
    if !badge.unlocked && badge.target > 1 {
        let progress = gtk::Label::new(Some(&format!("({}/{})", badge.progress, badge.target)));
        progress.add_css_class("dn-mono");
        progress.add_css_class("dn-muted");
        row.append(&progress);
    }
    row
}

fn badge_chip(
    badge: &Badge,
    lane_hex: &str,
    sender: Sender<AppMsg>,
    popover: Option<gtk::Popover>,
) -> gtk::Button {
    let btn = gtk::Button::new();
    pointer(&btn);
    btn.add_css_class("flat");
    btn.add_css_class("dn-badge-chip");
    btn.set_halign(gtk::Align::Start);
    btn.set_tooltip_text(Some(&format!(
        "{} {}\n{}",
        badge.name,
        if badge.unlocked {
            "✓".to_owned()
        } else {
            format!("({}/{})", badge.progress, badge.target)
        },
        badge.description
    )));
    btn.set_child(Some(&badge_face(badge, lane_hex)));
    btn.connect_clicked(move |_| {
        if let Some(popover) = &popover {
            popover.popdown();
        }
        sender.emit(AppMsg::InspectAchievements);
    });
    btn
}

fn badge_icon_name(icon: &str) -> &'static str {
    match icon {
        "terminal" => "dn-terminal-symbolic",
        "file-code" => "dn-file-code-symbolic",
        "flame" => "dn-flame-symbolic",
        "zap" => "dn-zap-symbolic",
        "server" => "dn-server-symbolic",
        "box" => "dn-box-symbolic",
        "shield" => "dn-shield-symbolic",
        "star" => "dn-star-symbolic",
        "sparkles" => "dn-sparkles-symbolic",
        "git-merge" => "dn-git-merge-symbolic",
        "key" => "dn-key-symbolic",
        _ => "dn-terminal-symbolic",
    }
}

pub fn achievements_dialog(
    achievements: &Achievements,
    user: &User,
    lane_hex: &str,
    lane_class_name: &str,
) -> adw::Dialog {
    let dialog = adw::Dialog::new();
    dialog.set_title("Developer Achievements");
    dialog.set_content_width(560);
    dialog.set_content_height(640);
    dialog.set_follows_content_size(false);

    let toolbar = adw::ToolbarView::new();
    toolbar.add_top_bar(&adw::HeaderBar::new());

    let scroll = gtk::ScrolledWindow::new();
    scroll.set_policy(gtk::PolicyType::Never, gtk::PolicyType::Automatic);
    scroll.set_vexpand(true);
    let body = gtk::Box::new(gtk::Orientation::Vertical, 12);
    body.add_css_class("dn-achievements-dialog");
    body.set_margin_top(8);
    body.set_margin_bottom(16);
    body.set_margin_start(16);
    body.set_margin_end(16);

    let head = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    let title = gtk::Label::new(Some("Developer Achievements"));
    title.add_css_class("dn-title");
    title.set_xalign(0.0);
    title.set_hexpand(true);
    head.append(&title);
    let chip = gtk::Label::new(Some(&achievements.label()));
    chip.add_css_class("dn-achievements-chip");
    chip.add_css_class(&hex_tone_class(lane_hex));
    head.append(&chip);
    body.append(&head);

    let sub = gtk::Label::new(Some(&format!(
        "@{} · {} XP · {}/{} badges unlocked",
        user.username,
        achievements.xp,
        achievements.unlocked_count(),
        achievements.badges.len()
    )));
    sub.add_css_class("dn-mono");
    sub.add_css_class("dn-muted");
    sub.set_xalign(0.0);
    body.append(&sub);

    let progress = gtk::Box::new(gtk::Orientation::Vertical, 0);
    progress.add_css_class("dn-xp-card");
    progress.set_margin_top(4);
    progress.append(&xp_progress(achievements, lane_hex));
    body.append(&progress);

    let tabs = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    tabs.add_css_class("dn-achieve-tabs");
    let badges_tab = gtk::Button::with_label(&format!("Badges ({})", achievements.badges.len()));
    let ranks_tab = gtk::Button::with_label(&format!("Unix Ranks ({})", achievements.ranks.len()));
    pointer(&badges_tab);
    pointer(&ranks_tab);
    badges_tab.add_css_class("flat");
    ranks_tab.add_css_class("flat");
    badges_tab.add_css_class("dn-achieve-tab");
    ranks_tab.add_css_class("dn-achieve-tab");
    badges_tab.add_css_class("active");
    badges_tab.add_css_class(lane_class_name);
    ranks_tab.add_css_class(lane_class_name);
    tabs.append(&badges_tab);
    tabs.append(&ranks_tab);
    body.append(&tabs);

    let stack = gtk::Stack::new();
    stack.set_vhomogeneous(false);
    stack.add_named(&achievement_badges_page(achievements), Some("badges"));
    stack.add_named(
        &achievement_ranks_page(achievements, lane_class_name),
        Some("ranks"),
    );
    let stack_badges = stack.clone();
    let stack_ranks = stack.clone();
    let badges_btn = badges_tab.clone();
    let ranks_btn = ranks_tab.clone();
    badges_tab.connect_clicked(move |_| {
        stack_badges.set_visible_child_name("badges");
        badges_btn.add_css_class("active");
        ranks_btn.remove_css_class("active");
    });
    let badges_btn = badges_tab.clone();
    let ranks_btn = ranks_tab.clone();
    ranks_tab.connect_clicked(move |_| {
        stack_ranks.set_visible_child_name("ranks");
        ranks_btn.add_css_class("active");
        badges_btn.remove_css_class("active");
    });
    body.append(&stack);

    let foot = gtk::Label::new(Some(
        "XP: 10/commit · 15/day streak · 5/star received · 5/reply · 2/star given",
    ));
    foot.add_css_class("dn-mono");
    foot.add_css_class("dn-muted");
    foot.add_css_class("dn-xp-footnote");
    foot.set_wrap(true);
    foot.set_xalign(0.5);
    body.append(&foot);

    scroll.set_child(Some(&body));
    toolbar.set_content(Some(&scroll));
    dialog.set_child(Some(&toolbar));
    dialog
}

fn achievement_badges_page(achievements: &Achievements) -> gtk::FlowBox {
    let grid = gtk::FlowBox::new();
    grid.set_selection_mode(gtk::SelectionMode::None);
    grid.set_min_children_per_line(1);
    grid.set_max_children_per_line(2);
    grid.set_column_spacing(8);
    grid.set_row_spacing(8);
    grid.set_homogeneous(true);
    for badge in &achievements.badges {
        grid.insert(&achievement_badge_card(badge), -1);
    }
    grid
}

fn achievement_badge_card(badge: &Badge) -> gtk::Box {
    let card = gtk::Box::new(gtk::Orientation::Vertical, 8);
    card.add_css_class("dn-badge-card");
    if !badge.unlocked {
        card.add_css_class("locked");
    }
    let top = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    let icon_wrap = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    icon_wrap.add_css_class("dn-badge-icon");
    icon_wrap.set_halign(gtk::Align::Center);
    icon_wrap.set_valign(gtk::Align::Center);
    icon_wrap.append(&named_icon(badge_icon_name(&badge.icon), 14));
    top.append(&icon_wrap);
    let name = gtk::Label::new(Some(&badge.name));
    name.add_css_class("dn-name");
    name.add_css_class("dn-mono");
    name.set_xalign(0.0);
    name.set_hexpand(true);
    top.append(&name);
    let status = gtk::Label::new(Some(&if badge.unlocked {
        "✓".to_owned()
    } else {
        format!("{}/{}", badge.progress, badge.target)
    }));
    status.add_css_class("dn-mono");
    status.add_css_class(if badge.unlocked {
        "dn-badge-done"
    } else {
        "dn-muted"
    });
    top.append(&status);
    card.append(&top);
    if !badge.description.is_empty() {
        let desc = gtk::Label::new(Some(&badge.description));
        desc.add_css_class("dn-muted");
        desc.set_wrap(true);
        desc.set_xalign(0.0);
        card.append(&desc);
    }
    if !badge.unlocked && badge.target > 1 {
        let frac = (badge.progress as f64 / badge.target as f64).clamp(0.0, 1.0);
        card.append(&xp_bar((frac * 100.0).round() as i64, "#8a8d91"));
    }
    card
}

fn achievement_ranks_page(achievements: &Achievements, lane_class_name: &str) -> gtk::Box {
    let list = gtk::Box::new(gtk::Orientation::Vertical, 8);
    for rank in &achievements.ranks {
        let row = gtk::Box::new(gtk::Orientation::Horizontal, 10);
        row.add_css_class("dn-rank-card");
        let current = rank.level == achievements.level;
        if current {
            row.add_css_class("current");
            row.add_css_class(lane_class_name);
        } else if !rank.achieved {
            row.add_css_class("locked");
        }
        let level = gtk::Label::new(Some(&rank.level.to_string()));
        level.add_css_class("dn-rank-level");
        level.add_css_class("dn-mono");
        row.append(&level);
        let text = gtk::Box::new(gtk::Orientation::Vertical, 2);
        text.set_hexpand(true);
        let name_row = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        let name = gtk::Label::new(Some(&rank.rank));
        name.add_css_class("dn-name");
        name.add_css_class("dn-mono");
        name.set_xalign(0.0);
        name_row.append(&name);
        if current {
            let now = gtk::Label::new(Some("CURRENT"));
            now.add_css_class("dn-rank-now");
            now.add_css_class("dn-mono");
            name_row.append(&now);
        }
        text.append(&name_row);
        if !rank.description.is_empty() {
            let desc = gtk::Label::new(Some(&rank.description));
            desc.add_css_class("dn-muted");
            desc.set_wrap(true);
            desc.set_xalign(0.0);
            text.append(&desc);
        }
        row.append(&text);
        let xp = gtk::Box::new(gtk::Orientation::Vertical, 2);
        xp.set_halign(gtk::Align::End);
        let amount = gtk::Label::new(Some(&format!("{} XP", rank.min_xp)));
        amount.add_css_class("dn-mono");
        amount.set_xalign(1.0);
        xp.append(&amount);
        let state = gtk::Label::new(Some(if rank.achieved {
            "✓ Achieved"
        } else {
            "Requires XP"
        }));
        state.add_css_class("dn-mono");
        state.add_css_class(if rank.achieved {
            "dn-badge-done"
        } else {
            "dn-muted"
        });
        state.set_xalign(1.0);
        xp.append(&state);
        row.append(&xp);
        list.append(&row);
    }
    list
}

pub fn fill_live_following(box_: &gtk::Box, users: &[User], sender: Sender<AppMsg>) {
    while let Some(child) = box_.first_child() {
        box_.remove(&child);
    }
    for user in users.iter().filter(|user| user.is_live()).take(3) {
        box_.append(&live_following_row(user, sender.clone()));
    }
}

fn live_following_row(user: &User, sender: Sender<AppMsg>) -> gtk::Button {
    let row = gtk::Button::new();
    pointer(&row);
    row.add_css_class("flat");
    row.add_css_class("dn-live-row");
    row.set_tooltip_text(Some(&format!("Watch {} live", user.display_name())));
    let inner = gtk::Box::new(gtk::Orientation::Horizontal, 10);
    inner.set_valign(gtk::Align::Center);
    inner.append(&avatar_widget(Some(user), 28, false));
    let col = gtk::Box::new(gtk::Orientation::Vertical, 0);
    col.set_hexpand(true);
    let name = gtk::Label::new(Some(user.display_name()));
    name.add_css_class("dn-name");
    name.set_xalign(0.0);
    name.set_ellipsize(pango::EllipsizeMode::End);
    col.append(&name);
    let handle = gtk::Label::new(Some(&format!("@{}", user.username)));
    handle.add_css_class("dn-handle");
    handle.add_css_class("dn-mono");
    handle.set_xalign(0.0);
    handle.set_ellipsize(pango::EllipsizeMode::End);
    col.append(&handle);
    inner.append(&col);
    let arrow = gtk::Image::from_icon_name("dn-arrow-up-right-symbolic");
    arrow.set_pixel_size(16);
    arrow.add_css_class("dn-live-arrow");
    inner.append(&arrow);
    row.set_child(Some(&inner));
    if let Some(url) = user.live_url().map(str::to_owned) {
        row.connect_clicked(move |_| {
            sender.emit(AppMsg::OpenWeb(url.clone()));
        });
    }
    row
}

fn live_badge() -> gtk::Box {
    let badge = gtk::Box::new(gtk::Orientation::Horizontal, 3);
    badge.add_css_class("dn-live-badge");
    badge.set_valign(gtk::Align::End);
    badge.set_halign(gtk::Align::Center);
    let dot = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    dot.add_css_class("dn-live-dot");
    badge.append(&dot);
    let label = gtk::Label::new(Some("LIVE"));
    label.add_css_class("dn-live-badge-label");
    badge.append(&label);
    badge
}

pub fn suggestion_candidates<'a>(users: &'a [User], me: Option<&str>) -> Vec<&'a User> {
    users
        .iter()
        .filter(|user| Some(user.username.as_str()) != me)
        .filter(|user| !user.is_followed_by_me())
        .take(3)
        .collect()
}

pub fn fill_suggestions(box_: &gtk::Box, users: &[User], me: Option<&str>, sender: Sender<AppMsg>) {
    while let Some(child) = box_.first_child() {
        box_.remove(&child);
    }
    let shown = suggestion_candidates(users, me);
    if shown.is_empty() {
        let empty = gtk::Label::new(Some("No suggestions right now."));
        empty.add_css_class("dn-muted");
        empty.add_css_class("dn-mono");
        empty.set_xalign(0.0);
        empty.set_wrap(true);
        box_.append(&empty);
        return;
    }
    for user in shown {
        box_.append(&suggestion_row(user, sender.clone()));
    }
}

fn suggestion_row(user: &User, sender: Sender<AppMsg>) -> gtk::Box {
    let row = gtk::Box::new(gtk::Orientation::Horizontal, 10);
    row.add_css_class("dn-suggest-row");
    row.set_overflow(gtk::Overflow::Visible);
    let open = gtk::Button::new();
    pointer(&open);
    open.add_css_class("flat");
    open.set_overflow(gtk::Overflow::Visible);
    open.set_hexpand(true);
    let inner = gtk::Box::new(gtk::Orientation::Horizontal, 10);
    inner.append(&avatar_widget(Some(user), 36, false));
    let col = gtk::Box::new(gtk::Orientation::Vertical, 0);
    let name = gtk::Label::new(Some(user.display_name()));
    name.add_css_class("dn-name");
    name.set_xalign(0.0);
    name.set_ellipsize(pango::EllipsizeMode::End);
    col.append(&name);
    let handle_row = gtk::Box::new(gtk::Orientation::Horizontal, 6);
    handle_row.append(&handle_dot(Some(user)));
    let handle = gtk::Label::new(Some(&format!("@{}", user.username)));
    handle.add_css_class("dn-handle");
    handle.set_xalign(0.0);
    handle_row.append(&handle);
    col.append(&handle_row);
    inner.append(&col);
    open.set_child(Some(&inner));
    let username = user.username.clone();
    let open_sender = sender.clone();
    open.connect_clicked(move |_| {
        open_sender.emit(AppMsg::OpenProfile(username.clone()));
    });
    row.append(&open);
    let following = user.is_followed.unwrap_or(false);
    let username = user.username.clone();
    let follow = icon_only_button(
        if following {
            "dn-user-minus-symbolic"
        } else {
            "dn-user-plus-symbolic"
        },
        if following { "Unfollow" } else { "Follow" },
        false,
    );
    if following {
        follow.add_css_class("dn-unfollow");
    }
    let follow_sender = sender.clone();
    let follow_name = username.clone();
    follow.connect_clicked(move |_| {
        follow_sender.emit(AppMsg::FollowUser(follow_name.clone(), !following));
    });
    row.append(&follow);
    let remove = icon_only_button("dn-x-symbolic", "Remove", false);
    remove.connect_clicked(move |_| {
        sender.emit(AppMsg::DismissSuggested(username.clone()));
    });
    row.append(&remove);
    row
}

pub fn append_search(list: &gtk::ListBox, results: &SearchResults, ctx: &FeedCtx) {
    clear_list(list);
    if results.is_empty() {
        list.append(&root_row("fatal: nothing matched."));
        return;
    }
    if !results.users.is_empty() {
        list.append(&section_row("People"));
        for user in &results.users {
            list.append(&search_user_row(user, ctx.sender.clone()));
        }
    }
    if !results.commits.is_empty() {
        list.append(&section_row("Commits"));
        let mut previous = None;
        let mut last_day = None;
        append_commits(
            list,
            &results.commits,
            ctx,
            &mut previous,
            &mut last_day,
            None,
            None,
        );
    }
}

fn section_row(title: &str) -> gtk::ListBoxRow {
    let row = gtk::ListBoxRow::new();
    row.set_activatable(false);
    let label = gtk::Label::new(Some(title));
    label.add_css_class("dn-day");
    label.set_xalign(0.0);
    row.set_child(Some(&label));
    row
}

fn search_user_row(user: &User, sender: Sender<AppMsg>) -> gtk::ListBoxRow {
    let row = gtk::ListBoxRow::new();
    row.set_activatable(true);
    let inner = gtk::Box::new(gtk::Orientation::Horizontal, 12);
    inner.add_css_class("dn-search-user");
    inner.append(&avatar_widget(Some(user), 40, true));
    let col = gtk::Box::new(gtk::Orientation::Vertical, 2);
    let name = gtk::Label::new(Some(user.display_name()));
    name.add_css_class("dn-name");
    name.set_xalign(0.0);
    col.append(&name);
    let handle_row = gtk::Box::new(gtk::Orientation::Horizontal, 6);
    handle_row.append(&handle_dot(Some(user)));
    let handle = gtk::Label::new(Some(&format!("@{}", user.username)));
    handle.add_css_class("dn-handle");
    handle_row.append(&handle);
    col.append(&handle_row);
    inner.append(&col);
    row.set_child(Some(&inner));
    let username = user.username.clone();
    row.connect_activate(move |_| {
        sender.emit(AppMsg::OpenProfile(username.clone()));
    });
    row
}

pub fn adw_is_dark() -> bool {
    adw::StyleManager::default().is_dark()
}

pub fn mime_for(path: &str) -> &'static str {
    let lower = path.to_ascii_lowercase();
    if lower.ends_with(".png") {
        "image/png"
    } else if lower.ends_with(".webp") {
        "image/webp"
    } else {
        "image/jpeg"
    }
}

pub fn read_upload(path: &std::path::Path) -> Option<Upload> {
    let data = std::fs::read(path).ok()?;
    let filename = path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("image.jpg");
    upload_from_bytes(&data, filename)
}

pub fn read_upload_file(file: &gtk::gio::File) -> Option<Upload> {
    if let Some(path) = file.path() {
        if let Some(upload) = read_upload(&path) {
            return Some(upload);
        }
    }
    let (bytes, _) = file.load_bytes(gtk::gio::Cancellable::NONE).ok()?;
    let filename = file
        .basename()
        .and_then(|name| name.into_string().ok())
        .unwrap_or_else(|| "image.jpg".into());
    upload_from_bytes(bytes.as_ref(), &filename)
}

fn upload_from_bytes(data: &[u8], filename: &str) -> Option<Upload> {
    if data.is_empty() || data.len() > 10 * 1024 * 1024 {
        return None;
    }
    Some(Upload {
        mime_type: mime_for(filename).to_owned(),
        filename: filename.to_owned(),
        data: data.to_vec(),
    })
}

pub fn fill_attach_previews(
    host: &gtk::Box,
    images: &[Upload],
    sender: Sender<AppMsg>,
    remote: bool,
) {
    while let Some(child) = host.first_child() {
        host.remove(&child);
    }
    host.set_visible(!images.is_empty());
    for (index, upload) in images.iter().enumerate() {
        let overlay = gtk::Overlay::new();
        overlay.add_css_class("dn-attach-thumb");
        overlay.set_overflow(gtk::Overflow::Hidden);
        overlay.set_size_request(80, 80);
        overlay.set_valign(gtk::Align::Start);
        let picture = gtk::Picture::new();
        picture.set_content_fit(gtk::ContentFit::Cover);
        picture.set_can_shrink(true);
        picture.set_hexpand(true);
        picture.set_vexpand(true);
        picture.set_size_request(80, 80);
        images::bind_bytes(&picture, &upload.data);
        overlay.set_child(Some(&picture));
        let remove = gtk::Button::from_icon_name("dn-x-symbolic");
        pointer(&remove);
        remove.add_css_class("flat");
        remove.add_css_class("circular");
        remove.add_css_class("dn-attach-remove");
        remove.set_halign(gtk::Align::End);
        remove.set_valign(gtk::Align::Start);
        remove.set_margin_top(4);
        remove.set_margin_end(4);
        remove.set_tooltip_text(Some(&format!("Remove image {}", index + 1)));
        let sender = sender.clone();
        remove.connect_clicked(move |_| {
            sender.emit(if remote {
                AppMsg::RemoveRemoteImage(index)
            } else {
                AppMsg::RemoveComposerImage(index)
            });
        });
        overlay.add_overlay(&remove);
        host.append(&overlay);
    }
}

pub fn notify(id: &str, title: &str, body: &str) {
    let notification = gio_notification(title, body);
    relm4::main_application().send_notification(Some(id), &notification);
}

pub fn badge_text(count: i64) -> String {
    if count > 99 {
        "99+".into()
    } else {
        count.to_string()
    }
}

const SOUND_COOLDOWN: Duration = Duration::from_secs(10);
static LAST_SOUND: Mutex<Option<Instant>> = Mutex::new(None);

pub fn play_notification_sound() {
    let mut last = match LAST_SOUND.lock() {
        Ok(guard) => guard,
        Err(_) => return,
    };
    if last.is_some_and(|at| at.elapsed() < SOUND_COOLDOWN) {
        return;
    }
    *last = Some(Instant::now());
    drop(last);
    spawn_notification_sound();
}

fn spawn_notification_sound() {
    if Command::new("canberra-gtk-play")
        .args(["-i", "message-new-instant"])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .is_ok()
    {
        return;
    }
    const PATHS: &[&str] = &[
        "/usr/share/sounds/freedesktop/stereo/message-new-instant.oga",
        "/usr/share/sounds/freedesktop/stereo/message-new-instant.ogg",
        "/usr/share/sounds/freedesktop/stereo/bell.oga",
    ];
    let Some(path) = PATHS
        .iter()
        .copied()
        .find(|candidate| Path::new(candidate).is_file())
    else {
        return;
    };
    for player in ["pw-play", "paplay", "ogg123"] {
        if Command::new(player)
            .arg(path)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .is_ok()
        {
            return;
        }
    }
}

fn gio_notification(title: &str, body: &str) -> gtk::gio::Notification {
    let n = gtk::gio::Notification::new(title);
    n.set_body(Some(body));
    n
}

pub fn clear_list(list: &gtk::ListBox) {
    while let Some(row) = list.row_at_index(0) {
        list.remove(&row);
    }
}

pub fn pin_log_composer(list: &gtk::ListBox, composer: &gtk::Box) {
    unparent(composer);
    let row = gtk::ListBoxRow::new();
    row.set_activatable(false);
    row.set_selectable(false);
    row.add_css_class("dn-commit-row");
    row.add_css_class("dn-log-composer-row");
    row.set_child(Some(composer));
    row.set_visible(composer.is_visible());
    list.prepend(&row);
}

pub fn pin_profile_hero(list: &gtk::ListBox, hero: &gtk::Box) {
    detach_from_list_row(hero);
    hero.set_hexpand(true);
    let row = gtk::ListBoxRow::new();
    row.set_activatable(false);
    row.set_selectable(false);
    row.add_css_class("dn-profile-hero-row");
    row.set_child(Some(hero));
    list.prepend(&row);
}

pub fn reset_profile_list(list: &gtk::ListBox, hero: &gtk::Box) {
    detach_from_list_row(hero);
    clear_list(list);
    pin_profile_hero(list, hero);
}

pub fn detach_from_list_row(widget: &impl IsA<gtk::Widget>) {
    let widget = widget.upcast_ref::<gtk::Widget>();
    let row = {
        let mut parent = widget.parent();
        loop {
            match parent {
                Some(node) => {
                    if let Ok(row) = node.clone().downcast::<gtk::ListBoxRow>() {
                        break Some(row);
                    }
                    parent = node.parent();
                }
                None => break None,
            }
        }
    };
    unparent(widget);
    if let Some(row) = row {
        if let Some(list) = row.parent().and_then(|p| p.downcast::<gtk::ListBox>().ok()) {
            list.remove(&row);
        }
    }
}

pub fn reset_log_list(list: &gtk::ListBox, composer: &gtk::Box) {
    unparent(composer);
    clear_list(list);
    pin_log_composer(list, composer);
}

pub fn append_commits(
    list: &gtk::ListBox,
    commits: &[Commit],
    ctx: &FeedCtx,
    previous_lane: &mut Option<String>,
    last_day: &mut Option<String>,
    first_seen_id: Option<&str>,
    threads: Option<&ThreadState<'_>>,
) {
    let dark = adw_is_dark();
    let gray = if dark { "#36363a" } else { "#d3d7dd" };
    let mut join = ctx.join_staged && previous_lane.is_none();
    for commit in commits {
        if first_seen_id == Some(commit.id.as_str()) {
            list.append(&last_visit_marker(
                if join {
                    Some(gray)
                } else {
                    previous_lane.as_deref()
                },
                ctx.connected || join,
                join,
            ));
        }
        if let Some(created) = commit.created_at {
            let key = day_key(created);
            if last_day.as_ref() != Some(&key) {
                list.append(&day_marker(
                    &day_label(created),
                    if join {
                        Some(gray)
                    } else {
                        previous_lane.as_deref()
                    },
                    ctx.connected || join,
                    join,
                ));
                *last_day = Some(key);
            }
        }
        list.append(&commit_row(
            commit,
            previous_lane.as_deref(),
            ctx,
            CommitChrome {
                connected: ctx.connected,
                terminator: !ctx.connected,
                dashed_in: join,
                ..CommitChrome::default()
            },
        ));
        join = false;
        if let Some(threads) = threads {
            append_open_thread(list, commit, threads, ctx, 0);
        }
        *previous_lane = Some(lane_color(commit.author.as_ref(), dark));
    }
}

pub fn enclosing_scroll(widget: &impl IsA<gtk::Widget>) -> Option<gtk::ScrolledWindow> {
    let mut current = widget.upcast_ref::<gtk::Widget>().parent()?;
    loop {
        if let Ok(scroll) = current.clone().downcast::<gtk::ScrolledWindow>() {
            return Some(scroll);
        }
        current = current.parent()?;
    }
}

pub fn unparent(widget: &impl IsA<gtk::Widget>) {
    let widget = widget.upcast_ref::<gtk::Widget>();
    let Some(parent) = widget.parent() else {
        return;
    };
    if let Ok(box_) = parent.clone().downcast::<gtk::Box>() {
        box_.remove(widget);
    } else if let Ok(row) = parent.clone().downcast::<gtk::ListBoxRow>() {
        row.set_child(None::<&gtk::Widget>);
    } else if let Ok(scroll) = parent.clone().downcast::<gtk::ScrolledWindow>() {
        scroll.set_child(None::<&gtk::Widget>);
    } else {
        widget.unparent();
    }
}

const MAX_POLL_OPTIONS: usize = 4;
const POLL_DURATIONS: &[(i64, &str)] = &[
    (60, "1 hour"),
    (1440, "1 day"),
    (4320, "3 days"),
    (10080, "7 days"),
];
const DEFAULT_POLL_DURATION: u32 = 1;
const MAX_SCHEDULE_DAYS: i64 = 30;

pub struct PollComposer {
    pub root: gtk::Box,
    options: gtk::Box,
    entries: Rc<RefCell<Vec<gtk::Entry>>>,
    duration: gtk::DropDown,
    add: gtk::Button,
}

impl PollComposer {
    pub fn new(sender: Sender<AppMsg>) -> Self {
        let root = gtk::Box::new(gtk::Orientation::Vertical, 0);
        root.add_css_class("dn-poll-editor");
        let options = gtk::Box::new(gtk::Orientation::Vertical, 8);
        let entries = Rc::new(RefCell::new(vec![poll_entry(0), poll_entry(1)]));
        let labels: Vec<&str> = POLL_DURATIONS.iter().map(|(_, label)| *label).collect();
        let duration = gtk::DropDown::from_strings(&labels);
        duration.set_selected(DEFAULT_POLL_DURATION);
        duration.set_valign(gtk::Align::Center);
        let add = gtk::Button::new();
        pointer(&add);
        add.add_css_class("flat");
        add.add_css_class("dn-poll-add");
        add.set_halign(gtk::Align::Start);
        let add_inner = gtk::Box::new(gtk::Orientation::Horizontal, 6);
        add_inner.append(&named_icon("dn-plus-symbolic", 16));
        add_inner.append(&gtk::Label::new(Some("Add option")));
        add.set_child(Some(&add_inner));
        refresh_poll_options(&options, &entries, &add);
        add.connect_clicked({
            let options = options.clone();
            let entries = entries.clone();
            let add = add.clone();
            move |_| {
                let mut list = entries.borrow_mut();
                if list.len() >= MAX_POLL_OPTIONS {
                    return;
                }
                let index = list.len();
                list.push(poll_entry(index));
                drop(list);
                refresh_poll_options(&options, &entries, &add);
                if let Some(entry) = entries.borrow().last() {
                    entry.grab_focus();
                }
            }
        });
        let footer = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        footer.add_css_class("dn-poll-footer");
        let length = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        length.set_valign(gtk::Align::Center);
        length.set_hexpand(true);
        let length_label = gtk::Label::new(Some("Poll length"));
        length_label.add_css_class("dn-muted");
        length.append(&length_label);
        length.append(&duration);
        footer.append(&length);
        let remove = gtk::Button::with_label("Remove poll");
        pointer(&remove);
        remove.add_css_class("flat");
        remove.add_css_class("dn-poll-remove");
        remove.connect_clicked(move |_| sender.emit(AppMsg::RemovePoll));
        footer.append(&remove);
        root.append(&options);
        root.append(&add);
        root.append(&footer);
        Self {
            root,
            options,
            entries,
            duration,
            add,
        }
    }

    pub fn reset(&self) {
        *self.entries.borrow_mut() = vec![poll_entry(0), poll_entry(1)];
        self.duration.set_selected(DEFAULT_POLL_DURATION);
        refresh_poll_options(&self.options, &self.entries, &self.add);
    }

    pub fn draft(&self) -> Option<(Vec<String>, i64)> {
        let options: Vec<String> = self
            .entries
            .borrow()
            .iter()
            .map(|entry| entry.text().trim().to_string())
            .collect();
        if options.len() < 2 || options.iter().any(String::is_empty) {
            return None;
        }
        let duration = POLL_DURATIONS
            .get(self.duration.selected() as usize)
            .map(|(minutes, _)| *minutes)
            .unwrap_or(1440);
        Some((options, duration))
    }
}

fn poll_entry(index: usize) -> gtk::Entry {
    let entry = gtk::Entry::new();
    entry.set_placeholder_text(Some(&format!("Option {}", index + 1)));
    entry.set_max_length(50);
    entry.set_hexpand(true);
    entry
}

fn refresh_poll_options(
    options: &gtk::Box,
    entries: &Rc<RefCell<Vec<gtk::Entry>>>,
    add: &gtk::Button,
) {
    while let Some(child) = options.first_child() {
        options.remove(&child);
    }
    let list = entries.borrow();
    for (index, entry) in list.iter().enumerate() {
        entry.set_placeholder_text(Some(&format!("Option {}", index + 1)));
        let row = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        row.append(entry);
        if index >= 2 {
            let remove = gtk::Button::from_icon_name("dn-x-symbolic");
            pointer(&remove);
            remove.add_css_class("flat");
            remove.add_css_class("dn-composer-icon");
            remove.set_tooltip_text(Some("Remove option"));
            let options = options.clone();
            let entries = entries.clone();
            let add = add.clone();
            remove.connect_clicked(move |_| {
                let mut list = entries.borrow_mut();
                if index < list.len() {
                    list.remove(index);
                }
                drop(list);
                refresh_poll_options(&options, &entries, &add);
            });
            row.append(&remove);
        }
        options.append(&row);
    }
    add.set_visible(list.len() < MAX_POLL_OPTIONS);
}

pub struct ScheduleCard {
    pub root: gtk::Box,
    calendar: gtk::Calendar,
    hour: gtk::DropDown,
    minute: gtk::DropDown,
}

impl ScheduleCard {
    pub fn new() -> Self {
        let root = gtk::Box::new(gtk::Orientation::Vertical, 10);
        root.add_css_class("dn-schedule-card");
        let title = gtk::Label::new(Some("Submit at"));
        title.set_xalign(0.0);
        title.add_css_class("dn-aside-title");
        root.append(&title);
        let calendar = gtk::Calendar::new();
        calendar.set_halign(gtk::Align::Fill);
        root.append(&calendar);
        let time = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        time.set_halign(gtk::Align::Fill);
        let hours: Vec<String> = (0..24).map(|hour| format!("{hour:02}")).collect();
        let hour_refs: Vec<&str> = hours.iter().map(String::as_str).collect();
        let hour = gtk::DropDown::from_strings(&hour_refs);
        hour.set_hexpand(true);
        hour.set_tooltip_text(Some("Hour"));
        let minutes: Vec<String> = (0..12).map(|index| format!("{:02}", index * 5)).collect();
        let minute_refs: Vec<&str> = minutes.iter().map(String::as_str).collect();
        let minute = gtk::DropDown::from_strings(&minute_refs);
        minute.set_hexpand(true);
        minute.set_tooltip_text(Some("Minute"));
        time.append(&hour);
        let colon = gtk::Label::new(Some(":"));
        colon.add_css_class("dn-muted");
        time.append(&colon);
        time.append(&minute);
        root.append(&time);
        let help = gtk::Label::new(Some(&schedule_help()));
        help.add_css_class("dn-muted");
        help.set_wrap(true);
        help.set_xalign(0.0);
        root.append(&help);
        let card = Self {
            root,
            calendar,
            hour,
            minute,
        };
        card.reset();
        card.calendar.connect_day_selected(|calendar| {
            clamp_calendar(calendar);
        });
        card
    }

    pub fn reset(&self) {
        let now = Local::now();
        let start = now.date_naive();
        select_calendar(&self.calendar, start);
        let next_hour = (now.hour() + 1).min(23);
        self.hour.set_selected(next_hour);
        self.minute.set_selected(0);
    }

    pub fn rfc3339(&self) -> Result<String, &'static str> {
        clamp_calendar(&self.calendar);
        let Some(date) = calendar_date(&self.calendar) else {
            return Err("fatal: pick a date and time");
        };
        let hour = self.hour.selected();
        let minute = self.minute.selected().saturating_mul(5);
        if hour > 23 || minute > 55 {
            return Err("fatal: pick a date and time");
        }
        let Some(naive) = date.and_hms_opt(hour, minute, 0) else {
            return Err("fatal: pick a date and time");
        };
        let Some(local) = Local.from_local_datetime(&naive).single() else {
            return Err("fatal: that time does not exist in your timezone");
        };
        let now = Local::now();
        if local <= now {
            return Err("fatal: pick a time in the future");
        }
        let last = now + ChronoDuration::days(MAX_SCHEDULE_DAYS);
        if local > last {
            return Err("fatal: schedule within 30 days");
        }
        Ok(local.to_rfc3339())
    }
}

fn schedule_help() -> String {
    let zone = local_timezone().unwrap_or_else(|| "local time".into());
    format!(
        "{zone}. Within 30 days. Private until submitted; then submitted for review, not published automatically."
    )
}

fn calendar_date(calendar: &gtk::Calendar) -> Option<NaiveDate> {
    let date = calendar.date();
    NaiveDate::from_ymd_opt(
        date.year(),
        u32::try_from(date.month()).ok()?,
        u32::try_from(date.day_of_month()).ok()?,
    )
}

fn select_calendar(calendar: &gtk::Calendar, date: NaiveDate) {
    if let Ok(dt) = glib::DateTime::from_local(
        date.year(),
        date.month() as i32,
        date.day() as i32,
        12,
        0,
        0.0,
    ) {
        calendar.select_day(&dt);
    }
}

fn clamp_calendar(calendar: &gtk::Calendar) {
    let today = Local::now().date_naive();
    let last = today + ChronoDuration::days(MAX_SCHEDULE_DAYS);
    let Some(date) = calendar_date(calendar) else {
        return;
    };
    if date < today {
        select_calendar(calendar, today);
    } else if date > last {
        select_calendar(calendar, last);
    }
}

pub fn inline_reply_box(view: &gtk::TextView, sender: Sender<AppMsg>) -> gtk::Box {
    let box_ = gtk::Box::new(gtk::Orientation::Vertical, 8);
    box_.add_css_class("dn-inline-reply");
    box_.add_css_class("dn-composer");
    let scroll = gtk::ScrolledWindow::new();
    scroll.set_min_content_height(56);
    scroll.set_max_content_height(120);
    scroll.set_hexpand(true);
    scroll.add_css_class("dn-composer-input");
    scroll.set_child(Some(view));
    box_.append(&scroll);
    let actions = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    actions.set_halign(gtk::Align::End);
    let cancel = gtk::Button::with_label("Cancel");
    cancel.add_css_class("flat");
    pointer(&cancel);
    let cancel_sender = sender.clone();
    cancel.connect_clicked(move |_| cancel_sender.emit(AppMsg::CancelReply));
    actions.append(&cancel);
    let submit = gtk::Button::with_label("Reply");
    submit.add_css_class("suggested-action");
    pointer(&submit);
    submit.connect_clicked(move |_| sender.emit(AppMsg::SubmitReply));
    actions.append(&submit);
    box_.append(&actions);
    box_
}

fn reply_composer_row(composer: &gtk::Box, depth: u8) -> gtk::ListBoxRow {
    unparent(composer);
    let row = gtk::ListBoxRow::new();
    row.set_activatable(false);
    row.add_css_class("dn-commit-row");
    row.set_margin_start(i32::from(depth.min(3)) * 28);
    let outer = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    let spacer = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    spacer.set_size_request(RAIL_WIDTH, 1);
    outer.append(&spacer);
    composer.set_hexpand(true);
    outer.append(composer);
    row.set_child(Some(&outer));
    row
}

fn append_open_thread(
    list: &gtk::ListBox,
    commit: &Commit,
    threads: &ThreadState<'_>,
    ctx: &FeedCtx,
    depth: u8,
) {
    if threads.expanded.contains(&commit.id) {
        match threads.nested.get(&commit.id) {
            Some(children) => {
                if !children.is_empty() {
                    append_nested(list, children, threads, ctx, depth.saturating_add(1));
                }
            }
            None => list.append(&reply_skeleton_row()),
        }
    }
    if threads.replying_to == Some(commit.id.as_str()) {
        list.append(&reply_composer_row(threads.composer, depth));
    }
}

pub fn append_thread(
    list: &gtk::ListBox,
    ancestors: &[Commit],
    commit: &Commit,
    replies: &[Commit],
    threads: &ThreadState<'_>,
    ctx: &FeedCtx,
) {
    let dark = adw_is_dark();
    let mut previous = None;
    let mut previous_author: Option<String> = None;
    for ancestor in ancestors {
        let grouped = same_author(previous_author.as_deref(), ancestor);
        list.append(&commit_row(
            ancestor,
            previous.as_deref(),
            ctx,
            CommitChrome {
                connected: true,
                terminator: false,
                grouped,
                ..CommitChrome::default()
            },
        ));
        previous = Some(lane_color(ancestor.author.as_ref(), dark));
        previous_author = ancestor.author.as_ref().map(|a| a.id.clone());
    }
    list.append(&commit_row(
        commit,
        previous.as_deref(),
        ctx,
        CommitChrome {
            focused: true,
            last: replies.is_empty(),
            depth: 0,
            connected: true,
            terminator: true,
            dashed_in: false,
            grouped: same_author(previous_author.as_deref(), commit),
        },
    ));
    if threads.replying_to == Some(commit.id.as_str()) {
        list.append(&reply_composer_row(threads.composer, 0));
    }
    append_nested(list, replies, threads, ctx, 1);
}

fn same_author(previous_id: Option<&str>, commit: &Commit) -> bool {
    previous_id
        .zip(commit.author.as_ref())
        .is_some_and(|(id, author)| id == author.id)
}

fn append_nested(
    list: &gtk::ListBox,
    replies: &[Commit],
    threads: &ThreadState<'_>,
    ctx: &FeedCtx,
    depth: u8,
) {
    let dark = adw_is_dark();
    let mut previous = None;
    for (index, reply) in replies.iter().enumerate() {
        let last = index + 1 == replies.len();
        list.append(&commit_row(
            reply,
            previous.as_deref(),
            ctx,
            CommitChrome {
                depth,
                focused: false,
                last,
                grouped: false,
                connected: true,
                terminator: last,
                dashed_in: false,
            },
        ));
        previous = Some(lane_color(reply.author.as_ref(), dark));
        append_open_thread(list, reply, threads, ctx, depth);
    }
}

pub fn append_commit_skeletons(list: &gtk::ListBox, count: usize) {
    for index in 0..count {
        list.append(&commit_skeleton_row(index > 0));
    }
}

pub fn append_notification_skeletons(list: &gtk::ListBox, count: usize) {
    for index in 0..count {
        list.append(&notification_skeleton_row(index > 0));
    }
}

pub fn append_inbox_skeletons(list: &gtk::ListBox, count: usize) {
    for _ in 0..count {
        list.append(&inbox_skeleton_row());
    }
}

pub fn append_thread_skeletons(list: &gtk::ListBox) {
    list.append(&thread_bubble_row(192, false));
    list.append(&thread_bubble_row(256, true));
    list.append(&thread_bubble_row(160, true));
    list.append(&thread_bubble_row(224, false));
}

pub fn content_skeleton_row() -> gtk::ListBoxRow {
    let row = gtk::ListBoxRow::new();
    row.set_activatable(false);
    row.add_css_class("dn-commit-row");
    let box_ = gtk::Box::new(gtk::Orientation::Vertical, 10);
    box_.add_css_class("dn-skeleton");
    box_.set_hexpand(true);
    box_.set_margin_start(16);
    box_.set_margin_end(16);
    box_.set_margin_top(16);
    box_.set_margin_bottom(16);
    box_.append(&skeleton_bars(&[0.7, 1.0, 0.9, 0.55], 10, 8));
    row.set_child(Some(&box_));
    row
}

pub fn fill_suggestion_skeletons(box_: &gtk::Box) {
    while let Some(child) = box_.first_child() {
        box_.remove(&child);
    }
    for _ in 0..3 {
        box_.append(&suggestion_skeleton_row());
    }
}

fn commit_skeleton_row(has_previous: bool) -> gtk::ListBoxRow {
    let row = gtk::ListBoxRow::new();
    row.set_activatable(false);
    row.add_css_class("dn-commit-row");
    let outer = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    outer.add_css_class("dn-skeleton");
    outer.set_hexpand(true);
    outer.append(&loading_rail(has_previous));
    let content = gtk::Box::new(gtk::Orientation::Vertical, 8);
    content.add_css_class("dn-commit-content");
    content.set_hexpand(true);
    content.append(&skeleton_bars(&[0.4, 0.8, 0.6], 12, 10));
    content.append(&bone(36, 16));
    outer.append(&content);
    row.set_child(Some(&outer));
    row
}

fn notification_skeleton_row(has_previous: bool) -> gtk::ListBoxRow {
    let row = gtk::ListBoxRow::new();
    row.set_activatable(false);
    row.add_css_class("dn-commit-row");
    let outer = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    outer.add_css_class("dn-skeleton");
    outer.set_hexpand(true);
    outer.append(&loading_rail(has_previous));
    let content = gtk::Box::new(gtk::Orientation::Vertical, 8);
    content.add_css_class("dn-commit-content");
    content.set_hexpand(true);
    content.append(&bone_circle(32));
    content.append(&skeleton_bars(&[0.5, 0.8], 12, 10));
    outer.append(&content);
    row.set_child(Some(&outer));
    row
}

fn reply_skeleton_row() -> gtk::ListBoxRow {
    let row = gtk::ListBoxRow::new();
    row.set_activatable(false);
    row.add_css_class("dn-commit-row");
    let outer = gtk::Box::new(gtk::Orientation::Horizontal, 10);
    outer.add_css_class("dn-skeleton");
    outer.set_hexpand(true);
    outer.set_margin_start(28);
    outer.set_margin_end(16);
    outer.set_margin_top(8);
    outer.set_margin_bottom(8);
    let node = bone_circle(24);
    node.set_halign(gtk::Align::Start);
    node.set_valign(gtk::Align::Start);
    outer.append(&node);
    let content = gtk::Box::new(gtk::Orientation::Vertical, 6);
    content.set_hexpand(true);
    content.append(&skeleton_bars(&[0.33, 0.75, 0.28], 12, 8));
    outer.append(&content);
    row.set_child(Some(&outer));
    row
}

fn inbox_skeleton_row() -> gtk::ListBoxRow {
    let row = gtk::ListBoxRow::new();
    row.set_activatable(false);
    row.add_css_class("dn-inbox-row");
    let box_ = gtk::Box::new(gtk::Orientation::Horizontal, 10);
    box_.add_css_class("dn-skeleton");
    box_.set_hexpand(true);
    let avatar = bone_circle(36);
    avatar.set_halign(gtk::Align::Start);
    box_.append(&avatar);
    let text = gtk::Box::new(gtk::Orientation::Vertical, 8);
    text.set_hexpand(true);
    text.append(&skeleton_bars(&[0.45, 0.7], 12, 8));
    box_.append(&text);
    row.set_child(Some(&box_));
    row
}

fn thread_bubble_row(width: i32, mine: bool) -> gtk::ListBoxRow {
    let row = gtk::ListBoxRow::new();
    row.set_activatable(false);
    let box_ = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    box_.add_css_class("dn-skeleton");
    box_.set_hexpand(true);
    box_.set_margin_start(12);
    box_.set_margin_end(12);
    box_.set_margin_top(4);
    box_.set_margin_bottom(4);
    if mine {
        box_.append(&hexpand_spacer());
    }
    let bubble = bone(width, 36);
    bubble.add_css_class("dn-skeleton-bubble");
    box_.append(&bubble);
    if !mine {
        box_.append(&hexpand_spacer());
    }
    row.set_child(Some(&box_));
    row
}

fn suggestion_skeleton_row() -> gtk::Box {
    let row = gtk::Box::new(gtk::Orientation::Horizontal, 10);
    row.add_css_class("dn-suggest-row");
    row.add_css_class("dn-skeleton");
    row.set_overflow(gtk::Overflow::Visible);
    let avatar = bone_circle(36);
    avatar.set_halign(gtk::Align::Start);
    row.append(&avatar);
    let col = gtk::Box::new(gtk::Orientation::Vertical, 6);
    col.set_hexpand(true);
    col.append(&skeleton_bars(&[0.55, 0.4], 12, 6));
    row.append(&col);
    row.append(&bone(32, 32));
    row
}

fn loading_rail(has_previous: bool) -> gtk::DrawingArea {
    let gray = if adw_is_dark() { "#36363a" } else { "#d3d7dd" };
    rail(
        gray,
        has_previous.then_some(gray),
        RailKind::Commit {
            incoming: has_previous,
            terminator: false,
            dashed_in: false,
        },
    )
}

fn skeleton_bars(fracs: &[f32], height: i32, gap: i32) -> gtk::DrawingArea {
    let area = gtk::DrawingArea::new();
    area.add_css_class("dn-skeleton-bars");
    area.add_css_class("dn-skeleton-pulse");
    area.set_hexpand(true);
    area.set_halign(gtk::Align::Fill);
    let n = fracs.len().max(1) as i32;
    area.set_content_height(n * height + (n - 1) * gap);
    let fracs = fracs.to_vec();
    area.set_draw_func(move |_, cairo, width, _height| {
        let (wr, wg, wb) = wash_rgb(adw_is_dark());
        cairo.set_source_rgb(wr, wg, wb);
        let mut y = 0.0;
        for &frac in &fracs {
            let w = (f64::from(width) * f64::from(frac)).max(24.0);
            rounded_cell(cairo, 0.0, y, w, f64::from(height), 4.0);
            let _ = cairo.fill();
            y += f64::from(height + gap);
        }
    });
    area
}

fn bone(width: i32, height: i32) -> gtk::Box {
    let bone = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    bone.add_css_class("dn-bone");
    bone.add_css_class("dn-skeleton-pulse");
    bone.set_size_request(width, height);
    bone.set_hexpand(false);
    bone.set_halign(gtk::Align::Start);
    bone
}

fn bone_circle(size: i32) -> gtk::Box {
    let bone = bone(size, size);
    bone.add_css_class("dn-bone-circle");
    bone
}

fn hexpand_spacer() -> gtk::Box {
    let spacer = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    spacer.set_hexpand(true);
    spacer
}

fn wash_rgb(dark: bool) -> (f64, f64, f64) {
    if dark {
        (0.14, 0.14, 0.15)
    } else {
        (0.93, 0.94, 0.95)
    }
}

pub fn quote_window(commit: &Commit, sender: Sender<AppMsg>) -> gtk::Window {
    let win = gtk::Window::new();
    win.set_title(Some(&format!(
        "Quote @{}",
        commit
            .author
            .as_ref()
            .map(|a| a.username.as_str())
            .unwrap_or("commit")
    )));
    win.set_default_width(520);
    win.set_modal(true);
    let outer = gtk::Box::new(gtk::Orientation::Vertical, 12);
    outer.set_margin_top(16);
    outer.set_margin_bottom(16);
    outer.set_margin_start(16);
    outer.set_margin_end(16);
    let intro = gtk::Label::new(Some(
        "Write a commit of your own. The quoted commit is embedded below it.",
    ));
    intro.set_wrap(true);
    intro.set_xalign(0.0);
    intro.add_css_class("dn-muted");
    outer.append(&intro);
    let view = gtk::TextView::new();
    view.set_wrap_mode(gtk::WrapMode::WordChar);
    view.add_css_class("dn-composer-text");
    view.set_hexpand(true);
    view.set_height_request(80);
    outer.append(&view);
    outer.append(&quoted_commit_widget(Some(commit), sender.clone()));
    let row = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    row.set_halign(gtk::Align::End);
    let cancel = gtk::Button::with_label("Cancel");
    cancel.add_css_class("flat");
    pointer(&cancel);
    let win_cancel = win.clone();
    cancel.connect_clicked(move |_| win_cancel.close());
    row.append(&cancel);
    let submit = gtk::Button::with_label("Quote");
    submit.add_css_class("suggested-action");
    pointer(&submit);
    let id = commit.id.clone();
    let win_submit = win.clone();
    submit.connect_clicked(move |_| {
        let buf = view.buffer();
        let body = buf
            .text(&buf.start_iter(), &buf.end_iter(), false)
            .to_string();
        sender.emit(AppMsg::SubmitQuote(id.clone(), body));
        win_submit.close();
    });
    row.append(&submit);
    outer.append(&row);
    win.set_child(Some(&outer));
    win
}

const REPORT_REASONS: &[(&str, &str)] = &[
    ("spam", "Spam or misleading links"),
    ("harassment", "Harassment or hate"),
    ("unsafe", "Harmful or private information"),
    ("other", "Something else"),
];

pub fn report_window(commit: &Commit, sender: Sender<AppMsg>) -> gtk::Window {
    let win = gtk::Window::new();
    win.set_title(Some("Report commit"));
    win.set_default_width(480);
    win.set_modal(true);
    let outer = gtk::Box::new(gtk::Orientation::Vertical, 12);
    outer.set_margin_top(16);
    outer.set_margin_bottom(16);
    outer.set_margin_start(16);
    outer.set_margin_end(16);
    let intro = gtk::Label::new(Some(
        "Your report is private. Only moderators can review it. Reporting does not hide this commit.",
    ));
    intro.set_wrap(true);
    intro.set_xalign(0.0);
    intro.add_css_class("dn-muted");
    outer.append(&intro);
    if let Some(body) = commit.body.as_deref().filter(|body| !body.is_empty()) {
        let excerpt = gtk::Label::new(Some(body));
        excerpt.set_wrap(true);
        excerpt.set_xalign(0.0);
        excerpt.set_max_width_chars(60);
        excerpt.set_lines(3);
        excerpt.set_ellipsize(pango::EllipsizeMode::End);
        excerpt.add_css_class("dn-report-excerpt");
        outer.append(&excerpt);
    }
    let legend = gtk::Label::new(Some("Why are you reporting this commit?"));
    legend.set_xalign(0.0);
    legend.add_css_class("dn-name");
    outer.append(&legend);
    let reason = Rc::new(RefCell::new(String::new()));
    let mut group: Option<gtk::CheckButton> = None;
    for (value, label) in REPORT_REASONS {
        let radio = gtk::CheckButton::with_label(label);
        radio.add_css_class("dn-report-reason");
        pointer(&radio);
        if let Some(leader) = &group {
            radio.set_group(Some(leader));
        } else {
            group = Some(radio.clone());
        }
        let chosen = reason.clone();
        let key = (*value).to_owned();
        radio.connect_toggled(move |btn| {
            if btn.is_active() {
                *chosen.borrow_mut() = key.clone();
            }
        });
        outer.append(&radio);
    }
    let details_label = gtk::Label::new(Some("Details (optional)"));
    details_label.set_xalign(0.0);
    outer.append(&details_label);
    let details = gtk::TextView::new();
    details.set_wrap_mode(gtk::WrapMode::WordChar);
    details.add_css_class("dn-composer-text");
    details.set_hexpand(true);
    details.set_height_request(72);
    outer.append(&details);
    let hint = gtk::Label::new(Some("Add context for moderators. Up to 500 characters."));
    hint.add_css_class("dn-muted");
    hint.set_xalign(0.0);
    outer.append(&hint);
    let row = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    row.set_halign(gtk::Align::End);
    let cancel = gtk::Button::with_label("Cancel");
    cancel.add_css_class("flat");
    pointer(&cancel);
    let win_cancel = win.clone();
    cancel.connect_clicked(move |_| win_cancel.close());
    row.append(&cancel);
    let submit = gtk::Button::with_label("Send report");
    submit.add_css_class("suggested-action");
    pointer(&submit);
    let id = commit.id.clone();
    let win_submit = win.clone();
    let chosen = reason.clone();
    submit.connect_clicked(move |_| {
        let reason = chosen.borrow().clone();
        if reason.is_empty() {
            return;
        }
        let buf = details.buffer();
        let details = buf
            .text(&buf.start_iter(), &buf.end_iter(), false)
            .to_string();
        sender.emit(AppMsg::SubmitReport {
            id: id.clone(),
            reason,
            details,
        });
        win_submit.close();
    });
    row.append(&submit);
    outer.append(&row);
    win.set_child(Some(&outer));
    win
}

pub fn compose_dialog() -> (adw::Dialog, gtk::Box) {
    let dialog = adw::Dialog::new();
    dialog.set_title("Write a commit");
    dialog.set_content_width(576);
    dialog.set_follows_content_size(true);
    dialog.add_css_class("dn-compose-dialog");

    let toolbar = adw::ToolbarView::new();
    toolbar.add_top_bar(&adw::HeaderBar::new());
    let host = gtk::Box::new(gtk::Orientation::Vertical, 0);
    host.set_hexpand(true);
    toolbar.set_content(Some(&host));
    dialog.set_child(Some(&toolbar));
    (dialog, host)
}

pub fn shortcuts_dialog() -> adw::Dialog {
    let dialog = adw::Dialog::new();
    dialog.set_title("Keyboard shortcuts");
    dialog.set_content_width(384);
    dialog.set_content_height(560);
    dialog.set_follows_content_size(false);

    let toolbar = adw::ToolbarView::new();
    toolbar.add_top_bar(&adw::HeaderBar::new());

    let scroll = gtk::ScrolledWindow::new();
    scroll.set_policy(gtk::PolicyType::Never, gtk::PolicyType::Automatic);
    scroll.set_vexpand(true);
    let outer = gtk::Box::new(gtk::Orientation::Vertical, 8);
    outer.add_css_class("dn-shortcuts");
    outer.set_margin_top(8);
    outer.set_margin_bottom(16);
    outer.set_margin_start(16);
    outer.set_margin_end(16);
    let intro = gtk::Label::new(Some("They work anywhere except while you're typing."));
    intro.set_wrap(true);
    intro.set_xalign(0.0);
    intro.add_css_class("dn-muted");
    outer.append(&intro);
    for (keys, label) in SHORTCUTS {
        outer.append(&shortcut_row(keys, label));
    }
    scroll.set_child(Some(&outer));
    toolbar.set_content(Some(&scroll));
    dialog.set_child(Some(&toolbar));
    dialog
}

const SHORTCUTS: &[(&[&str], &str)] = &[
    (&["j"], "Next commit"),
    (&["k"], "Previous commit"),
    (&["y"], "Copy link to the focused commit"),
    (&["↵"], "Open the focused commit"),
    (&["r"], "Reply to commit"),
    (&["q"], "Quote commit"),
    (&["o"], "Show or hide replies"),
    (&["c"], "Write a commit"),
    (&["Ctrl", "K"], "Search developers and commits"),
    (&["/"], "Search developers and commits"),
    (&["Ctrl", "↵"], "Commit (in the composer)"),
    (&["Ctrl", "Shift", "P"], "Preview (in the composer)"),
    (&["g", "l"], "Go to your log"),
    (&["g", "e"], "Go to trending"),
    (&["g", "p"], "Go to your profile"),
    (&["g", "s"], "Go to settings"),
    (&["?"], "Show this list"),
];

fn shortcut_row(keys: &[&str], label: &str) -> gtk::Widget {
    let row = gtk::Box::new(gtk::Orientation::Horizontal, 12);
    row.add_css_class("dn-shortcut");
    let text = gtk::Label::new(Some(label));
    text.set_xalign(0.0);
    text.set_hexpand(true);
    row.append(&text);
    let keys_box = gtk::Box::new(gtk::Orientation::Horizontal, 4);
    for key in keys {
        let kbd = gtk::Label::new(Some(key));
        kbd.add_css_class("dn-kbd");
        kbd.set_valign(gtk::Align::Center);
        keys_box.append(&kbd);
    }
    row.append(&keys_box);
    row.upcast()
}

const SUGGESTED_QUERIES: &[&str] = &[
    "shipping a side project",
    "debugging in production",
    "testing strategies that actually work",
    "learning a new programming language",
    "speeding up a slow app",
    "working with AI coding tools",
];

pub struct SearchOmnibox {
    pub window: gtk::Window,
    pub entry: gtk::Entry,
    pub status: gtk::Label,
    pub list: gtk::ListBox,
    sender: Sender<AppMsg>,
}

impl SearchOmnibox {
    pub fn new(sender: Sender<AppMsg>) -> Self {
        let window = gtk::Window::new();
        window.set_title(Some("Search"));
        window.set_modal(true);
        window.set_default_width(576);
        window.set_hide_on_close(true);
        window.add_css_class("dn-omnibox");

        let root = gtk::Box::new(gtk::Orientation::Vertical, 0);
        let header = gtk::Box::new(gtk::Orientation::Horizontal, 12);
        header.add_css_class("dn-omnibox-header");
        header.append(&named_icon("dn-search-symbolic", 20));
        let entry = gtk::Entry::new();
        entry.set_placeholder_text(Some("Search developers or commits"));
        entry.set_hexpand(true);
        entry.set_max_length(100);
        entry.add_css_class("dn-omnibox-entry");
        header.append(&entry);
        let esc = gtk::Label::new(Some("esc"));
        esc.add_css_class("dn-kbd");
        esc.set_valign(gtk::Align::Center);
        header.append(&esc);
        root.append(&header);

        let status = gtk::Label::new(None);
        status.add_css_class("dn-omnibox-status");
        status.add_css_class("dn-mono");
        status.add_css_class("dn-muted");
        status.set_xalign(0.0);
        status.set_visible(false);
        root.append(&status);

        let scroll = gtk::ScrolledWindow::new();
        scroll.set_policy(gtk::PolicyType::Never, gtk::PolicyType::Automatic);
        scroll.set_min_content_height(80);
        scroll.set_max_content_height(420);
        scroll.set_propagate_natural_height(true);
        let list = gtk::ListBox::new();
        list.set_selection_mode(gtk::SelectionMode::Browse);
        list.add_css_class("dn-omnibox-list");
        scroll.set_child(Some(&list));
        root.append(&scroll);

        let footer = gtk::Box::new(gtk::Orientation::Horizontal, 16);
        footer.add_css_class("dn-omnibox-footer");
        footer.append(&hint("↑↓", "to move"));
        footer.append(&hint("↵", "to open"));
        footer.append(&hint("@", "for developers"));
        let anywhere = hint("Ctrl+K", "anywhere");
        anywhere.set_hexpand(true);
        anywhere.set_halign(gtk::Align::End);
        footer.append(&anywhere);
        root.append(&footer);

        window.set_child(Some(&root));

        let keys = gtk::EventControllerKey::new();
        let win_keys = window.clone();
        let list_keys = list.clone();
        let entry_keys = entry.clone();
        let sender_keys = sender.clone();
        keys.connect_key_pressed(move |_, key, _, _| {
            if key == gtk::gdk::Key::Escape {
                win_keys.set_visible(false);
                return gtk::glib::Propagation::Stop;
            }
            if key == gtk::gdk::Key::Down {
                move_omnibox_selection(&list_keys, 1);
                return gtk::glib::Propagation::Stop;
            }
            if key == gtk::gdk::Key::Up {
                move_omnibox_selection(&list_keys, -1);
                return gtk::glib::Propagation::Stop;
            }
            if key == gtk::gdk::Key::Return || key == gtk::gdk::Key::KP_Enter {
                if let Some(row) = list_keys.selected_row() {
                    row.activate();
                    return gtk::glib::Propagation::Stop;
                }
                let q = entry_keys.text().trim().to_string();
                if !q.is_empty() && q != "@" {
                    win_keys.set_visible(false);
                    sender_keys.emit(AppMsg::RunSearch(q));
                    return gtk::glib::Propagation::Stop;
                }
            }
            gtk::glib::Propagation::Proceed
        });
        entry.add_controller(keys);

        entry.connect_changed({
            let sender = sender.clone();
            move |entry| sender.emit(AppMsg::OmniboxQuery(entry.text().to_string()))
        });

        Self {
            window,
            entry,
            status,
            list,
            sender,
        }
    }

    pub fn open(&self) {
        if adw_is_dark() {
            self.window.add_css_class("dark");
        } else {
            self.window.remove_css_class("dark");
        }
        self.entry.set_text("");
        self.set_status(None);
        self.show_suggestions();
        self.window.present();
        self.entry.grab_focus();
    }

    pub fn close(&self) {
        self.window.set_visible(false);
    }

    pub fn set_status(&self, text: Option<&str>) {
        match text {
            Some(text) => {
                self.status.set_label(text);
                self.status.set_visible(true);
            }
            None => self.status.set_visible(false),
        }
    }

    pub fn show_skeletons(&self) {
        self.set_status(None);
        clear_list(&self.list);
        for _ in 0..3 {
            self.list.append(&omnibox_skeleton_row());
        }
    }

    pub fn show_suggestions(&self) {
        clear_list(&self.list);
        self.list.append(&omnibox_heading_row(
            Some("dn-sparkles-symbolic"),
            "Try searching for…",
        ));
        for term in SUGGESTED_QUERIES {
            let row = gtk::ListBoxRow::new();
            row.set_activatable(true);
            let inner = gtk::Box::new(gtk::Orientation::Horizontal, 8);
            inner.add_css_class("dn-omnibox-row");
            let label = gtk::Label::new(Some(term));
            label.set_xalign(0.0);
            label.set_hexpand(true);
            label.set_ellipsize(pango::EllipsizeMode::End);
            inner.append(&label);
            inner.append(&named_icon("dn-arrow-right-symbolic", 16));
            row.set_child(Some(&inner));
            let entry = self.entry.clone();
            let picked = (*term).to_string();
            row.connect_activate(move |_| {
                entry.set_text(&picked);
                entry.set_position(-1);
            });
            self.list.append(&row);
        }
        select_first_activatable(&self.list);
    }

    pub fn show_results(&self, term: &str, mut results: SearchResults) {
        let intent = search_intent(term);
        if intent == Intent::Developer {
            results.commits.clear();
        } else {
            results.commits.truncate(5);
        }
        let developers: Vec<User> = if intent == Intent::Topic {
            results.users.into_iter().take(2).collect()
        } else {
            results.users
        };
        let see_all = intent != Intent::Developer;
        if developers.is_empty() && results.commits.is_empty() && !see_all {
            self.set_status(Some(&format!("fatal: nothing matches '{term}'")));
            self.show_suggestions();
            return;
        }
        self.set_status(None);
        clear_list(&self.list);
        if intent == Intent::Topic {
            self.append_commits(&results.commits);
            if see_all {
                self.append_see_all(term);
            }
            self.append_developers(&developers);
        } else {
            self.append_developers(&developers);
            self.append_commits(&results.commits);
            if see_all {
                self.append_see_all(term);
            }
        }
        select_first_activatable(&self.list);
    }

    fn append_developers(&self, users: &[User]) {
        if users.is_empty() {
            return;
        }
        self.list.append(&omnibox_heading_row(None, "Developers"));
        for user in users {
            let row = gtk::ListBoxRow::new();
            row.set_activatable(true);
            let inner = gtk::Box::new(gtk::Orientation::Horizontal, 12);
            inner.add_css_class("dn-omnibox-row");
            inner.append(&avatar_widget(Some(user), 36, false));
            let col = gtk::Box::new(gtk::Orientation::Vertical, 2);
            let names = gtk::Box::new(gtk::Orientation::Horizontal, 6);
            let name = gtk::Label::new(Some(user.display_name()));
            name.add_css_class("dn-name");
            name.set_ellipsize(pango::EllipsizeMode::End);
            names.append(&name);
            names.append(&handle_dot(Some(user)));
            let handle = gtk::Label::new(Some(&format!("@{}", user.username)));
            handle.add_css_class("dn-handle");
            names.append(&handle);
            col.append(&names);
            if let Some(bio) = user.bio.as_deref().filter(|s| !s.is_empty()) {
                let bio_l = gtk::Label::new(Some(bio));
                bio_l.add_css_class("dn-muted");
                bio_l.set_xalign(0.0);
                bio_l.set_ellipsize(pango::EllipsizeMode::End);
                col.append(&bio_l);
            }
            inner.append(&col);
            row.set_child(Some(&inner));
            let username = user.username.clone();
            let sender = self.sender.clone();
            let window = self.window.clone();
            row.connect_activate(move |_| {
                window.set_visible(false);
                sender.emit(AppMsg::OpenProfile(username.clone()));
            });
            self.list.append(&row);
        }
    }

    fn append_commits(&self, commits: &[Commit]) {
        if commits.is_empty() {
            return;
        }
        self.list
            .append(&omnibox_heading_row(None, "Most relevant commits"));
        for commit in commits {
            let row = gtk::ListBoxRow::new();
            row.set_activatable(true);
            let inner = gtk::Box::new(gtk::Orientation::Horizontal, 12);
            inner.add_css_class("dn-omnibox-row");
            inner.append(&avatar_widget(commit.author.as_ref(), 36, false));
            let col = gtk::Box::new(gtk::Orientation::Vertical, 2);
            let meta = gtk::Box::new(gtk::Orientation::Horizontal, 6);
            if let Some(author) = &commit.author {
                meta.append(&handle_dot(Some(author)));
                let handle = gtk::Label::new(Some(&format!("@{}", author.username)));
                handle.add_css_class("dn-handle");
                meta.append(&handle);
            }
            let time = gtk::Label::new(Some(&relative_time(commit.created_at)));
            time.add_css_class("dn-mono");
            time.add_css_class("dn-muted");
            meta.append(&time);
            col.append(&meta);
            if let Some(body) = commit.body.as_deref().filter(|s| !s.is_empty()) {
                let body_l = gtk::Label::new(Some(body));
                body_l.set_xalign(0.0);
                body_l.set_wrap(true);
                body_l.set_lines(2);
                body_l.set_ellipsize(pango::EllipsizeMode::End);
                col.append(&body_l);
            }
            inner.append(&col);
            row.set_child(Some(&inner));
            let id = commit.id.clone();
            let sender = self.sender.clone();
            let window = self.window.clone();
            row.connect_activate(move |_| {
                window.set_visible(false);
                sender.emit(AppMsg::OpenCommit(id.clone()));
            });
            self.list.append(&row);
        }
    }

    fn append_see_all(&self, term: &str) {
        let row = gtk::ListBoxRow::new();
        row.set_activatable(true);
        let inner = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        inner.add_css_class("dn-omnibox-row");
        let label = gtk::Label::new(Some("See all matching commits"));
        label.add_css_class("dn-omnibox-all");
        label.set_xalign(0.0);
        label.set_hexpand(true);
        inner.append(&label);
        inner.append(&named_icon("dn-arrow-right-symbolic", 16));
        row.set_child(Some(&inner));
        let q = term.to_string();
        let sender = self.sender.clone();
        let window = self.window.clone();
        row.connect_activate(move |_| {
            window.set_visible(false);
            sender.emit(AppMsg::RunSearch(q.clone()));
        });
        self.list.append(&row);
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Intent {
    Developer,
    Either,
    Topic,
}

fn search_intent(term: &str) -> Intent {
    let trimmed = term.trim();
    if trimmed.starts_with('@') {
        return Intent::Developer;
    }
    let words: Vec<&str> = trimmed.split_whitespace().collect();
    if words.len() >= 2 && words.len() <= 3 && words.iter().all(|word| looks_like_name_word(word)) {
        return Intent::Either;
    }
    if words.len() > 1 {
        return Intent::Topic;
    }
    if looks_like_handle(trimmed) {
        return Intent::Developer;
    }
    Intent::Either
}

fn looks_like_name_word(word: &str) -> bool {
    let mut chars = word.chars();
    let Some(first) = chars.next() else {
        return false;
    };
    first.is_uppercase() && chars.all(|c| c.is_alphabetic() || c == '\'' || c == '-')
}

fn looks_like_handle(word: &str) -> bool {
    word.len() <= 12
        && word.chars().all(|c| c.is_ascii_alphanumeric() || c == '-')
        && !is_dictionary_shaped(word)
}

fn is_dictionary_shaped(word: &str) -> bool {
    word.len() >= 6 && word.chars().all(|c| c.is_ascii_lowercase())
}

fn hint(key: &str, label: &str) -> gtk::Box {
    let row = gtk::Box::new(gtk::Orientation::Horizontal, 6);
    let kbd = gtk::Label::new(Some(key));
    kbd.add_css_class("dn-kbd");
    kbd.set_valign(gtk::Align::Center);
    row.append(&kbd);
    let text = gtk::Label::new(Some(label));
    text.add_css_class("dn-muted");
    row.append(&text);
    row
}

fn omnibox_skeleton_row() -> gtk::ListBoxRow {
    let row = gtk::ListBoxRow::new();
    row.set_activatable(false);
    row.set_selectable(false);
    let inner = gtk::Box::new(gtk::Orientation::Horizontal, 12);
    inner.add_css_class("dn-omnibox-row");
    inner.add_css_class("dn-skeleton");
    let avatar = bone_circle(36);
    avatar.set_halign(gtk::Align::Start);
    inner.append(&avatar);
    let col = gtk::Box::new(gtk::Orientation::Vertical, 8);
    col.set_hexpand(true);
    col.append(&skeleton_bars(&[0.4, 0.7], 12, 8));
    inner.append(&col);
    row.set_child(Some(&inner));
    row
}

fn omnibox_heading_row(icon: Option<&str>, title: &str) -> gtk::ListBoxRow {
    let row = gtk::ListBoxRow::new();
    row.set_activatable(false);
    row.set_selectable(false);
    let inner = gtk::Box::new(gtk::Orientation::Horizontal, 6);
    inner.add_css_class("dn-omnibox-heading");
    if let Some(name) = icon {
        inner.append(&named_icon(name, 14));
    }
    let label = gtk::Label::new(Some(title));
    label.add_css_class("dn-mono");
    label.add_css_class("dn-muted");
    label.set_xalign(0.0);
    inner.append(&label);
    row.set_child(Some(&inner));
    row
}

fn select_first_activatable(list: &gtk::ListBox) {
    let mut index = 0;
    while let Some(row) = list.row_at_index(index) {
        if row.is_activatable() {
            list.select_row(Some(&row));
            return;
        }
        index += 1;
    }
}

fn move_omnibox_selection(list: &gtk::ListBox, step: i32) {
    let selected = list.selected_row().map(|row| row.index()).unwrap_or(-1);
    let mut index = selected + step;
    loop {
        let Some(row) = list.row_at_index(index) else {
            return;
        };
        if row.is_activatable() {
            list.select_row(Some(&row));
            return;
        }
        index += step;
    }
}

pub struct EmojiSuggest {
    popover: gtk::Popover,
    list: gtk::ListBox,
    items: Rc<RefCell<Vec<emoji::Suggestion>>>,
    open: Rc<Cell<bool>>,
    view: RefCell<Option<gtk::TextView>>,
}

impl EmojiSuggest {
    pub fn new(sender: Sender<AppMsg>) -> Self {
        let popover = gtk::Popover::new();
        popover.set_autohide(true);
        popover.set_has_arrow(true);
        popover.set_position(gtk::PositionType::Bottom);
        popover.set_can_focus(false);
        popover.add_css_class("dn-emoji");

        let list = gtk::ListBox::new();
        list.set_selection_mode(gtk::SelectionMode::Browse);
        list.set_can_focus(false);
        list.add_css_class("dn-emoji-list");
        list.set_activate_on_single_click(true);
        popover.set_child(Some(&list));

        let items = Rc::new(RefCell::new(Vec::<emoji::Suggestion>::new()));
        list.connect_row_activated({
            let items = items.clone();
            let sender = sender.clone();
            move |_, row| {
                let index = row.index() as usize;
                if let Some(item) = items.borrow().get(index) {
                    sender.emit(AppMsg::EmojiChosen(item.emoji.clone()));
                }
            }
        });

        let open = Rc::new(Cell::new(false));
        popover.connect_closed({
            let open = open.clone();
            move |_| open.set(false)
        });

        Self {
            popover,
            list,
            items,
            open,
            view: RefCell::new(None),
        }
    }

    pub fn open_flag(&self) -> Rc<Cell<bool>> {
        self.open.clone()
    }

    pub fn show(&self, view: &gtk::TextView, query: &str, items: &[emoji::Suggestion]) {
        if items.is_empty() {
            self.hide();
            return;
        }
        ensure_popover_parent(&self.popover, view);
        *self.view.borrow_mut() = Some(view.clone());
        *self.items.borrow_mut() = items.to_vec();
        clear_list(&self.list);
        for (index, item) in items.iter().enumerate() {
            let row = emoji_row(item, query);
            self.list.append(&row);
            if index == 0 {
                self.list.select_row(Some(&row));
            }
        }
        if let Some(rect) = cursor_rect(view) {
            self.popover.set_pointing_to(Some(&rect));
        }
        if !self.popover.is_visible() {
            self.popover.popup();
        }
        self.open.set(true);
    }

    pub fn hide(&self) {
        if self.popover.is_visible() {
            self.popover.popdown();
        }
        self.open.set(false);
        self.view.borrow_mut().take();
        self.items.borrow_mut().clear();
    }

    pub fn move_selection(&self, step: i32) {
        let count = self.items.borrow().len() as i32;
        if count == 0 {
            return;
        }
        let current = self.list.selected_row().map(|row| row.index()).unwrap_or(0);
        let next = (current + step).rem_euclid(count);
        if let Some(row) = self.list.row_at_index(next) {
            self.list.select_row(Some(&row));
        }
    }

    pub fn active_emoji(&self) -> Option<String> {
        let index = self.list.selected_row()?.index() as usize;
        self.items
            .borrow()
            .get(index)
            .map(|item| item.emoji.clone())
    }

    pub fn view(&self) -> Option<gtk::TextView> {
        self.view.borrow().clone()
    }
}

fn ensure_popover_parent(popover: &gtk::Popover, view: &gtk::TextView) {
    if popover.parent().as_ref() == Some(view.upcast_ref()) {
        return;
    }
    if popover.parent().is_some() {
        popover.unparent();
    }
    popover.set_parent(view);
}

fn cursor_rect(view: &gtk::TextView) -> Option<gtk::gdk::Rectangle> {
    let buf = view.buffer();
    let iter = buf.iter_at_offset(buf.cursor_position());
    let loc = view.iter_location(&iter);
    let (x, y) = view.buffer_to_window_coords(gtk::TextWindowType::Widget, loc.x(), loc.y());
    Some(gtk::gdk::Rectangle::new(
        x,
        y,
        loc.width().max(1),
        loc.height().max(1),
    ))
}

fn emoji_row(item: &emoji::Suggestion, query: &str) -> gtk::ListBoxRow {
    let row = gtk::ListBoxRow::new();
    row.add_css_class("dn-emoji-row");
    row.set_activatable(true);
    pointer(&row);
    let inner = gtk::Box::new(gtk::Orientation::Horizontal, 10);
    let glyph = gtk::Label::new(Some(&item.emoji));
    glyph.add_css_class("dn-emoji-char");
    glyph.set_width_chars(2);
    inner.append(&glyph);
    let name = gtk::Label::new(None);
    name.set_use_markup(true);
    name.set_markup(&emoji::highlighted_shortcode(&item.name, query));
    name.add_css_class("dn-emoji-name");
    name.add_css_class("dn-mono");
    name.set_xalign(0.0);
    name.set_hexpand(true);
    inner.append(&name);
    row.set_child(Some(&inner));
    row
}

#[cfg(test)]
mod last_visit_tests {
    use super::*;

    fn commit(id: &str, at: &str, author: &str) -> Commit {
        Commit {
            id: id.to_owned(),
            user_id: Some(author.to_owned()),
            created_at: DateTime::parse_from_rfc3339(at)
                .ok()
                .map(|dt| dt.with_timezone(&Utc)),
            ..Default::default()
        }
    }

    #[test]
    fn marks_first_already_seen_commit_when_others_posted() {
        let commits = [
            commit("new", "2026-10-26T12:00:00Z", "alice"),
            commit("seen", "2026-10-26T10:00:00Z", "bob"),
        ];
        assert_eq!(
            last_visit_id(&commits, Some("2026-10-26T11:00:00Z"), Some("me")).as_deref(),
            Some("seen")
        );
    }

    #[test]
    fn hides_marker_when_only_own_commits_are_new() {
        let commits = [
            commit("mine", "2026-10-26T12:00:00Z", "me"),
            commit("seen", "2026-10-26T10:00:00Z", "bob"),
        ];
        assert_eq!(
            last_visit_id(&commits, Some("2026-10-26T11:00:00Z"), Some("me")),
            None
        );
    }

    #[test]
    fn hides_marker_without_a_previous_visit() {
        let commits = [commit("new", "2026-10-26T12:00:00Z", "alice")];
        assert_eq!(last_visit_id(&commits, None, Some("me")), None);
    }
}

#[cfg(test)]
mod badge_tests {
    use super::*;

    #[test]
    fn caps_counts_above_ninety_nine() {
        assert_eq!(badge_text(0), "0");
        assert_eq!(badge_text(3), "3");
        assert_eq!(badge_text(99), "99");
        assert_eq!(badge_text(100), "99+");
    }

    #[test]
    fn profile_bar_names_the_commit_count() {
        assert_eq!(commits_label(0), "0 commits");
        assert_eq!(commits_label(1), "1 commit");
        assert_eq!(commits_label(33), "33 commits");
        assert_eq!(commits_label(355), "355 commits");
    }

    #[test]
    fn shows_every_unlocked_badge_when_the_row_is_wide_enough() {
        let widths = [80, 90, 70, 110];
        assert_eq!(visible_badge_count(&widths, 48, 400), 4);
        assert_eq!(badge_more_label(0), "+0 more");
    }

    #[test]
    fn reserves_room_for_plus_more_when_badges_overflow() {
        let widths = [80, 80, 80, 80];
        // 80+8+80+8+80 = 256, plus 8+48 for "+N more" = 312, which fits in 320.
        // The fourth chip would need 320 + 8 + 80.
        assert_eq!(visible_badge_count(&widths, 48, 320), 3);
        assert_eq!(badge_more_label(1), "+1 more");
        assert_eq!(badge_more_label(7), "+7 more");
    }

    #[test]
    fn hides_every_chip_when_even_one_plus_more_cannot_fit() {
        assert_eq!(visible_badge_count(&[80, 80], 48, 40), 0);
        assert_eq!(visible_badge_count(&[], 48, 400), 0);
        assert_eq!(visible_badge_count(&[80], 48, 1), 0);
    }

    #[test]
    fn hover_ring_uses_the_lane_hex_not_commit_blue() {
        let css = hex_tone_css("#9c36b5", "dn-hex-9c36b5");
        assert!(css.contains("border-color: #9c36b5"));
        assert!(!css.contains("@dn_primary"));
        assert!(css.contains("button.dn-badge-chip:hover .dn-badge.unlocked.dn-hex-9c36b5"));
    }
}

#[cfg(test)]
mod activity_tests {
    use super::*;

    #[test]
    fn summary_keeps_the_count_apart_from_the_window() {
        assert_eq!(
            activity_summary_copy(33, "in the last 30 days"),
            ("33".into(), "commits in the last 30 days".into())
        );
        assert_eq!(
            activity_summary_copy(1, "in the last 24 hours"),
            ("1".into(), "commit in the last 24 hours".into())
        );
    }
}

#[cfg(test)]
mod profile_icon_tests {
    use super::*;

    #[test]
    fn profile_meta_icons_are_filled_symbolic_paths() {
        let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../resources/icons/hicolor/scalable/actions");
        for name in [
            "dn-building-2-symbolic.svg",
            "dn-map-pin-symbolic.svg",
            "dn-link-2-symbolic.svg",
            "dn-calendar-days-symbolic.svg",
        ] {
            let svg = std::fs::read_to_string(dir.join(name)).unwrap();
            assert!(
                !svg.contains("fill=\"none\""),
                "{name} still has stroke-only Lucide markup"
            );
            assert!(
                svg.contains("fill=\"#2e3436\""),
                "{name} missing symbolic fill"
            );
        }
    }
}

#[cfg(test)]
mod embed_tests {
    use super::*;

    #[test]
    fn parses_youtube_aspect_ratios() {
        let wide = parse_aspect(Some("16 / 9")).unwrap();
        let tall = parse_aspect(Some("9 / 16")).unwrap();
        assert!((wide - 16.0 / 9.0).abs() < f32::EPSILON);
        assert!((tall - 9.0 / 16.0).abs() < f32::EPSILON);
        assert!(wide > 1.0);
        assert!(tall < 1.0);
    }
}

#[cfg(test)]
mod readme_tests {
    use super::*;

    #[test]
    fn converts_html_images_to_markdown() {
        let md = htmlish_to_markdown("<p>Hi</p><img src=\"banner.png\" alt=\"banner\"><br/>More");
        assert!(md.contains("![banner](banner.png)"));
        assert!(md.contains("Hi"));
        assert!(md.contains("More"));
    }

    #[test]
    fn resolves_relative_github_images() {
        assert_eq!(
            github_asset_url("ada", "./banner.png"),
            "https://raw.githubusercontent.com/ada/ada/HEAD/banner.png"
        );
        assert_eq!(
            github_asset_url("ada", "https://example.com/a.png"),
            "https://example.com/a.png"
        );
    }

    #[test]
    fn splits_markdown_images_from_prose() {
        let parts = readme_parts("Hello\n\n![banner](./hi.png)\n\nBye", "ada");
        assert!(matches!(&parts[0], ReadmePart::Markdown(t) if t.contains("Hello")));
        assert!(
            matches!(&parts[1], ReadmePart::Image { url, alt } if url.contains("raw.githubusercontent.com/ada/ada/HEAD/hi.png") && alt == "banner")
        );
        assert!(matches!(&parts[2], ReadmePart::Markdown(t) if t.contains("Bye")));
    }
}

#[cfg(test)]
mod suggestion_tests {
    use super::*;

    fn user(username: &str, followed: bool) -> User {
        User {
            username: username.into(),
            is_followed: Some(followed),
            ..Default::default()
        }
    }

    #[test]
    fn who_to_follow_skips_people_already_followed() {
        let users = [
            user("ada", true),
            user("grace", false),
            user("me", false),
            user("linus", false),
        ];
        let shown: Vec<&str> = suggestion_candidates(&users, Some("me"))
            .iter()
            .map(|user| user.username.as_str())
            .collect();
        assert_eq!(shown, ["grace", "linus"]);
    }
}

#[cfg(test)]
mod skeleton_tests {
    use super::*;

    #[test]
    fn wash_is_lighter_in_light_mode() {
        let light = wash_rgb(false);
        let dark = wash_rgb(true);
        assert!(light.0 + light.1 + light.2 > dark.0 + dark.1 + dark.2);
    }
}
