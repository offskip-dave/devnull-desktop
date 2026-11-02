use std::cell::{Cell, RefCell};
use std::collections::{HashMap, HashSet};
use std::rc::Rc;
use std::time::{Duration, Instant};

use relm4::adw::prelude::*;
use relm4::adw::{self};
use relm4::gtk;
use relm4::gtk::gio;
use relm4::gtk::glib;
use relm4::gtk::pango;
use relm4::prelude::*;

use devnull_api::{
    ApiError, ChatThread, Client, Commit, CommitPage, CommitTranslation, ConversationSummary,
    CurrentUser, DeviceKey, HotTopic, InboxPage, InboxTab, Lane, LogKind, LogPage,
    MentionSuggestions, Message, MessageReaction, Notification, NotificationFilter, Organization,
    Page, PastelAward, Profile, RemoteCandidate, SearchResults, ThreadRef, Upload, User,
    UserPreview, UserSettings,
};
use devnull_auth::{self as auth, Pkce, APP_ID};

use crate::prefs::{Prefs, Theme};
use crate::ui;
use crate::{APP_SENDER, PENDING_URI};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Nav {
    Following,
    Latest,
    Trending,
    Starred,
    Notifications,
    Remotes,
}

impl Nav {
    fn log(self) -> Option<LogKind> {
        match self {
            Self::Following => Some(LogKind::Following),
            Self::Latest => Some(LogKind::Latest),
            Self::Trending => Some(LogKind::Trending),
            Self::Starred | Self::Notifications | Self::Remotes => None,
        }
    }

    fn title(self) -> &'static str {
        match self {
            Self::Following => "Following",
            Self::Latest => "Latest",
            Self::Trending => "Trending",
            Self::Starred => "Starred",
            Self::Notifications => "Notifications",
            Self::Remotes => "Remotes",
        }
    }
}

#[derive(Debug)]
pub enum AppMsg {
    Restore,
    Restored(Result<(CurrentUser, Client), ApiError>),
    SignIn,
    BrowserOpened(Pkce, String),
    CancelSignIn,
    OpenUri(String),
    PasteCallback,
    PollClipboard,
    Redeemed(Result<(CurrentUser, Client), ApiError>),
    SignOut,
    Select(Nav),
    OpenMe,
    ComposerAvatar,
    PostAs(Option<String>),
    OpenSettings,
    SettingsTab(SettingsTab),
    SetTheme(Theme),
    SetWhoToFollow(bool),
    SetHighlightOnHover(bool),
    SetTimezone(Option<String>),
    TimezoneSaved(Result<CurrentUser, ApiError>),
    SetAutomaticTranslation(bool),
    AutomaticTranslationSaved(Result<CurrentUser, ApiError>),
    SetTranslationLanguage(Option<String>),
    TranslationLanguageSaved(Result<CurrentUser, ApiError>),
    UserSettingsLoaded(Result<UserSettings, ApiError>),
    SetRemotes(bool),
    RemotesSaved(Result<CurrentUser, ApiError>),
    ExportData,
    ExportReady(Result<Vec<u8>, ApiError>),
    DeleteAccount,
    ConfirmDeleteAccount,
    AccountDeleted(Result<(), ApiError>),
    SafetyLoaded(Result<Vec<User>, ApiError>, Result<Vec<User>, ApiError>),
    Unmute(String),
    Unblock(String),
    SafetyChanged(String, bool, Result<(), ApiError>),
    FocusSearch,
    FocusComposer,
    OpenCompose,
    ComposeClosed,
    ToggleSidebar,
    NavCompact(bool),
    AsideCompact(bool),
    ShellCompact(bool),
    TogglePoll,
    RemovePoll,
    ToggleSchedule,
    TogglePreview,
    SetPasteis(bool),
    PasteisSaved(Result<CurrentUser, ApiError>),
    StartRemote,
    CopyCommitLink,
    CopyPermalink(String),
    ReportCommit(Commit),
    SubmitReport {
        id: String,
        reason: String,
        details: String,
    },
    ReportSubmitted(Result<(), ApiError>),
    ShowShortcuts,
    ShortcutCompose,
    ShortcutOnCommit(char),
    MoveCommit(i32),
    CopyHomeLink,
    AppearanceChanged,
    Back,
    ComposerChanged,
    ReplyComposerChanged,
    RemoteComposerChanged,
    EmojiMove(i32),
    EmojiPick,
    EmojiDismiss,
    EmojiChosen(String),
    SubmitReply,
    LogLoaded(Result<LogPage, ApiError>),
    LoadMore,
    ToggleStar(String, bool, i64),
    StarToggled(String, bool, i64, Result<(), ApiError>),
    GivePastel(String),
    PastelGiven(String, Result<PastelAward, ApiError>),
    StartQuote(Commit),
    SubmitQuote(String, String),
    ReplyTo(String),
    CancelReply,
    TranslateCommit(String),
    ShowOriginal(String),
    CommitTranslated(String, Result<CommitTranslation, ApiError>),
    ToggleReplies(String),
    NestedRepliesLoaded(String, Result<Page<Commit>, ApiError>),
    PreviewRepliesLoaded(String, Result<Page<Commit>, ApiError>),
    ToggleNotificationFilter(NotificationFilter),
    ShowAllNotificationFilters,
    OpenCommit(String),
    CommitLoaded(Result<CommitPage, ApiError>, Result<Page<Commit>, ApiError>),
    RelatedLoaded(String, Vec<Commit>),
    OpenProfile(String),
    OpenOrganization(String),
    ProfileLoaded(
        Result<Box<Profile>, ApiError>,
        Result<Page<Commit>, ApiError>,
    ),
    OrganizationLoaded(
        Result<Box<Organization>, ApiError>,
        Result<Page<Commit>, ApiError>,
    ),
    ProfileTab(ProfileFeed),
    ProfileReadmeLoaded(String, Result<Option<String>, ApiError>),
    ProfileBarScrolled(bool),
    GoLive,
    GoLiveSubmit {
        url: String,
        hours: i64,
    },
    EndLive,
    LiveSaved(Result<CurrentUser, ApiError>),
    OpenConnections {
        handle: String,
        kind: ConnectionsKind,
    },
    OpenStargazers {
        commit_id: String,
        count: i64,
    },
    ConnectionsLoaded {
        handle: String,
        kind: ConnectionsKind,
        more: bool,
        users: Result<Page<User>, ApiError>,
        organizations: Result<Page<Organization>, ApiError>,
    },
    LoadMoreConnections,
    ExpandConnectionOrgs,
    RetryStargazers,
    VotePoll {
        commit: String,
        option: String,
    },
    PollVoted(String, Result<Commit, ApiError>),
    MentionMove(i32),
    MentionPick,
    MentionDismiss,
    MentionChosen(String),
    MentionsLoaded(u64, Result<MentionSuggestions, ApiError>),
    ShareProfile,
    CopyProfileId,
    PickEmoji,
    ClaimOrganization,
    OrganizationClaimed(String, Result<(), ApiError>),
    FollowOrganization(String, bool),
    OrganizationFollowed(String, bool),
    EditOnGitHub,
    SyncProfile,
    ProfileSynced(Result<CurrentUser, ApiError>),
    DraftLane(Option<Lane>),
    SetLane(Option<Lane>),
    LaneSaved(Result<CurrentUser, ApiError>),
    InspectAchievements,
    OmniboxQuery(String),
    OmniboxGo(u64, String),
    OmniboxLoaded(u64, String, Result<SearchResults, ApiError>),
    RunSearch(String),
    SearchLoaded(Result<SearchResults, ApiError>),
    SuggestedLoaded(Result<Vec<User>, ApiError>, HashSet<String>),
    LiveFollowingLoaded(Result<Vec<User>, ApiError>),
    HotTopicsLoaded(Result<Vec<HotTopic>, ApiError>),
    FollowUser(String, bool),
    UserFollowed(String, bool),
    RefreshSuggested,
    RefreshMeAvatars,
    PreviewUser(String),
    UserPreviewLoaded(String, Result<UserPreview, ApiError>),
    MuteUser(String),
    UserMuted(String, Result<(), ApiError>),
    ConfirmBlockUser(String),
    BlockUser(String),
    UserBlocked(String, Result<(), ApiError>),
    ToggleFollowCurrent,
    AttachImages,
    ImagesPicked(Vec<Upload>),
    RemoveComposerImage(usize),
    RemoveRemoteImage(usize),
    SubmitCommit,
    CommitCreated(Result<Commit, ApiError>),
    NotificationsLoaded(Result<Page<Notification>, ApiError>),
    ToggleNotificationGroup(String),
    LoadMoreNotificationGroup(String),
    NotificationGroupLoaded(String, bool, Result<Page<Notification>, ApiError>),
    RemotesLoaded(Result<InboxPage, ApiError>),
    InboxTab(InboxTab),
    InboxQuery(String),
    InboxSearchLoaded(u64, String, Result<InboxPage, ApiError>, Vec<User>),
    ToggleChatDock,
    ToggleDockExpand,
    PickRemote,
    PickOrg,
    PeopleQuery(String),
    PeopleLoaded(u64, Result<Vec<RemoteCandidate>, ApiError>),
    CreateOrg {
        name: String,
        members: Vec<String>,
    },
    OrgCreated(Result<ThreadRef, ApiError>),
    SetRemoteFrom(String),
    SetPullReceipts(bool),
    RemotePrefsSaved(Result<CurrentUser, ApiError>),
    UnreadPolled(Result<(i64, i64), ApiError>),
    OpenThread(ThreadRef),
    ThreadLoaded(u64, Result<ChatThread, ApiError>),
    AttachRemoteImages,
    PushRemote,
    RemotePushed(Result<devnull_api::Message, ApiError>),
    PinRemote,
    StashRemote,
    MuteRemote,
    LeaveOrg,
    BlockRemote,
    ConfirmLeaveOrg,
    ConfirmBlockRemote,
    CloseThread,
    ThreadActionDone(Result<String, ApiError>),
    PingMention,
    StartCall,
    PinThreadMessage(i64),
    ConfirmPinThreadMessage {
        id: i64,
        dismiss: bool,
    },
    ThreadPinsLoaded(String, bool, Result<Vec<Message>, ApiError>),
    JumpToThreadMessage(i64),
    ReplyToRemote(Message),
    CancelRemoteReply,
    ReactToRemote {
        id: i64,
        emoji: String,
    },
    MessageReactionsLoaded(String, i64, Result<Vec<MessageReaction>, ApiError>),
    RecordVoice,
    Toast(String),
    Poll,
    OpenWeb(String),
    LogPolled(Result<LogPage, ApiError>),
    JumpToNewest,
    LogScrolled,
    DismissSuggested(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Phase {
    Loading,
    SignedOut,
    WaitingForBrowser,
    SignedIn,
}

pub struct App {
    phase: Phase,
    visible: &'static str,
    user: Option<CurrentUser>,
    client: Option<Client>,
    error: Option<String>,
    pending: Option<Pkce>,
    nav: Nav,
    log_cursor: Option<String>,
    profile: Option<Profile>,
    organization: Option<Organization>,
    profile_tab: ProfileFeed,
    profile_readme: Option<(String, Result<Option<String>, ApiError>)>,
    suggested: Vec<User>,
    following: HashSet<String>,
    user_settings: Option<UserSettings>,
    translations: HashMap<String, CommitTranslation>,
    showing_original: HashSet<String>,
    requested_translation: HashSet<String>,
    translation_loading: HashSet<String>,
    auto_translate_retry_at: Option<Instant>,
    live_following: Vec<User>,
    hot_topics: Vec<HotTopic>,
    muted: Vec<User>,
    blocked: Vec<User>,
    settings_tab: SettingsTab,
    draft_lane: Option<Option<Lane>>,
    prefs: Prefs,
    hover_enabled: Rc<Cell<bool>>,
    user_card: Rc<ui::UserCard>,
    user_preview_loading: HashSet<String>,
    open_commit: Option<Commit>,
    ancestors: Vec<Commit>,
    related_commits: Vec<Commit>,
    thread_replies: Vec<Commit>,
    nested_replies: HashMap<String, Vec<Commit>>,
    expanded_replies: HashSet<String>,
    previewed_replies: HashSet<String>,
    preview_loading: HashSet<String>,
    log_commits: Vec<Commit>,
    profile_commits: Vec<Commit>,
    log_first_seen: Option<String>,
    log_loading_more: bool,
    replying_to: Option<String>,
    inline_reply: gtk::TextView,
    inline_reply_box: gtk::Box,
    focus_reply: bool,
    notif_types: HashSet<NotificationFilter>,
    notifications: Vec<Notification>,
    notif_groups: HashMap<String, ui::NotifGroup>,
    busy: u32,
    thread_ref: Option<ThreadRef>,
    composer_images: Vec<Upload>,
    remote_images: Vec<Upload>,
    unread_notifications: i64,
    unread_remotes: i64,
    counts_ready: bool,
    remote_arrivals: HashMap<String, i64>,
    last_thread_ping: Option<i64>,
    composer_len: usize,
    composer_poll: bool,
    composer_schedule: bool,
    composer_preview: bool,
    /// `None` follows the window breakpoint; `Some` is a Collapse/Expand choice.
    sidebar_collapsed: Option<bool>,
    nav_compact: bool,
    aside_compact: bool,
    shell_compact: bool,
    log_last_day: Option<String>,
    log_last_lane: Option<String>,
    log_visit_at: Option<String>,
    log_seen_id: Option<String>,
    log_away: bool,
    new_posts_count: u32,
    new_authors: Vec<User>,
    log_scroll: gtk::ScrolledWindow,
    log_composer: gtk::Box,
    new_posts: gtk::Button,
    aside_toast: gtk::Revealer,
    aside_toast_label: gtk::Label,
    toast_commit: Rc<RefCell<Option<String>>>,
    toast_hide: Rc<RefCell<Option<glib::SourceId>>>,
    stars: HashMap<String, (bool, i64)>,
    pasteis: HashMap<String, i64>,
    composer_avatar: gtk::Box,
    author_menu: gtk::Popover,
    composer_as: Option<User>,
    composer_rail: gtk::Overlay,
    composer_rail_area: gtk::DrawingArea,
    staged_paint: Rc<RefCell<ui::StagedPaint>>,
    nav_avatar: gtk::Box,
    header_avatar: gtk::Box,
    wordmark_btn: gtk::Button,
    logo_menu: gtk::Popover,
    user_menu: ui::UserMenu,
    header_user_menu: ui::UserMenu,
    log_list: gtk::ListBox,
    reply_list: gtk::ListBox,
    profile_list: gtk::ListBox,
    search_list: gtk::ListBox,
    notif_list: gtk::ListBox,
    inbox_list: gtk::ListBox,
    thread_list: gtk::ListBox,
    thread_scroll: gtk::ScrolledWindow,
    thread_follow: Rc<Cell<bool>>,
    composer: gtk::TextView,
    reply_composer: gtk::TextView,
    remote_composer: gtk::TextView,
    emoji: ui::EmojiSuggest,
    mentions: ui::MentionSuggest,
    mention_gen: u64,
    mention_timeout: Rc<RefCell<Option<glib::SourceId>>>,
    paste_entry: gtk::Entry,
    omnibox: ui::SearchOmnibox,
    omnibox_gen: u64,
    profile_hero: gtk::Box,
    profile_bar_avatar: gtk::Box,
    profile_bar_scrolled: bool,
    suggested_list: gtk::Box,
    live_list: gtk::Box,
    hot_topics_list: gtk::Box,
    settings_box: gtk::Box,
    poll_ui: ui::PollComposer,
    schedule_ui: ui::ScheduleCard,
    poll_editor: gtk::Box,
    schedule_card: gtk::Box,
    image_previews: gtk::Box,
    remote_image_previews: gtk::Box,
    compose_dialog: adw::Dialog,
    compose_host: gtk::Box,
    compose_open: bool,
    connections_kind: ConnectionsKind,
    connections_handle: String,
    connections_commit: Option<String>,
    connections_star_count: i64,
    connections_title: String,
    connections_users: Vec<User>,
    connections_orgs: Vec<Organization>,
    connections_cursor: Option<String>,
    connections_orgs_cursor: Option<String>,
    connections_orgs_expanded: bool,
    connections_loading: bool,
    connections_list: gtk::ListBox,
    inbox: Vec<ConversationSummary>,
    inbox_tab: InboxTab,
    inbox_requests: i64,
    remote_draft_len: usize,
    remote_reply: Option<Message>,
    thread_typing: String,
    last_typing_ping: Option<Instant>,
    open_thread: Option<ChatThread>,
    thread_gen: u64,
    thread_header_stamp: String,
    inbox_query: String,
    inbox_hits: Option<Vec<ConversationSummary>>,
    inbox_people: Vec<User>,
    inbox_search_gen: u64,
    remote_settings: gtk::MenuButton,
    thread_header: gtk::Box,
    chat_dock: ui::ChatDock,
    chat_dock_expanded: bool,
    people_picker: ui::PeoplePicker,
    people_gen: u64,
    bubble_width: ui::BubbleWidth,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProfileFeed {
    Commits,
    Replies,
    Readme,
    Mentions,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConnectionsKind {
    Following,
    Followers,
    Stargazers,
}

impl ConnectionsKind {
    fn title(self) -> &'static str {
        match self {
            Self::Following => "Following",
            Self::Followers => "Followers",
            Self::Stargazers => "Starred by",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SettingsTab {
    Profile,
    Appearance,
    Terminal,
    Safety,
    Devices,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Sidebar {
    Log,
    Notifications,
    Remotes,
    Profile,
    Starred,
    Settings,
    Search,
}

const MIN_BODY: usize = 3;
const MAX_BODY: usize = 4000;
/// Lane ring around the composer avatar fills solid at this length (web soft limit).
const RING_FULL_AT: usize = 200;

#[relm4::component(pub)]
impl SimpleComponent for App {
    type Init = ();
    type Input = AppMsg;
    type Output = ();

    view! {
    #[root]
    adw::ApplicationWindow {
        #[watch]
        set_title: Some(model.header_title()),
        set_default_width: 1280,
        set_default_height: 800,
        add_css_class: "dn-window",
        set_icon_name: Some(APP_ID),

        gtk::Box {
            set_orientation: gtk::Orientation::Vertical,
            set_hexpand: true,
            set_vexpand: true,

            gtk::HeaderBar {
                add_css_class: "dn-macos-header",
                set_show_title_buttons: true,
                set_decoration_layout: Some("close,minimize,maximize:"),
                set_visible: cfg!(target_os = "macos"),
            },

            #[name = "shell_overlay"]
            gtk::Overlay {
            set_overflow: gtk::Overflow::Visible,
            set_hexpand: true,
            set_vexpand: true,
            #[wrap(Some)]
            set_child = &gtk::Box {
                    set_orientation: gtk::Orientation::Horizontal,
                    set_vexpand: true,
                    add_css_class: "dn-shell",
                    set_hexpand: true,

                    gtk::Box {
                        set_hexpand: true,
                        add_css_class: "dn-gutter",
                        #[watch]
                        set_visible: !model.feed_fills(),
                    },

                    #[name = "nav_sidebar"]
                    gtk::Box {
                        set_orientation: gtk::Orientation::Vertical,
                        set_spacing: 4,
                        #[watch]
                        set_css_classes: &model.sidebar_classes(),
                        set_hexpand: false,
                        set_overflow: gtk::Overflow::Hidden,
                        #[watch]
                        set_width_request: model.sidebar_width(),
                        #[watch]
                        set_visible: model.nav_visible(),

                        #[local_ref]
                        wordmark_btn -> gtk::Button {
                            add_css_class: "flat",
                            add_css_class: "dn-wordmark-btn",
                            add_css_class: "dn-wordmark-row",
                            set_has_frame: false,
                            set_tooltip_text: Some("/dev/null"),
                            connect_clicked => AppMsg::Select(Nav::Latest),
                            #[watch]
                            set_halign: if model.nav_collapsed() {
                                gtk::Align::Center
                            } else {
                                gtk::Align::Fill
                            },
                            #[wrap(Some)]
                            set_child = &gtk::Box {
                                set_orientation: gtk::Orientation::Vertical,
                                set_spacing: 4,
                                gtk::Box {
                                    set_orientation: gtk::Orientation::Horizontal,
                                    set_spacing: 8,
                                    gtk::Image {
                                        add_css_class: "dn-mark",
                                        set_from_file: Some(ui::mark_path(ui::adw_is_dark())),
                                        set_pixel_size: 28,
                                    },
                                    gtk::Label {
                                        #[watch]
                                        set_markup: &ui::wordmark_markup(),
                                        set_xalign: 0.0,
                                        add_css_class: "dn-wordmark",
                                        #[watch]
                                        set_visible: !model.nav_collapsed(),
                                    },
                                },
                                gtk::Label {
                                    set_label: "a social network for devs",
                                    set_xalign: 0.0,
                                    add_css_class: "dn-tagline",
                                    #[watch]
                                    set_visible: !model.nav_collapsed(),
                                },
                            },
                        },

                        gtk::Button {
                            #[watch]
                            set_css_classes: &model.nav_classes(Sidebar::Log),
                            #[watch]
                            set_halign: model.nav_rail_align(),
                            set_tooltip_text: Some("Log"),
                            connect_clicked => AppMsg::Select(Nav::Latest),
                            #[wrap(Some)]
                            set_child = &gtk::Box {
                                set_orientation: gtk::Orientation::Horizontal,
                                set_spacing: 12,
                                gtk::Image {
                                    set_icon_name: Some("dn-git-commit-vertical-symbolic"),
                                    set_pixel_size: 20,
                                },
                                gtk::Label {
                                    set_label: "Log",
                                    set_xalign: 0.0,
                                    set_hexpand: true,
                                    #[watch]
                                    set_visible: !model.nav_collapsed(),
                                },
                            },
                        },
                        gtk::Button {
                            #[watch]
                            set_css_classes: &model.nav_classes(Sidebar::Notifications),
                            #[watch]
                            set_halign: model.nav_rail_align(),
                            set_overflow: gtk::Overflow::Visible,
                            set_tooltip_text: Some("Notifications"),
                            connect_clicked => AppMsg::Select(Nav::Notifications),
                            #[wrap(Some)]
                            set_child = &gtk::Box {
                                set_orientation: gtk::Orientation::Horizontal,
                                set_spacing: 12,
                                gtk::Overlay {
                                    set_overflow: gtk::Overflow::Visible,
                                    #[wrap(Some)]
                                    set_child = &gtk::Image {
                                        set_icon_name: Some("dn-bell-symbolic"),
                                        set_pixel_size: 20,
                                    },
                                    add_overlay = &gtk::Label {
                                        #[watch]
                                        set_label: &ui::badge_text(model.unread_notifications),
                                        #[watch]
                                        set_visible: model.show_notif_badge() && model.nav_collapsed(),
                                        add_css_class: "dn-nav-badge",
                                        set_halign: gtk::Align::End,
                                        set_valign: gtk::Align::Start,
                                        set_can_target: false,
                                    },
                                },
                                gtk::Label {
                                    set_label: "Notifications",
                                    set_xalign: 0.0,
                                    set_hexpand: true,
                                    #[watch]
                                    set_visible: !model.nav_collapsed(),
                                },
                                gtk::Label {
                                    #[watch]
                                    set_label: &ui::badge_text(model.unread_notifications),
                                    #[watch]
                                    set_visible: model.show_notif_badge() && !model.nav_collapsed(),
                                    add_css_class: "dn-nav-count",
                                },
                            },
                        },
                        gtk::Button {
                            #[watch]
                            set_css_classes: &model.nav_classes(Sidebar::Remotes),
                            #[watch]
                            set_halign: model.nav_rail_align(),
                            set_overflow: gtk::Overflow::Visible,
                            set_tooltip_text: Some("Remotes"),
                            #[watch]
                            set_visible: model.remotes_on(),
                            connect_clicked => AppMsg::Select(Nav::Remotes),
                            #[wrap(Some)]
                            set_child = &gtk::Box {
                                set_orientation: gtk::Orientation::Horizontal,
                                set_spacing: 12,
                                gtk::Overlay {
                                    set_overflow: gtk::Overflow::Visible,
                                    #[wrap(Some)]
                                    set_child = &gtk::Image {
                                        set_icon_name: Some("dn-messages-square-symbolic"),
                                        set_pixel_size: 20,
                                    },
                                    add_overlay = &gtk::Label {
                                        #[watch]
                                        set_label: &ui::badge_text(model.unread_remotes),
                                        #[watch]
                                        set_visible: model.show_remotes_badge() && model.nav_collapsed(),
                                        add_css_class: "dn-nav-badge",
                                        set_halign: gtk::Align::End,
                                        set_valign: gtk::Align::Start,
                                        set_can_target: false,
                                    },
                                },
                                gtk::Label {
                                    set_label: "Remotes",
                                    set_xalign: 0.0,
                                    set_hexpand: true,
                                    #[watch]
                                    set_visible: !model.nav_collapsed(),
                                },
                                gtk::Label {
                                    #[watch]
                                    set_label: &ui::badge_text(model.unread_remotes),
                                    #[watch]
                                    set_visible: model.show_remotes_badge() && !model.nav_collapsed(),
                                    add_css_class: "dn-nav-count",
                                },
                            },
                        },
                        gtk::Button {
                            #[watch]
                            set_css_classes: &model.nav_classes(Sidebar::Profile),
                            #[watch]
                            set_halign: model.nav_rail_align(),
                            set_tooltip_text: Some("Profile"),
                            connect_clicked => AppMsg::OpenMe,
                            #[wrap(Some)]
                            set_child = &gtk::Box {
                                set_orientation: gtk::Orientation::Horizontal,
                                set_spacing: 12,
                                gtk::Image {
                                    set_icon_name: Some("dn-user-round-symbolic"),
                                    set_pixel_size: 20,
                                },
                                gtk::Label {
                                    set_label: "Profile",
                                    set_xalign: 0.0,
                                    set_hexpand: true,
                                    #[watch]
                                    set_visible: !model.nav_collapsed(),
                                },
                            },
                        },
                        gtk::Button {
                            #[watch]
                            set_css_classes: &model.nav_classes(Sidebar::Starred),
                            #[watch]
                            set_halign: model.nav_rail_align(),
                            set_tooltip_text: Some("Starred"),
                            connect_clicked => AppMsg::Select(Nav::Starred),
                            #[wrap(Some)]
                            set_child = &gtk::Box {
                                set_orientation: gtk::Orientation::Horizontal,
                                set_spacing: 12,
                                gtk::Image {
                                    set_icon_name: Some("dn-star-symbolic"),
                                    set_pixel_size: 20,
                                },
                                gtk::Label {
                                    set_label: "Starred",
                                    set_xalign: 0.0,
                                    set_hexpand: true,
                                    #[watch]
                                    set_visible: !model.nav_collapsed(),
                                },
                            },
                        },
                        gtk::Button {
                            #[watch]
                            set_css_classes: &model.nav_classes(Sidebar::Settings),
                            #[watch]
                            set_halign: model.nav_rail_align(),
                            set_tooltip_text: Some("Settings"),
                            connect_clicked => AppMsg::OpenSettings,
                            #[wrap(Some)]
                            set_child = &gtk::Box {
                                set_orientation: gtk::Orientation::Horizontal,
                                set_spacing: 12,
                                gtk::Image {
                                    set_icon_name: Some("dn-settings-2-symbolic"),
                                    set_pixel_size: 20,
                                },
                                gtk::Label {
                                    set_label: "Settings",
                                    set_xalign: 0.0,
                                    set_hexpand: true,
                                    #[watch]
                                    set_visible: !model.nav_collapsed(),
                                },
                            },
                        },
                        gtk::Button {
                            add_css_class: "flat",
                            add_css_class: "dn-nav-btn",
                            #[watch]
                            set_halign: model.nav_rail_align(),
                            #[watch]
                            set_visible: model.nav_search_visible(),
                            set_tooltip_text: Some("Search"),
                            connect_clicked => AppMsg::FocusSearch,
                            #[wrap(Some)]
                            set_child = &gtk::Box {
                                set_orientation: gtk::Orientation::Horizontal,
                                set_spacing: 12,
                                gtk::Image {
                                    set_icon_name: Some("dn-search-symbolic"),
                                    set_pixel_size: 20,
                                },
                                gtk::Label {
                                    set_label: "Search",
                                    set_xalign: 0.0,
                                    set_hexpand: true,
                                    #[watch]
                                    set_visible: !model.nav_collapsed(),
                                },
                                gtk::Label {
                                    set_label: "Ctrl+K",
                                    add_css_class: "dn-kbd",
                                    #[watch]
                                    set_visible: !model.nav_collapsed(),
                                },
                            },
                        },
                        gtk::Button {
                            add_css_class: "suggested-action",
                            add_css_class: "dn-nav-commit",
                            #[watch]
                            set_hexpand: !model.nav_collapsed(),
                            #[watch]
                            set_halign: model.nav_rail_align(),
                            set_tooltip_text: Some("Write a commit (C)"),
                            connect_clicked => AppMsg::OpenCompose,
                            #[wrap(Some)]
                            set_child = &gtk::Box {
                                set_orientation: gtk::Orientation::Horizontal,
                                set_valign: gtk::Align::Center,
                                #[watch]
                                set_halign: model.nav_rail_align(),
                                #[watch]
                                set_hexpand: !model.nav_collapsed(),
                                #[watch]
                                set_spacing: if model.nav_collapsed() { 0 } else { 12 },
                                gtk::Image {
                                    set_icon_name: Some("dn-square-pen-symbolic"),
                                    set_pixel_size: 20,
                                },
                                gtk::Label {
                                    set_label: "Commit",
                                    set_xalign: 0.0,
                                    set_hexpand: true,
                                    #[watch]
                                    set_visible: !model.nav_collapsed(),
                                },
                            },
                        },
                        gtk::Button {
                            add_css_class: "flat",
                            add_css_class: "dn-nav-btn",
                            #[watch]
                            set_halign: model.nav_rail_align(),
                            #[watch]
                            set_tooltip_text: Some(model.collapse_label()),
                            connect_clicked => AppMsg::ToggleSidebar,
                            #[wrap(Some)]
                            set_child = &gtk::Box {
                                set_orientation: gtk::Orientation::Horizontal,
                                set_spacing: 12,
                                gtk::Image {
                                    #[watch]
                                    set_icon_name: Some(model.collapse_icon()),
                                    set_pixel_size: 20,
                                },
                                gtk::Label {
                                    #[watch]
                                    set_label: model.collapse_label(),
                                    set_xalign: 0.0,
                                    set_hexpand: true,
                                    #[watch]
                                    set_visible: !model.nav_collapsed(),
                                },
                            },
                        },

                        gtk::Box {
                            set_vexpand: true,
                        },

                            gtk::MenuButton {
                                add_css_class: "flat",
                                add_css_class: "dn-user-menu",
                                set_overflow: gtk::Overflow::Visible,
                                set_always_show_arrow: false,
                                set_has_frame: false,
                                #[watch]
                                set_halign: model.nav_rail_align(),
                                set_direction: gtk::ArrowType::Up,
                            #[wrap(Some)]
                            set_child = &gtk::Box {
                                set_orientation: gtk::Orientation::Horizontal,
                                set_spacing: 10,
                                #[local_ref]
                                nav_avatar -> gtk::Box {
                                    add_css_class: "dn-nav-avatar",
                                    set_valign: gtk::Align::Center,
                                },
                                gtk::Box {
                                    set_orientation: gtk::Orientation::Vertical,
                                    set_spacing: 0,
                                    set_hexpand: true,
                                    #[watch]
                                    set_visible: !model.nav_collapsed(),
                                    gtk::Label {
                                        #[watch]
                                        set_label: &model.me_name(),
                                        set_xalign: 0.0,
                                        add_css_class: "dn-name",
                                    },
                                    gtk::Label {
                                        #[watch]
                                        set_label: &model.me_handle(),
                                        set_xalign: 0.0,
                                        add_css_class: "dn-handle",
                                    },
                                },
                            },
                            set_popover: Some(&model.user_menu.popover),
                        },
                    },

                    gtk::Box {
                        set_orientation: gtk::Orientation::Vertical,
                        #[watch]
                        set_css_classes: &model.feed_classes(),
                        #[watch]
                        set_hexpand: model.feed_fills(),
                        #[watch]
                        set_width_request: model.feed_width_request(),
                        set_vexpand: true,

                        gtk::Box {
                            set_orientation: gtk::Orientation::Horizontal,
                            set_spacing: 4,
                            add_css_class: "dn-mobile-header",
                            #[watch]
                            set_visible: model.mobile_header_visible(),

                            gtk::Button {
                                add_css_class: "flat",
                                add_css_class: "dn-wordmark-btn",
                                set_has_frame: false,
                                set_tooltip_text: Some("/dev/null"),
                                connect_clicked => AppMsg::Select(Nav::Latest),
                                #[wrap(Some)]
                                set_child = &gtk::Box {
                                    set_orientation: gtk::Orientation::Horizontal,
                                    set_spacing: 8,
                                    gtk::Image {
                                        add_css_class: "dn-mark",
                                        set_from_file: Some(ui::mark_path(ui::adw_is_dark())),
                                        set_pixel_size: 24,
                                    },
                                    gtk::Label {
                                        #[watch]
                                        set_markup: &ui::wordmark_markup(),
                                        set_xalign: 0.0,
                                        add_css_class: "dn-wordmark",
                                    },
                                },
                            },
                            gtk::Box {
                                set_hexpand: true,
                            },
                            gtk::Button {
                                add_css_class: "flat",
                                add_css_class: "dn-mobile-header-btn",
                                set_icon_name: "dn-search-symbolic",
                                set_tooltip_text: Some("Search"),
                                connect_clicked => AppMsg::FocusSearch,
                            },
                            gtk::MenuButton {
                                add_css_class: "flat",
                                add_css_class: "dn-mobile-header-btn",
                                add_css_class: "dn-user-menu",
                                set_always_show_arrow: false,
                                set_has_frame: false,
                                set_tooltip_text: Some("Account menu"),
                                #[wrap(Some)]
                                set_child = &gtk::Box {
                                    #[local_ref]
                                    header_avatar -> gtk::Box {
                                        add_css_class: "dn-nav-avatar",
                                        set_valign: gtk::Align::Center,
                                    },
                                },
                                set_popover: Some(&model.header_user_menu.popover),
                            },
                        },

                    adw::Clamp {
                        #[watch]
                        set_maximum_size: model.feed_width(),
                        #[watch]
                        set_tightening_threshold: model.feed_width(),
                        set_unit: adw::LengthUnit::Px,
                        set_hexpand: true,

                    #[name = "stack"]
                    #[wrap(Some)]
                    set_child = &gtk::Stack {
                        set_hexpand: true,
                        set_hhomogeneous: false,
                        add_css_class: "dn-feed",

                        add_titled[Some("sign_in"), "Sign in"] = &gtk::Box {
                            set_orientation: gtk::Orientation::Vertical,
                            set_valign: gtk::Align::Center,
                            set_halign: gtk::Align::Center,
                            set_spacing: 16,
                            set_margin_start: 32,
                            set_margin_end: 32,

                            gtk::Label {
                                set_markup: "<span size='xx-large' weight='bold'>/dev/null</span>",
                            },
                            gtk::Label {
                                set_label: "A social network for developers.",
                                add_css_class: "dn-muted",
                            },
                            gtk::Button {
                                set_label: "Sign in with GitHub",
                                add_css_class: "suggested-action",
                                add_css_class: "pill",
                                #[watch]
                                set_sensitive: model.phase != Phase::WaitingForBrowser,
                                connect_clicked => AppMsg::SignIn,
                            },
                            gtk::Label {
                                #[watch]
                                set_label: model.sign_in_hint(),
                                add_css_class: "dn-muted",
                                set_wrap: true,
                                set_max_width_chars: 48,
                            },
                            gtk::Box {
                                set_orientation: gtk::Orientation::Vertical,
                                set_spacing: 8,
                                #[watch]
                                set_visible: model.phase == Phase::WaitingForBrowser,

                                gtk::Box {
                                    set_orientation: gtk::Orientation::Horizontal,
                                    set_spacing: 8,
                                    set_halign: gtk::Align::Center,

                                    gtk::Button {
                                        set_label: "I approved it",
                                        connect_clicked => AppMsg::PollClipboard,
                                    },
                                    gtk::Button {
                                        set_label: "Cancel",
                                        add_css_class: "flat",
                                        connect_clicked => AppMsg::CancelSignIn,
                                    },
                                },
                                #[local_ref]
                                paste_entry -> gtk::Entry {
                                    set_placeholder_text: Some("Last resort: paste the callback URL"),
                                    set_hexpand: true,
                                    set_width_chars: 28,
                                    connect_activate => AppMsg::PasteCallback,
                                },
                            },
                            gtk::Label {
                                #[watch]
                                set_label: model.error.as_deref().unwrap_or(""),
                                add_css_class: "dn-fatal",
                                set_wrap: true,
                            },
                        },

                        add_titled[Some("log"), "Log"] = &gtk::Box {
                            set_orientation: gtk::Orientation::Vertical,

                            gtk::Box {
                                set_orientation: gtk::Orientation::Horizontal,
                                set_homogeneous: true,
                                add_css_class: "dn-tabs",
                                #[watch]
                                set_visible: model.nav.log().is_some(),
                                gtk::Button {
                                    set_label: "Following",
                                    add_css_class: "flat",
                                    #[watch]
                                    set_css_classes: &model.tab_classes(Nav::Following),
                                    connect_clicked => AppMsg::Select(Nav::Following),
                                },
                                gtk::Button {
                                    set_label: "Latest",
                                    add_css_class: "flat",
                                    #[watch]
                                    set_css_classes: &model.tab_classes(Nav::Latest),
                                    connect_clicked => AppMsg::Select(Nav::Latest),
                                },
                                gtk::Button {
                                    set_label: "Trending",
                                    add_css_class: "flat",
                                    #[watch]
                                    set_css_classes: &model.tab_classes(Nav::Trending),
                                    connect_clicked => AppMsg::Select(Nav::Trending),
                                },
                            },
                            gtk::Label {
                                set_label: "Starred",
                                add_css_class: "dn-title",
                                set_xalign: 0.0,
                                set_margin_start: 16,
                                set_margin_top: 12,
                                set_margin_bottom: 8,
                                #[watch]
                                set_visible: model.nav == Nav::Starred,
                            },

                            #[local_ref]
                            log_composer -> gtk::Box {
                                set_orientation: gtk::Orientation::Horizontal,
                                set_spacing: 0,
                                set_hexpand: true,
                                set_vexpand: false,
                                add_css_class: "dn-composer",
                                add_css_class: "dn-log-composer",
                                #[watch]
                                set_visible: model.nav.log().is_some() || model.compose_open,
                                #[local_ref]
                                composer_rail -> gtk::Overlay {
                                    add_css_class: "dn-rail",
                                    set_width_request: 64,
                                    set_hexpand: false,
                                    set_vexpand: false,
                                    set_halign: gtk::Align::Start,
                                    set_valign: gtk::Align::Fill,
                                },
                                gtk::Box {
                                    set_orientation: gtk::Orientation::Vertical,
                                    set_spacing: 12,
                                    set_hexpand: true,
                                    add_css_class: "dn-composer-body",
                                    gtk::Overlay {
                                        #[wrap(Some)]
                                        set_child = &gtk::ScrolledWindow {
                                            set_min_content_height: 40,
                                            set_max_content_height: 180,
                                            set_vexpand: false,
                                            set_has_frame: false,
                                            add_css_class: "dn-composer-input",
                                            set_child: Some(&model.composer),
                                        },
                                        add_overlay = &gtk::Label {
                                            #[watch]
                                            set_label: model.composer_placeholder(),
                                            add_css_class: "dn-placeholder",
                                            set_halign: gtk::Align::Start,
                                            set_valign: gtk::Align::Start,
                                            set_can_target: false,
                                            #[watch]
                                            set_visible: model.composer_len == 0 && !model.composer_preview,
                                        },
                                        add_overlay = &gtk::Label {
                                            #[watch]
                                            set_markup: &model.composer_preview_markup(),
                                            add_css_class: "dn-composer-preview",
                                            set_halign: gtk::Align::Start,
                                            set_valign: gtk::Align::Start,
                                            set_wrap: true,
                                            set_xalign: 0.0,
                                            #[watch]
                                            set_visible: model.composer_preview,
                                        },
                                    },
                                    #[local_ref]
                                    poll_editor -> gtk::Box {
                                        #[watch]
                                        set_visible: model.composer_poll,
                                        #[watch]
                                        set_sensitive: !model.composer_preview,
                                    },
                                    #[local_ref]
                                    schedule_card -> gtk::Box {
                                        #[watch]
                                        set_visible: model.composer_schedule,
                                        #[watch]
                                        set_sensitive: !model.composer_preview,
                                    },
                                    #[local_ref]
                                    image_previews -> gtk::Box {
                                        set_orientation: gtk::Orientation::Horizontal,
                                        set_spacing: 8,
                                        add_css_class: "dn-attach-previews",
                                        #[watch]
                                        set_visible: !model.composer_images.is_empty(),
                                        #[watch]
                                        set_sensitive: !model.composer_preview,
                                    },
                                    gtk::Box {
                                        set_orientation: gtk::Orientation::Horizontal,
                                        set_spacing: 4,
                                        gtk::Button {
                                            set_icon_name: "dn-image-plus-symbolic",
                                            #[watch]
                                            set_tooltip_text: Some(model.image_tooltip()),
                                            add_css_class: "flat",
                                            add_css_class: "dn-composer-icon",
                                            #[watch]
                                            set_sensitive: !model.composer_poll && model.composer_images.len() < 3,
                                            connect_clicked => AppMsg::AttachImages,
                                        },
                                        gtk::Button {
                                            set_icon_name: "dn-smile-symbolic",
                                            set_tooltip_text: Some("Emoji"),
                                            add_css_class: "flat",
                                            add_css_class: "dn-composer-icon",
                                            #[watch]
                                            set_sensitive: !model.composer_preview,
                                            connect_clicked => AppMsg::PickEmoji,
                                        },
                                        gtk::Button {
                                            set_icon_name: "dn-chart-bar-symbolic",
                                            #[watch]
                                            set_tooltip_text: Some(model.poll_tooltip()),
                                            add_css_class: "flat",
                                            add_css_class: "dn-composer-icon",
                                            #[watch]
                                            set_sensitive: !model.composer_poll && model.composer_images.is_empty(),
                                            connect_clicked => AppMsg::TogglePoll,
                                        },
                                        gtk::Button {
                                            set_icon_name: "dn-clock-3-symbolic",
                                            #[watch]
                                            set_tooltip_text: Some(model.schedule_tooltip()),
                                            add_css_class: "flat",
                                            #[watch]
                                            set_css_classes: &model.composer_icon_classes(model.composer_schedule),
                                            connect_clicked => AppMsg::ToggleSchedule,
                                        },
                                        gtk::Button {
                                            set_icon_name: "dn-eye-symbolic",
                                            set_tooltip_text: Some("Preview"),
                                            add_css_class: "flat",
                                            #[watch]
                                            set_css_classes: &model.composer_icon_classes(model.composer_preview),
                                            connect_clicked => AppMsg::TogglePreview,
                                        },
                                        gtk::Box {
                                            set_hexpand: true,
                                        },
                                        gtk::Button {
                                            set_label: "Commit",
                                            add_css_class: "suggested-action",
                                            add_css_class: "dn-commit-btn",
                                            #[watch]
                                            set_sensitive: model.can_commit(),
                                            connect_clicked => AppMsg::SubmitCommit,
                                        },
                                    },
                                },
                            },
                            gtk::Overlay {
                                set_vexpand: true,
                                set_child: Some(&model.log_scroll),
                                add_overlay: &model.new_posts,
                            },
                        },

                        add_titled[Some("commit"), "Commit"] = &gtk::Box {
                            set_orientation: gtk::Orientation::Vertical,
                            gtk::Box {
                                set_orientation: gtk::Orientation::Horizontal,
                                set_spacing: 12,
                                add_css_class: "dn-commit-bar",
                                gtk::Button {
                                    set_icon_name: "dn-arrow-left-symbolic",
                                    set_tooltip_text: Some("Back"),
                                    add_css_class: "flat",
                                    add_css_class: "dn-commit-bar-btn",
                                    connect_clicked => AppMsg::Back,
                                },
                                gtk::Label {
                                    set_label: "Commit",
                                    add_css_class: "dn-commit-bar-title",
                                },
                                gtk::Label {
                                    #[watch]
                                    set_label: &model.commit_sha(),
                                    add_css_class: "dn-mono",
                                    add_css_class: "dn-muted",
                                    set_hexpand: true,
                                    set_xalign: 1.0,
                                },
                                gtk::Button {
                                    set_icon_name: "dn-link-symbolic",
                                    set_tooltip_text: Some("Copy link"),
                                    add_css_class: "flat",
                                    add_css_class: "dn-commit-bar-btn",
                                    #[watch]
                                    set_sensitive: model.open_commit.is_some(),
                                    connect_clicked => AppMsg::CopyCommitLink,
                                },
                            },
                            #[name = "commit_scroll"]
                            gtk::ScrolledWindow {
                                set_vexpand: true,
                                set_hscrollbar_policy: gtk::PolicyType::Never,
                                set_child: Some(&model.reply_list),
                            },
                            gtk::Box {
                                set_orientation: gtk::Orientation::Vertical,
                                add_css_class: "dn-composer",
                                gtk::ScrolledWindow {
                                    set_min_content_height: 60,
                                    set_max_content_height: 120,
                                    set_child: Some(&model.reply_composer),
                                },
                                gtk::Box {
                                    set_orientation: gtk::Orientation::Horizontal,
                                    set_spacing: 8,
                                    gtk::Button {
                                        set_icon_name: "dn-smile-symbolic",
                                        set_tooltip_text: Some("Emoji"),
                                        add_css_class: "flat",
                                        add_css_class: "dn-composer-icon",
                                        connect_clicked => AppMsg::PickEmoji,
                                    },
                                    gtk::Label {
                                        set_label: "Reply",
                                        add_css_class: "dn-muted",
                                        set_hexpand: true,
                                        set_xalign: 0.0,
                                    },
                                    gtk::Button {
                                        set_label: "Reply",
                                        add_css_class: "suggested-action",
                                        connect_clicked => AppMsg::SubmitReply,
                                    },
                                },
                            },
                        },

                        add_titled[Some("profile"), "Profile"] = &gtk::Box {
                            set_orientation: gtk::Orientation::Vertical,
                            set_hexpand: true,
                            add_css_class: "dn-profile-page",
                            gtk::Box {
                                set_orientation: gtk::Orientation::Horizontal,
                                set_spacing: 12,
                                add_css_class: "dn-commit-bar",
                                #[local_ref]
                                profile_bar_avatar -> gtk::Box {
                                    set_valign: gtk::Align::Center,
                                    add_css_class: "dn-profile-bar-avatar",
                                    #[watch]
                                    set_visible: model.profile_bar_scrolled,
                                },
                                gtk::Label {
                                    #[watch]
                                    set_label: &model.profile_bar_name(),
                                    add_css_class: "dn-commit-bar-title",
                                    set_xalign: 0.0,
                                    set_ellipsize: pango::EllipsizeMode::End,
                                    set_hexpand: true,
                                    set_width_chars: 1,
                                },
                                gtk::Label {
                                    #[watch]
                                    set_label: &model.profile_bar_handle(),
                                    add_css_class: "dn-handle",
                                    add_css_class: "dn-profile-bar-handle",
                                    #[watch]
                                    set_visible: model.profile_bar_scrolled,
                                },
                                gtk::Label {
                                    #[watch]
                                    set_label: &model.profile_bar_commits(),
                                    add_css_class: "dn-mono",
                                    add_css_class: "dn-muted",
                                    add_css_class: "dn-profile-bar-count",
                                    #[watch]
                                    set_visible: !model.profile_bar_scrolled,
                                },
                            },
                            #[name = "profile_scroll"]
                            gtk::ScrolledWindow {
                                set_vexpand: true,
                                set_hexpand: true,
                                set_hscrollbar_policy: gtk::PolicyType::Never,
                                set_child: Some(&model.profile_list),
                            },
                        },

                        add_titled[Some("search"), "Search"] = &gtk::Box {
                            set_orientation: gtk::Orientation::Vertical,
                            gtk::Box {
                                set_orientation: gtk::Orientation::Horizontal,
                                set_spacing: 12,
                                add_css_class: "dn-commit-bar",
                                gtk::Button {
                                    set_icon_name: "dn-arrow-left-symbolic",
                                    set_tooltip_text: Some("Back"),
                                    add_css_class: "flat",
                                    add_css_class: "dn-commit-bar-btn",
                                    connect_clicked => AppMsg::Back,
                                },
                                gtk::Label {
                                    set_label: "Search",
                                    add_css_class: "dn-commit-bar-title",
                                },
                            },
                            #[name = "search_scroll"]
                            gtk::ScrolledWindow {
                                set_vexpand: true,
                                set_hscrollbar_policy: gtk::PolicyType::Never,
                                set_child: Some(&model.search_list),
                            },
                        },

                        add_titled[Some("connections"), "Connections"] = &gtk::Box {
                            set_orientation: gtk::Orientation::Vertical,
                            gtk::Box {
                                set_orientation: gtk::Orientation::Horizontal,
                                set_spacing: 12,
                                add_css_class: "dn-commit-bar",
                                gtk::Button {
                                    set_icon_name: "dn-arrow-left-symbolic",
                                    set_tooltip_text: Some("Back"),
                                    add_css_class: "flat",
                                    add_css_class: "dn-commit-bar-btn",
                                    connect_clicked => AppMsg::Back,
                                },
                                gtk::Label {
                                    #[watch]
                                    set_label: &model.connections_title,
                                    add_css_class: "dn-commit-bar-title",
                                    set_xalign: 0.0,
                                    set_hexpand: true,
                                },
                            },
                            #[name = "connections_scroll"]
                            gtk::ScrolledWindow {
                                set_vexpand: true,
                                set_hscrollbar_policy: gtk::PolicyType::Never,
                                set_child: Some(&model.connections_list),
                            },
                        },

                        add_titled[Some("settings"), "Settings"] = &gtk::Box {
                            set_orientation: gtk::Orientation::Vertical,
                            gtk::Box {
                                set_orientation: gtk::Orientation::Horizontal,
                                add_css_class: "dn-commit-bar",
                                gtk::Label {
                                    set_label: "Settings",
                                    add_css_class: "dn-commit-bar-title",
                                    set_xalign: 0.0,
                                },
                            },
                            gtk::Box {
                                set_orientation: gtk::Orientation::Horizontal,
                                set_homogeneous: true,
                                set_hexpand: true,
                                add_css_class: "dn-tabs",
                                add_css_class: "dn-settings-tabs",
                                gtk::Button {
                                    set_label: "Profile",
                                    set_hexpand: true,
                                    #[watch]
                                    set_css_classes: &model.settings_tab_classes(SettingsTab::Profile),
                                    connect_clicked => AppMsg::SettingsTab(SettingsTab::Profile),
                                },
                                gtk::Button {
                                    set_label: "Appearance",
                                    set_hexpand: true,
                                    #[watch]
                                    set_css_classes: &model.settings_tab_classes(SettingsTab::Appearance),
                                    connect_clicked => AppMsg::SettingsTab(SettingsTab::Appearance),
                                },
                                gtk::Button {
                                    set_label: "Terminal",
                                    set_hexpand: true,
                                    #[watch]
                                    set_css_classes: &model.settings_tab_classes(SettingsTab::Terminal),
                                    connect_clicked => AppMsg::SettingsTab(SettingsTab::Terminal),
                                },
                                gtk::Button {
                                    set_label: "Safety",
                                    set_hexpand: true,
                                    #[watch]
                                    set_css_classes: &model.settings_tab_classes(SettingsTab::Safety),
                                    connect_clicked => AppMsg::SettingsTab(SettingsTab::Safety),
                                },
                                gtk::Button {
                                    set_label: "Devices",
                                    set_hexpand: true,
                                    #[watch]
                                    set_css_classes: &model.settings_tab_classes(SettingsTab::Devices),
                                    connect_clicked => AppMsg::SettingsTab(SettingsTab::Devices),
                                },
                            },
                            #[name = "settings_scroll"]
                            gtk::ScrolledWindow {
                                set_vexpand: true,
                                set_hscrollbar_policy: gtk::PolicyType::Never,
                                set_child: Some(&model.settings_box),
                            },
                        },

                        add_titled[Some("notifications"), "Notifications"] = &gtk::Box {
                            set_orientation: gtk::Orientation::Vertical,
                            set_hexpand: true,
                            add_css_class: "dn-notif-page",
                            gtk::Box {
                                set_orientation: gtk::Orientation::Vertical,
                                set_spacing: 12,
                                add_css_class: "dn-notif-header",
                                gtk::Label {
                                    set_label: "Notifications",
                                    add_css_class: "dn-name",
                                    set_xalign: 0.0,
                                },
                                gtk::FlowBox {
                                    set_selection_mode: gtk::SelectionMode::None,
                                    set_homogeneous: false,
                                    set_min_children_per_line: 1,
                                    set_max_children_per_line: 8,
                                    set_column_spacing: 8,
                                    set_row_spacing: 8,
                                    set_hexpand: true,
                                    add_css_class: "dn-notif-filters",
                                    gtk::Button {
                                        #[watch]
                                        set_css_classes: &model.notif_filter_classes(NotificationFilter::Replies),
                                        set_label: "Replies",
                                        connect_clicked => AppMsg::ToggleNotificationFilter(NotificationFilter::Replies),
                                    },
                                    gtk::Button {
                                        #[watch]
                                        set_css_classes: &model.notif_filter_classes(NotificationFilter::Quotes),
                                        set_label: "Quotes",
                                        connect_clicked => AppMsg::ToggleNotificationFilter(NotificationFilter::Quotes),
                                    },
                                    gtk::Button {
                                        #[watch]
                                        set_css_classes: &model.notif_filter_classes(NotificationFilter::Stars),
                                        set_label: "Stars",
                                        connect_clicked => AppMsg::ToggleNotificationFilter(NotificationFilter::Stars),
                                    },
                                    gtk::Button {
                                        #[watch]
                                        set_css_classes: &model.notif_filter_classes(NotificationFilter::Mentions),
                                        set_label: "Mentions",
                                        connect_clicked => AppMsg::ToggleNotificationFilter(NotificationFilter::Mentions),
                                    },
                                    gtk::Button {
                                        #[watch]
                                        set_css_classes: &model.notif_filter_classes(NotificationFilter::Follows),
                                        set_label: "Follows",
                                        connect_clicked => AppMsg::ToggleNotificationFilter(NotificationFilter::Follows),
                                    },
                                    gtk::Button {
                                        #[watch]
                                        set_css_classes: &model.notif_filter_classes(NotificationFilter::Moderation),
                                        set_label: "Moderation",
                                        connect_clicked => AppMsg::ToggleNotificationFilter(NotificationFilter::Moderation),
                                    },
                                },
                            },
                            #[name = "notif_scroll"]
                            gtk::ScrolledWindow {
                                set_vexpand: true,
                                set_hexpand: true,
                                set_propagate_natural_width: false,
                                set_hscrollbar_policy: gtk::PolicyType::Never,
                                set_child: Some(&model.notif_list),
                            },
                        },

                        add_titled[Some("remotes"), "Remotes"] = &gtk::Box {
                            set_orientation: gtk::Orientation::Horizontal,
                            set_hexpand: true,
                            set_vexpand: true,
                            add_css_class: "dn-remotes-page",
                            gtk::Box {
                                set_orientation: gtk::Orientation::Vertical,
                                set_hexpand: false,
                                set_vexpand: true,
                                set_width_request: 320,
                                add_css_class: "dn-remotes-inbox",
                                #[watch]
                                set_visible: model.remotes_inbox_visible(),
                                gtk::Box {
                                    set_orientation: gtk::Orientation::Horizontal,
                                    set_spacing: 2,
                                    add_css_class: "dn-inbox-header",
                                    gtk::Label {
                                        set_label: "Remotes",
                                        add_css_class: "dn-title",
                                        set_xalign: 0.0,
                                        set_hexpand: true,
                                        set_ellipsize: pango::EllipsizeMode::End,
                                        set_width_chars: 1,
                                    },
                                    gtk::Button {
                                        #[watch]
                                        set_css_classes: model.dock_toggle_classes(),
                                        set_tooltip_text: Some("Keep chat open on every page"),
                                        set_valign: gtk::Align::Center,
                                        #[watch]
                                        set_visible: !model.shell_compact,
                                        connect_clicked => AppMsg::ToggleChatDock,
                                        #[wrap(Some)]
                                        set_child = &gtk::Box {
                                            set_orientation: gtk::Orientation::Horizontal,
                                            set_spacing: 4,
                                            gtk::Image {
                                                set_icon_name: Some("dn-panel-bottom-open-symbolic"),
                                                set_pixel_size: 16,
                                            },
                                            gtk::Label {
                                                #[watch]
                                                set_label: if model.prefs.chat_docked { "Docked" } else { "Dock" },
                                            },
                                        },
                                    },
                                    #[local_ref]
                                    remote_settings -> gtk::MenuButton {},
                                    gtk::Button {
                                        add_css_class: "flat",
                                        add_css_class: "dn-icon-btn",
                                        set_icon_name: "dn-users-round-symbolic",
                                        set_tooltip_text: Some("New org"),
                                        connect_clicked => AppMsg::PickOrg,
                                    },
                                    gtk::Button {
                                        add_css_class: "flat",
                                        add_css_class: "dn-icon-btn",
                                        set_icon_name: "dn-square-pen-symbolic",
                                        set_tooltip_text: Some("New remote"),
                                        connect_clicked => AppMsg::PickRemote,
                                    },
                                },
                                gtk::Box {
                                    add_css_class: "dn-inbox-search",
                                    gtk::Entry {
                                        set_placeholder_text: Some("Search names or @handles"),
                                        set_hexpand: true,
                                        set_max_length: 100,
                                        add_css_class: "dn-inbox-search-entry",
                                        set_primary_icon_name: Some("dn-search-symbolic"),
                                        connect_changed[sender] => move |entry| {
                                            sender.input(AppMsg::InboxQuery(entry.text().to_string()));
                                        },
                                    },
                                },
                                gtk::Box {
                                    set_orientation: gtk::Orientation::Horizontal,
                                    set_homogeneous: true,
                                    set_hexpand: true,
                                    add_css_class: "dn-tabs",
                                    add_css_class: "dn-inbox-tabs",
                                    gtk::Button {
                                        set_label: "Open",
                                        set_hexpand: true,
                                        #[watch]
                                        set_css_classes: model.inbox_tab_classes(InboxTab::Open),
                                        connect_clicked => AppMsg::InboxTab(InboxTab::Open),
                                    },
                                    gtk::Button {
                                        #[watch]
                                        set_label: &model.fetched_tab_label(),
                                        set_hexpand: true,
                                        #[watch]
                                        set_visible: model.remotes_requests(),
                                        #[watch]
                                        set_css_classes: model.inbox_tab_classes(InboxTab::Fetched),
                                        connect_clicked => AppMsg::InboxTab(InboxTab::Fetched),
                                    },
                                    gtk::Button {
                                        set_label: "Stashed",
                                        set_hexpand: true,
                                        #[watch]
                                        set_css_classes: model.inbox_tab_classes(InboxTab::Stashed),
                                        connect_clicked => AppMsg::InboxTab(InboxTab::Stashed),
                                    },
                                },
                                gtk::ScrolledWindow {
                                    set_vexpand: true,
                                    set_hexpand: true,
                                    set_hscrollbar_policy: gtk::PolicyType::Never,
                                    set_child: Some(&model.inbox_list),
                                },
                            },
                            gtk::Box {
                                set_orientation: gtk::Orientation::Vertical,
                                set_hexpand: true,
                                set_vexpand: true,
                                add_css_class: "dn-remotes-thread",
                                #[watch]
                                set_visible: model.remotes_thread_visible(),
                                #[name = "thread_width_probe"]
                                gtk::DrawingArea {
                                    set_hexpand: true,
                                    set_content_height: 1,
                                    set_can_target: false,
                                    add_css_class: "dn-thread-width-probe",
                                },
                                #[local_ref]
                                thread_header -> gtk::Box {
                                    set_orientation: gtk::Orientation::Vertical,
                                    add_css_class: "dn-thread-header",
                                    #[watch]
                                    set_visible: model.thread_ref.is_some(),
                                },
                                gtk::CenterBox {
                                    set_orientation: gtk::Orientation::Vertical,
                                    set_hexpand: true,
                                    set_vexpand: true,
                                    add_css_class: "dn-thread-empty",
                                    #[watch]
                                    set_visible: model.thread_ref.is_none(),
                                    #[wrap(Some)]
                                    set_center_widget = &gtk::Box {
                                        set_orientation: gtk::Orientation::Vertical,
                                        set_spacing: 12,
                                        set_halign: gtk::Align::Center,
                                        set_valign: gtk::Align::Center,
                                        gtk::Label {
                                            set_label: "no conversation checked out",
                                            add_css_class: "dn-mono",
                                            add_css_class: "dn-muted",
                                            set_justify: gtk::Justification::Center,
                                            set_xalign: 0.5,
                                            set_halign: gtk::Align::Center,
                                        },
                                        gtk::Button {
                                            add_css_class: "dn-outline-btn",
                                            set_halign: gtk::Align::Center,
                                            connect_clicked => AppMsg::PickRemote,
                                            #[wrap(Some)]
                                            set_child = &gtk::Box {
                                                set_orientation: gtk::Orientation::Horizontal,
                                                set_spacing: 8,
                                                gtk::Image {
                                                    set_icon_name: Some("dn-square-pen-symbolic"),
                                                    set_pixel_size: 16,
                                                },
                                                gtk::Label {
                                                    set_label: "New remote",
                                                },
                                            },
                                        },
                                    },
                                },
                                #[local_ref]
                                thread_scroll -> gtk::ScrolledWindow {
                                    set_vexpand: true,
                                    set_hexpand: true,
                                    set_hscrollbar_policy: gtk::PolicyType::Never,
                                    add_css_class: "dn-thread-scroll",
                                    #[watch]
                                    set_visible: model.thread_ref.is_some(),
                                    set_child: Some(&model.thread_list),
                                },
                                gtk::Box {
                                    set_orientation: gtk::Orientation::Vertical,
                                    add_css_class: "dn-composer",
                                    add_css_class: "dn-remote-composer",
                                    #[watch]
                                    set_visible: model.thread_ref.is_some(),
                                    gtk::Label {
                                        #[watch]
                                        set_label: &model.thread_typing,
                                        add_css_class: "dn-mono",
                                        add_css_class: "dn-muted",
                                        add_css_class: "dn-thread-typing",
                                        set_xalign: 0.0,
                                        set_ellipsize: pango::EllipsizeMode::End,
                                        #[watch]
                                        set_visible: !model.thread_typing.is_empty(),
                                    },
                                    gtk::Box {
                                        set_orientation: gtk::Orientation::Horizontal,
                                        set_spacing: 8,
                                        add_css_class: "dn-remote-reply",
                                        #[watch]
                                        set_visible: model.remote_reply.is_some(),
                                        gtk::Box {
                                            set_orientation: gtk::Orientation::Vertical,
                                            set_hexpand: true,
                                            gtk::Label {
                                                #[watch]
                                                set_label: &model.remote_reply_author(),
                                                add_css_class: "dn-name",
                                                set_xalign: 0.0,
                                                set_ellipsize: pango::EllipsizeMode::End,
                                            },
                                            gtk::Label {
                                                #[watch]
                                                set_label: &model.remote_reply_snippet(),
                                                add_css_class: "dn-muted",
                                                set_xalign: 0.0,
                                                set_ellipsize: pango::EllipsizeMode::End,
                                            },
                                        },
                                        gtk::Button {
                                            add_css_class: "flat",
                                            add_css_class: "dn-icon-btn",
                                            set_icon_name: "dn-x-symbolic",
                                            set_tooltip_text: Some("Cancel reply"),
                                            set_valign: gtk::Align::Center,
                                            connect_clicked => AppMsg::CancelRemoteReply,
                                        },
                                    },
                                    gtk::Box {
                                        set_orientation: gtk::Orientation::Horizontal,
                                        set_spacing: 8,
                                        add_css_class: "dn-remote-row",
                                        set_valign: gtk::Align::Fill,
                                        gtk::Button {
                                            add_css_class: "flat",
                                            add_css_class: "dn-icon-btn",
                                            add_css_class: "dn-composer-icon",
                                            set_icon_name: "dn-image-plus-symbolic",
                                            set_tooltip_text: Some("Attach up to 3 images"),
                                            set_valign: gtk::Align::End,
                                            set_size_request: (40, 40),
                                            #[watch]
                                            set_sensitive: model.remote_images.len() < 3,
                                            connect_clicked => AppMsg::AttachRemoteImages,
                                        },
                                        gtk::Button {
                                            add_css_class: "flat",
                                            add_css_class: "dn-icon-btn",
                                            add_css_class: "dn-composer-icon",
                                            set_icon_name: "dn-smile-symbolic",
                                            set_tooltip_text: Some("Emoji"),
                                            set_valign: gtk::Align::End,
                                            set_size_request: (40, 40),
                                            connect_clicked => AppMsg::PickEmoji,
                                        },
                                        gtk::Button {
                                            add_css_class: "flat",
                                            add_css_class: "dn-icon-btn",
                                            add_css_class: "dn-composer-icon",
                                            set_icon_name: "dn-mic-symbolic",
                                            set_tooltip_text: Some("Voice note"),
                                            set_valign: gtk::Align::End,
                                            set_size_request: (40, 40),
                                            connect_clicked => AppMsg::RecordVoice,
                                        },
                                        gtk::Button {
                                            add_css_class: "flat",
                                            add_css_class: "dn-icon-btn",
                                            add_css_class: "dn-composer-icon",
                                            set_icon_name: "dn-megaphone-symbolic",
                                            set_tooltip_text: Some("Ping everyone in the org"),
                                            set_valign: gtk::Align::End,
                                            set_size_request: (40, 40),
                                            connect_clicked => AppMsg::PingMention,
                                        },
                                        gtk::Button {
                                            add_css_class: "flat",
                                            add_css_class: "dn-icon-btn",
                                            add_css_class: "dn-composer-icon",
                                            set_icon_name: "dn-phone-symbolic",
                                            set_tooltip_text: Some("Call"),
                                            set_valign: gtk::Align::End,
                                            set_size_request: (40, 40),
                                            connect_clicked => AppMsg::StartCall,
                                        },
                                        gtk::Overlay {
                                            add_css_class: "dn-remote-input",
                                            set_hexpand: true,
                                            set_valign: gtk::Align::End,
                                            #[wrap(Some)]
                                            set_child = &gtk::ScrolledWindow {
                                                set_hexpand: true,
                                                set_min_content_height: 40,
                                                set_max_content_height: 120,
                                                set_propagate_natural_height: true,
                                                set_hscrollbar_policy: gtk::PolicyType::Never,
                                                set_vscrollbar_policy: gtk::PolicyType::Automatic,
                                                set_child: Some(&model.remote_composer),
                                            },
                                            add_overlay = &gtk::Label {
                                                #[watch]
                                                set_label: &model.remote_placeholder(),
                                                #[watch]
                                                set_visible: model.remote_draft_len == 0,
                                                add_css_class: "dn-remote-placeholder",
                                                set_halign: gtk::Align::Fill,
                                                set_valign: gtk::Align::Center,
                                                set_xalign: 0.0,
                                                set_can_target: false,
                                            },
                                        },
                                        gtk::Button {
                                            set_label: "Push",
                                            add_css_class: "suggested-action",
                                            add_css_class: "dn-push-btn",
                                            set_valign: gtk::Align::End,
                                            set_vexpand: false,
                                            set_size_request: (80, 40),
                                            #[watch]
                                            set_sensitive: model.can_push_remote(),
                                            connect_clicked => AppMsg::PushRemote,
                                        },
                                    },
                                    #[local_ref]
                                    remote_image_previews -> gtk::Box {
                                        set_orientation: gtk::Orientation::Horizontal,
                                        set_spacing: 8,
                                        add_css_class: "dn-attach-previews",
                                        #[watch]
                                        set_visible: !model.remote_images.is_empty(),
                                    },
                                },
                            },
                        },

                        #[watch]
                        set_visible_child_name: model.visible,
                    },
                    },
                    },

                    #[name = "aside_pane"]
                    gtk::Box {
                        set_orientation: gtk::Orientation::Vertical,
                        set_width_request: 320,
                        set_hexpand: false,
                        set_vexpand: true,
                        add_css_class: "dn-aside-scroll",
                        #[watch]
                        set_visible: model.aside_visible(),

                        gtk::ScrolledWindow {
                            set_vexpand: true,
                            set_hexpand: true,
                            set_hscrollbar_policy: gtk::PolicyType::Never,
                            #[wrap(Some)]
                            set_child = &gtk::Box {
                                set_orientation: gtk::Orientation::Vertical,
                                set_spacing: 16,
                                add_css_class: "dn-aside",

                                gtk::Button {
                                    add_css_class: "dn-search-trigger",
                                    set_tooltip_text: Some("Search"),
                                    connect_clicked => AppMsg::FocusSearch,
                                    #[wrap(Some)]
                                    set_child = &gtk::Box {
                                        set_orientation: gtk::Orientation::Horizontal,
                                        set_spacing: 10,
                                        set_valign: gtk::Align::Center,
                                        gtk::Image {
                                            set_icon_name: Some("dn-search-symbolic"),
                                            set_pixel_size: 16,
                                            set_valign: gtk::Align::Center,
                                        },
                                        gtk::Label {
                                            set_label: "Search",
                                            set_xalign: 0.0,
                                            set_hexpand: true,
                                            set_valign: gtk::Align::Center,
                                        },
                                        gtk::Label {
                                            set_label: "Ctrl+K",
                                            add_css_class: "dn-kbd",
                                            set_valign: gtk::Align::Center,
                                        },
                                    },
                                },

                                gtk::Box {
                                    set_orientation: gtk::Orientation::Vertical,
                                    set_spacing: 8,
                                    add_css_class: "dn-hot-topics",
                                    #[watch]
                                    set_visible: model.user.is_some() && !model.hot_topics.is_empty(),
                                    gtk::Box {
                                        set_orientation: gtk::Orientation::Horizontal,
                                        set_spacing: 6,
                                        gtk::Image {
                                            set_icon_name: Some("dn-flame-symbolic"),
                                            set_pixel_size: 16,
                                            set_valign: gtk::Align::Center,
                                        },
                                        gtk::Label {
                                            set_label: "Hot topics",
                                            set_xalign: 0.0,
                                            add_css_class: "dn-aside-title",
                                            set_hexpand: true,
                                        },
                                    },
                                    #[local_ref]
                                    hot_topics_list -> gtk::Box {
                                        set_orientation: gtk::Orientation::Vertical,
                                        set_spacing: 2,
                                    },
                                },

                                gtk::Box {
                                    set_orientation: gtk::Orientation::Vertical,
                                    set_spacing: 6,
                                    add_css_class: "dn-live-card",
                                    #[watch]
                                    set_visible: model.user.is_some() && !model.live_following.is_empty(),
                                    gtk::Label {
                                        set_label: "Live now",
                                        set_xalign: 0.0,
                                        add_css_class: "dn-aside-title",
                                    },
                                    #[local_ref]
                                    live_list -> gtk::Box {
                                        set_orientation: gtk::Orientation::Vertical,
                                        set_spacing: 2,
                                    },
                                },

                                gtk::Box {
                                    set_orientation: gtk::Orientation::Vertical,
                                    set_spacing: 8,
                                    add_css_class: "dn-aside-follow",
                                    #[watch]
                                    set_visible: model.prefs.show_who_to_follow,
                                    gtk::Box {
                                        set_orientation: gtk::Orientation::Horizontal,
                                        set_spacing: 4,
                                        gtk::Label {
                                            set_label: "Who to follow",
                                            set_xalign: 0.0,
                                            add_css_class: "dn-aside-title",
                                            set_hexpand: true,
                                        },
                                        gtk::Button {
                                            set_icon_name: "dn-refresh-cw-symbolic",
                                            set_tooltip_text: Some("Suggest other people"),
                                            add_css_class: "flat",
                                            add_css_class: "dn-aside-icon",
                                            connect_clicked => AppMsg::RefreshSuggested,
                                        },
                                        gtk::MenuButton {
                                            set_icon_name: "dn-ellipsis-symbolic",
                                            set_tooltip_text: Some("Who to follow options"),
                                            add_css_class: "flat",
                                            add_css_class: "dn-aside-icon",
                                            set_always_show_arrow: false,
                                            set_has_frame: false,
                                            #[wrap(Some)]
                                            set_popover = &gtk::Popover {
                                                set_position: gtk::PositionType::Bottom,
                                                gtk::Button {
                                                    add_css_class: "flat",
                                                    add_css_class: "dn-aside-menu-item",
                                                    connect_clicked => AppMsg::SetWhoToFollow(false),
                                                    #[wrap(Some)]
                                                    set_child = &gtk::Box {
                                                        set_orientation: gtk::Orientation::Horizontal,
                                                        set_spacing: 10,
                                                        gtk::Image {
                                                            set_icon_name: Some("dn-eye-off-symbolic"),
                                                            set_pixel_size: 16,
                                                            set_valign: gtk::Align::Start,
                                                        },
                                                        gtk::Box {
                                                            set_orientation: gtk::Orientation::Vertical,
                                                            set_spacing: 2,
                                                            gtk::Label {
                                                                set_label: "Don't show again",
                                                                set_xalign: 0.0,
                                                            },
                                                            gtk::Label {
                                                                set_label: "> /dev/null",
                                                                set_xalign: 0.0,
                                                                add_css_class: "dn-mono",
                                                                add_css_class: "dn-muted",
                                                            },
                                                        },
                                                    },
                                                },
                                            },
                                        },
                                    },
                                    #[local_ref]
                                    suggested_list -> gtk::Box {
                                        set_orientation: gtk::Orientation::Vertical,
                                        set_spacing: 8,
                                    },
                                },

                                gtk::FlowBox {
                                    add_css_class: "dn-aside-legal",
                                    set_selection_mode: gtk::SelectionMode::None,
                                    set_homogeneous: false,
                                    set_column_spacing: 12,
                                    set_row_spacing: 4,
                                    set_max_children_per_line: 3,
                                    set_min_children_per_line: 2,
                                    set_halign: gtk::Align::Start,
                                    append = &gtk::Button {
                                        set_label: "About",
                                        add_css_class: "flat",
                                        add_css_class: "dn-aside-link",
                                        connect_clicked => AppMsg::OpenWeb(format!("{}/about", devnull_api::default_base_url())),
                                    },
                                    append = &gtk::Button {
                                        set_label: "Partners",
                                        add_css_class: "flat",
                                        add_css_class: "dn-aside-link",
                                        connect_clicked => AppMsg::OpenWeb(format!("{}/partners", devnull_api::default_base_url())),
                                    },
                                    append = &gtk::Button {
                                        set_label: "Apps",
                                        add_css_class: "flat",
                                        add_css_class: "dn-aside-link",
                                        connect_clicked => AppMsg::OpenWeb(format!("{}/apps", devnull_api::default_base_url())),
                                    },
                                    append = &gtk::Button {
                                        set_label: "Changelog",
                                        add_css_class: "flat",
                                        add_css_class: "dn-aside-link",
                                        connect_clicked => AppMsg::OpenWeb(format!("{}/changelog", devnull_api::default_base_url())),
                                    },
                                    append = &gtk::Button {
                                        set_label: "Terms",
                                        add_css_class: "flat",
                                        add_css_class: "dn-aside-link",
                                        connect_clicked => AppMsg::OpenWeb(format!("{}/terms", devnull_api::default_base_url())),
                                    },
                                    append = &gtk::Button {
                                        set_label: "Privacy",
                                        add_css_class: "flat",
                                        add_css_class: "dn-aside-link",
                                        connect_clicked => AppMsg::OpenWeb(format!("{}/privacy", devnull_api::default_base_url())),
                                    },
                                },
                            },
                        },

                        gtk::Box {
                            set_orientation: gtk::Orientation::Vertical,
                            set_spacing: 8,
                            add_css_class: "dn-aside-links",
                            gtk::Button {
                                add_css_class: "flat",
                                add_css_class: "dn-aside-link",
                                set_halign: gtk::Align::Start,
                                connect_clicked => AppMsg::ShowShortcuts,
                                #[wrap(Some)]
                                set_child = &gtk::Box {
                                    set_orientation: gtk::Orientation::Horizontal,
                                    set_spacing: 8,
                                    set_valign: gtk::Align::Center,
                                    gtk::Label {
                                        set_label: "Keyboard shortcuts",
                                        set_xalign: 0.0,
                                    },
                                    gtk::Label {
                                        set_label: "?",
                                        add_css_class: "dn-kbd",
                                        set_valign: gtk::Align::Center,
                                    },
                                },
                            },
                        },
                    },

                    gtk::Box {
                        set_hexpand: true,
                        add_css_class: "dn-gutter",
                        #[watch]
                        set_visible: !model.feed_fills(),
                    },
            }
        }
        }
    }
    }

    fn init(
        _init: Self::Init,
        root: Self::Root,
        sender: ComponentSender<Self>,
    ) -> ComponentParts<Self> {
        let _ = APP_SENDER.set(sender.input_sender().clone());

        ui::register_icons();
        crate::macos::set_dock_icon();

        let composer = gtk::TextView::new();
        composer.set_wrap_mode(gtk::WrapMode::WordChar);
        composer.add_css_class("dn-composer-text");
        composer.set_top_margin(6);
        composer.set_left_margin(2);

        let reply_composer = gtk::TextView::new();
        reply_composer.set_wrap_mode(gtk::WrapMode::WordChar);

        let inline_reply = gtk::TextView::new();
        inline_reply.set_wrap_mode(gtk::WrapMode::WordChar);
        inline_reply.add_css_class("dn-composer-text");

        let remote_composer = gtk::TextView::new();
        remote_composer.set_wrap_mode(gtk::WrapMode::WordChar);
        remote_composer.add_css_class("dn-composer-text");
        remote_composer.set_top_margin(0);
        remote_composer.set_bottom_margin(0);
        remote_composer.set_left_margin(0);
        remote_composer.set_right_margin(0);
        remote_composer.set_pixels_above_lines(0);
        remote_composer.set_pixels_below_lines(0);

        let emoji = ui::EmojiSuggest::new(sender.input_sender().clone());
        let mentions = ui::MentionSuggest::new(sender.input_sender().clone());
        let emoji_open = emoji.open_flag();
        let mention_open = mentions.open_flag();
        bind_composer_keys(
            &composer,
            sender.clone(),
            emoji_open.clone(),
            mention_open.clone(),
            ComposerBind::Log,
        );
        bind_composer_keys(
            &reply_composer,
            sender.clone(),
            emoji_open.clone(),
            mention_open.clone(),
            ComposerBind::Reply,
        );
        bind_composer_keys(
            &inline_reply,
            sender.clone(),
            emoji_open.clone(),
            mention_open.clone(),
            ComposerBind::Reply,
        );
        bind_composer_keys(
            &remote_composer,
            sender.clone(),
            emoji_open,
            mention_open,
            ComposerBind::Remote,
        );

        let paste_entry = gtk::Entry::new();
        paste_entry.connect_changed({
            let sender = sender.clone();
            move |entry| {
                if auth::extract_callback(&entry.text()).is_some() {
                    sender.input(AppMsg::PasteCallback);
                }
            }
        });

        let log_list = gtk::ListBox::new();
        log_list.set_selection_mode(gtk::SelectionMode::None);
        fill_feed_list(&log_list);
        let log_scroll = gtk::ScrolledWindow::new();
        log_scroll.set_vexpand(true);
        log_scroll.set_hscrollbar_policy(gtk::PolicyType::Never);
        log_scroll.set_child(Some(&log_list));
        let new_posts = gtk::Button::new();
        new_posts.add_css_class("dn-new-posts");
        new_posts.set_valign(gtk::Align::Start);
        new_posts.set_halign(gtk::Align::Center);
        new_posts.set_margin_top(12);
        new_posts.set_visible(false);
        new_posts.set_can_focus(false);
        new_posts.set_cursor_from_name(Some("pointer"));
        let reply_list = gtk::ListBox::new();
        reply_list.set_selection_mode(gtk::SelectionMode::None);
        fill_feed_list(&reply_list);
        let profile_list = gtk::ListBox::new();
        profile_list.set_selection_mode(gtk::SelectionMode::None);
        fill_feed_list(&profile_list);
        let search_list = gtk::ListBox::new();
        search_list.set_selection_mode(gtk::SelectionMode::None);
        fill_feed_list(&search_list);
        let connections_list = gtk::ListBox::new();
        connections_list.set_selection_mode(gtk::SelectionMode::None);
        fill_feed_list(&connections_list);
        let notif_list = gtk::ListBox::new();
        notif_list.set_selection_mode(gtk::SelectionMode::None);
        fill_feed_list(&notif_list);
        let inbox_list = gtk::ListBox::new();
        inbox_list.set_selection_mode(gtk::SelectionMode::None);
        inbox_list.set_activate_on_single_click(true);
        fill_feed_list(&inbox_list);
        let thread_list = gtk::ListBox::new();
        thread_list.set_selection_mode(gtk::SelectionMode::None);
        thread_list.add_css_class("dn-thread-list");
        fill_feed_list(&thread_list);
        let thread_scroll = gtk::ScrolledWindow::new();
        thread_scroll.set_vexpand(true);
        thread_scroll.set_hexpand(true);
        thread_scroll.set_hscrollbar_policy(gtk::PolicyType::Never);
        thread_scroll.add_css_class("dn-thread-scroll");
        thread_scroll.set_child(Some(&thread_list));
        let thread_follow = Rc::new(Cell::new(true));
        ui::bind_follow_end(&thread_scroll, thread_follow.clone());
        let poll_ui = ui::PollComposer::new(sender.input_sender().clone());
        let schedule_ui = ui::ScheduleCard::new();
        let poll_editor = poll_ui.root.clone();
        let schedule_card = schedule_ui.root.clone();
        let (compose_dialog, compose_host) = ui::compose_dialog();
        let prefs = Prefs::load();
        let hover_enabled = Rc::new(Cell::new(prefs.highlight_on_hover));
        let user_card = ui::UserCard::new();
        let remote_settings = ui::remote_settings_button(sender.input_sender().clone());
        let chat_dock = ui::ChatDock::new(sender.input_sender().clone());
        let people_picker = ui::PeoplePicker::new(sender.input_sender().clone());

        let mut model = App {
            phase: Phase::Loading,
            visible: "sign_in",
            user: None,
            client: None,
            error: None,
            pending: None,
            nav: Nav::Latest,
            log_cursor: None,
            profile: None,
            organization: None,
            profile_tab: ProfileFeed::Commits,
            profile_readme: None,
            suggested: Vec::new(),
            following: HashSet::new(),
            user_settings: None,
            translations: HashMap::new(),
            showing_original: HashSet::new(),
            requested_translation: HashSet::new(),
            translation_loading: HashSet::new(),
            auto_translate_retry_at: None,
            live_following: Vec::new(),
            hot_topics: Vec::new(),
            muted: Vec::new(),
            blocked: Vec::new(),
            settings_tab: SettingsTab::Profile,
            draft_lane: None,
            prefs,
            open_commit: None,
            ancestors: Vec::new(),
            related_commits: Vec::new(),
            thread_replies: Vec::new(),
            nested_replies: HashMap::new(),
            expanded_replies: HashSet::new(),
            previewed_replies: HashSet::new(),
            preview_loading: HashSet::new(),
            log_commits: Vec::new(),
            profile_commits: Vec::new(),
            log_first_seen: None,
            log_loading_more: false,
            replying_to: None,
            inline_reply: inline_reply.clone(),
            inline_reply_box: ui::inline_reply_box(&inline_reply, sender.input_sender().clone()),
            focus_reply: false,
            notif_types: NotificationFilter::enabled_all(),
            notifications: Vec::new(),
            notif_groups: HashMap::new(),
            busy: 0,
            thread_ref: None,
            composer_images: Vec::new(),
            remote_images: Vec::new(),
            unread_notifications: 0,
            unread_remotes: 0,
            counts_ready: false,
            remote_arrivals: HashMap::new(),
            last_thread_ping: None,
            composer_len: 0,
            composer_poll: false,
            composer_schedule: false,
            composer_preview: false,
            sidebar_collapsed: None,
            nav_compact: false,
            aside_compact: false,
            shell_compact: false,
            log_last_day: None,
            log_last_lane: None,
            log_visit_at: None,
            log_seen_id: None,
            log_away: false,
            new_posts_count: 0,
            new_authors: Vec::new(),
            log_scroll,
            log_composer: gtk::Box::new(gtk::Orientation::Horizontal, 0),
            new_posts,
            aside_toast: gtk::Revealer::new(),
            aside_toast_label: gtk::Label::new(None),
            toast_commit: Rc::new(RefCell::new(None)),
            toast_hide: Rc::new(RefCell::new(None)),
            stars: HashMap::new(),
            pasteis: HashMap::new(),
            composer_avatar: gtk::Box::new(gtk::Orientation::Horizontal, 0),
            author_menu: gtk::Popover::new(),
            composer_as: None,
            composer_rail: gtk::Overlay::new(),
            composer_rail_area: gtk::DrawingArea::new(),
            staged_paint: Rc::new(RefCell::new(ui::StagedPaint::new())),
            nav_avatar: gtk::Box::new(gtk::Orientation::Horizontal, 0),
            header_avatar: gtk::Box::new(gtk::Orientation::Horizontal, 0),
            wordmark_btn: gtk::Button::new(),
            logo_menu: ui::logo_menu(sender.input_sender().clone()),
            user_menu: ui::UserMenu::new(sender.input_sender().clone()),
            header_user_menu: ui::UserMenu::for_header(sender.input_sender().clone()),
            hover_enabled,
            user_card,
            user_preview_loading: HashSet::new(),
            log_list,
            reply_list,
            profile_list,
            search_list,
            connections_list,
            notif_list,
            inbox_list,
            thread_list,
            thread_scroll,
            thread_follow,
            composer,
            reply_composer,
            remote_composer,
            emoji,
            mentions,
            mention_gen: 0,
            mention_timeout: Rc::new(RefCell::new(None)),
            paste_entry,
            omnibox: ui::SearchOmnibox::new(sender.input_sender().clone()),
            omnibox_gen: 0,
            profile_hero: gtk::Box::new(gtk::Orientation::Vertical, 0),
            profile_bar_avatar: gtk::Box::new(gtk::Orientation::Horizontal, 0),
            profile_bar_scrolled: false,
            suggested_list: gtk::Box::new(gtk::Orientation::Vertical, 8),
            live_list: gtk::Box::new(gtk::Orientation::Vertical, 2),
            hot_topics_list: gtk::Box::new(gtk::Orientation::Vertical, 2),
            settings_box: gtk::Box::new(gtk::Orientation::Vertical, 0),
            poll_ui,
            schedule_ui,
            poll_editor,
            schedule_card,
            image_previews: gtk::Box::new(gtk::Orientation::Horizontal, 8),
            remote_image_previews: gtk::Box::new(gtk::Orientation::Horizontal, 8),
            compose_dialog,
            compose_host,
            compose_open: false,
            connections_kind: ConnectionsKind::Following,
            connections_handle: String::new(),
            connections_commit: None,
            connections_star_count: 0,
            connections_title: "Following".into(),
            connections_users: Vec::new(),
            connections_orgs: Vec::new(),
            connections_cursor: None,
            connections_orgs_cursor: None,
            connections_orgs_expanded: false,
            connections_loading: false,
            inbox: Vec::new(),
            inbox_tab: InboxTab::Open,
            inbox_requests: 0,
            remote_draft_len: 0,
            remote_reply: None,
            thread_typing: String::new(),
            last_typing_ping: None,
            open_thread: None,
            thread_gen: 0,
            thread_header_stamp: String::new(),
            inbox_query: String::new(),
            inbox_hits: None,
            inbox_people: Vec::new(),
            inbox_search_gen: 0,
            remote_settings,
            thread_header: gtk::Box::new(gtk::Orientation::Vertical, 0),
            chat_dock,
            chat_dock_expanded: false,
            people_picker,
            people_gen: 0,
            bubble_width: ui::BubbleWidth::default(),
        };

        model.omnibox.window.set_transient_for(Some(&root));
        model.people_picker.window.set_transient_for(Some(&root));
        let paste_entry = &model.paste_entry;
        let suggested_list = &model.suggested_list;
        let live_list = &model.live_list;
        let poll_editor = &model.poll_editor;
        let profile_bar_avatar = &model.profile_bar_avatar;
        let schedule_card = &model.schedule_card;
        let image_previews = &model.image_previews;
        let remote_image_previews = &model.remote_image_previews;
        let log_composer = &model.log_composer;
        let composer_rail = &model.composer_rail;
        let hot_topics_list = &model.hot_topics_list;
        let nav_avatar = &model.nav_avatar;
        let header_avatar = &model.header_avatar;
        let wordmark_btn = &model.wordmark_btn;
        let remote_settings = &model.remote_settings;
        let thread_header = &model.thread_header;
        let thread_scroll = &model.thread_scroll;
        let widgets = view_output!();
        crate::macos::decorate_window(&root);
        model.bubble_width.bind(&widgets.thread_width_probe);
        ui::pin_profile_hero(&model.profile_list, &model.profile_hero);
        widgets
            .connections_scroll
            .vadjustment()
            .connect_value_changed({
                let sender = sender.clone();
                move |adj| {
                    if adj.value() + adj.page_size() >= adj.upper() - 80.0 {
                        sender.input(AppMsg::LoadMoreConnections);
                    }
                }
            });
        widgets.profile_scroll.vadjustment().connect_value_changed({
            let sender = sender.clone();
            move |adj| {
                sender.input(AppMsg::ProfileBarScrolled(adj.value() > 72.0));
            }
        });
        ui::apply_pointer_cursors(&root);
        model.logo_menu.set_parent(&model.wordmark_btn);
        let logo_press = gtk::GestureClick::new();
        logo_press.set_button(gtk::gdk::BUTTON_SECONDARY);
        let logo_menu = model.logo_menu.clone();
        logo_press.connect_pressed(move |gesture, _, _, _| {
            gesture.set_state(gtk::EventSequenceState::Claimed);
            logo_menu.popup();
        });
        model.wordmark_btn.add_controller(logo_press);
        let avatar_press = gtk::GestureClick::new();
        avatar_press.set_button(1);
        let avatar_sender = sender.clone();
        avatar_press.connect_released(move |_, _, _, _| {
            avatar_sender.input(AppMsg::ComposerAvatar);
        });
        model.composer_avatar.add_css_class("dn-composer-avatar");
        model
            .composer_avatar
            .add_css_class("dn-composer-avatar-btn");
        model.composer_avatar.set_halign(gtk::Align::Center);
        model.composer_avatar.set_valign(gtk::Align::Start);
        model
            .composer_avatar
            .set_margin_top(ui::staged_avatar_top());
        model.composer_avatar.set_size_request(40, 40);
        model.composer_avatar.set_hexpand(false);
        model.composer_avatar.set_vexpand(false);
        model.composer_avatar.set_tooltip_text(Some("Your profile"));
        model.composer_avatar.set_visible(false);
        model.composer_avatar.add_controller(avatar_press);
        model.composer_avatar.set_cursor_from_name(Some("pointer"));
        model.author_menu.set_parent(&model.composer_avatar);
        model.author_menu.set_autohide(true);
        model.author_menu.set_has_arrow(true);
        model.author_menu.set_position(gtk::PositionType::Bottom);
        model.composer_rail_area = ui::bind_staged_rail(&model.composer_rail, &model.staged_paint);
        model.composer_rail.set_vexpand(false);
        model.composer_rail.add_overlay(&model.composer_avatar);
        model.aside_toast_label.add_css_class("dn-mono");
        model.aside_toast_label.set_wrap(true);
        model.aside_toast_label.set_xalign(0.0);
        model.aside_toast_label.set_max_width_chars(36);
        let toast_card = gtk::Box::new(gtk::Orientation::Horizontal, 0);
        toast_card.add_css_class("dn-aside-toast");
        toast_card.append(&model.aside_toast_label);
        let toast_press = gtk::GestureClick::new();
        toast_press.set_button(1);
        let toast_commit = model.toast_commit.clone();
        let toast_host = model.aside_toast.clone();
        let toast_hide = model.toast_hide.clone();
        let toast_sender = sender.clone();
        toast_press.connect_released(move |_, _, _, _| {
            if let Some(id) = toast_commit.borrow().clone() {
                toast_host.remove_css_class("visible");
                toast_host.set_reveal_child(false);
                if let Some(src) = toast_hide.borrow_mut().take() {
                    src.remove();
                }
                toast_sender.input(AppMsg::OpenCommit(id));
            }
        });
        toast_card.add_controller(toast_press);
        model.aside_toast.set_child(Some(&toast_card));
        model.aside_toast.add_css_class("dn-aside-toast-host");
        model
            .aside_toast
            .set_transition_type(gtk::RevealerTransitionType::SlideUp);
        model.aside_toast.set_transition_duration(220);
        model.aside_toast.set_reveal_child(false);
        model.aside_toast.set_halign(gtk::Align::End);
        model.aside_toast.set_valign(gtk::Align::End);
        model.aside_toast.set_margin_end(20);
        model.aside_toast.set_margin_bottom(20);
        model.aside_toast.set_margin_start(20);
        model.aside_toast.set_overflow(gtk::Overflow::Visible);
        let log_vscroll = gtk::Scrollbar::new(gtk::Orientation::Vertical, None::<&gtk::Adjustment>);
        widgets.shell_overlay.add_overlay(&log_vscroll);
        ui::bind_window_scroll(
            &log_vscroll,
            &widgets.stack,
            &widgets.nav_sidebar,
            &widgets.aside_pane,
            &[
                ("log", model.log_scroll.clone()),
                ("commit", widgets.commit_scroll.clone()),
                ("profile", widgets.profile_scroll.clone()),
                ("search", widgets.search_scroll.clone()),
                ("connections", widgets.connections_scroll.clone()),
                ("settings", widgets.settings_scroll.clone()),
                ("notifications", widgets.notif_scroll.clone()),
            ],
        );
        widgets.shell_overlay.add_overlay(&model.chat_dock.root);
        widgets.shell_overlay.add_overlay(&model.aside_toast);
        ui::pin_log_composer(&model.log_list, &model.log_composer);
        model.compose_dialog.connect_closed({
            let sender = sender.clone();
            move |_| sender.input(AppMsg::ComposeClosed)
        });
        model.log_composer.connect_visible_notify(|composer| {
            if let Some(parent) = composer.parent() {
                parent.set_visible(composer.is_visible());
            }
        });
        model.log_scroll.vadjustment().connect_value_changed({
            let sender = sender.clone();
            move |_| sender.input(AppMsg::LogScrolled)
        });
        model.new_posts.connect_clicked({
            let sender = sender.clone();
            move |_| sender.input(AppMsg::JumpToNewest)
        });
        relm4::adw::StyleManager::default().connect_dark_notify({
            let sender = sender.clone();
            move |_| sender.input(AppMsg::AppearanceChanged)
        });

        let shortcuts = gtk::EventControllerKey::new();
        shortcuts.set_propagation_phase(gtk::PropagationPhase::Capture);
        let shortcut_sender = sender.clone();
        let root_for_keys = root.clone();
        let awaiting_go = Rc::new(Cell::new(false));
        shortcuts.connect_key_pressed(move |_, key, _, mods| {
            let ctrl = mods.contains(gtk::gdk::ModifierType::CONTROL_MASK)
                || mods.contains(gtk::gdk::ModifierType::META_MASK);
            let shift = mods.contains(gtk::gdk::ModifierType::SHIFT_MASK);
            let alt = mods.contains(gtk::gdk::ModifierType::ALT_MASK);
            if ctrl && !shift && !alt && (key == gtk::gdk::Key::k || key == gtk::gdk::Key::K) {
                shortcut_sender.input(AppMsg::FocusSearch);
                return glib::Propagation::Stop;
            }
            let typing = gtk::prelude::RootExt::focus(&root_for_keys)
                .is_some_and(|widget| is_text_input(&widget));
            if typing || alt {
                return glib::Propagation::Proceed;
            }
            if awaiting_go.get() {
                awaiting_go.set(false);
                let dest = match key {
                    gtk::gdk::Key::l | gtk::gdk::Key::L => Some(AppMsg::Select(Nav::Latest)),
                    gtk::gdk::Key::e | gtk::gdk::Key::E => Some(AppMsg::Select(Nav::Trending)),
                    gtk::gdk::Key::n | gtk::gdk::Key::N => Some(AppMsg::Select(Nav::Notifications)),
                    gtk::gdk::Key::m | gtk::gdk::Key::M => Some(AppMsg::Select(Nav::Remotes)),
                    gtk::gdk::Key::asterisk | gtk::gdk::Key::KP_Multiply => {
                        Some(AppMsg::Select(Nav::Starred))
                    }
                    gtk::gdk::Key::p | gtk::gdk::Key::P => Some(AppMsg::OpenMe),
                    gtk::gdk::Key::s | gtk::gdk::Key::S => Some(AppMsg::OpenSettings),
                    _ => None,
                };
                if let Some(msg) = dest {
                    shortcut_sender.input(msg);
                    return glib::Propagation::Stop;
                }
                return glib::Propagation::Proceed;
            }
            if ctrl {
                return glib::Propagation::Proceed;
            }
            let action = match key {
                gtk::gdk::Key::j | gtk::gdk::Key::J => Some(AppMsg::MoveCommit(1)),
                gtk::gdk::Key::k | gtk::gdk::Key::K => Some(AppMsg::MoveCommit(-1)),
                gtk::gdk::Key::c | gtk::gdk::Key::C => Some(AppMsg::ShortcutCompose),
                gtk::gdk::Key::slash => Some(AppMsg::FocusSearch),
                gtk::gdk::Key::question => Some(AppMsg::ShowShortcuts),
                gtk::gdk::Key::y | gtk::gdk::Key::Y => Some(AppMsg::ShortcutOnCommit('y')),
                gtk::gdk::Key::r | gtk::gdk::Key::R => Some(AppMsg::ShortcutOnCommit('r')),
                gtk::gdk::Key::q | gtk::gdk::Key::Q => Some(AppMsg::ShortcutOnCommit('q')),
                gtk::gdk::Key::o | gtk::gdk::Key::O => Some(AppMsg::ShortcutOnCommit('o')),
                gtk::gdk::Key::s | gtk::gdk::Key::S => Some(AppMsg::ShortcutOnCommit('s')),
                gtk::gdk::Key::g | gtk::gdk::Key::G => {
                    awaiting_go.set(true);
                    let awaiting_go = awaiting_go.clone();
                    glib::timeout_add_local_once(Duration::from_millis(1000), move || {
                        awaiting_go.set(false);
                    });
                    return glib::Propagation::Stop;
                }
                _ => None,
            };
            if let Some(msg) = action {
                shortcut_sender.input(msg);
                return glib::Propagation::Stop;
            }
            glib::Propagation::Proceed
        });
        root.add_controller(shortcuts);
        Self::bind_width_breakpoints(&root, &sender);

        sender.input(AppMsg::Restore);
        let poll_sender = sender.clone();
        relm4::spawn(async move {
            loop {
                tokio::time::sleep(Duration::from_secs(30)).await;
                poll_sender.input(AppMsg::Poll);
            }
        });

        ComponentParts { model, widgets }
    }

    fn update(&mut self, msg: Self::Input, sender: ComponentSender<Self>) {
        match msg {
            AppMsg::Restore => {
                let uri = PENDING_URI
                    .get()
                    .and_then(|m| m.lock().ok().and_then(|mut g| g.take()))
                    .or_else(auth::take_callback_uri);
                if let Some(uri) = uri {
                    if let Some((pkce, _)) = auth::load_pending() {
                        self.pending = Some(pkce);
                        self.phase = Phase::WaitingForBrowser;
                        sender.input(AppMsg::OpenUri(uri));
                        return;
                    }
                    sender.input(AppMsg::OpenUri(uri));
                }
                let sender2 = sender.clone();
                relm4::spawn(async move {
                    sender2.input(AppMsg::Restored(restore().await));
                });
            }
            AppMsg::Restored(Ok((user, client))) => {
                if matches!(self.phase, Phase::WaitingForBrowser | Phase::SignedIn) {
                    return;
                }
                self.client = Some(client);
                self.set_user(Some(user));
                self.phase = Phase::SignedIn;
                sender.input(AppMsg::Select(Nav::Latest));
                sender.input(AppMsg::RefreshSuggested);
                self.load_safety_quiet(&sender);
                self.load_user_settings(&sender);
            }
            AppMsg::Restored(Err(err)) => {
                if matches!(self.phase, Phase::WaitingForBrowser | Phase::SignedIn) {
                    return;
                }
                self.phase = Phase::SignedOut;
                self.visible = "sign_in";
                if !err.requires_sign_in() {
                    self.error = Some(err.message);
                }
            }
            AppMsg::SignIn => {
                let _ = auth::register_url_handler();
                let sender2 = sender.clone();
                relm4::spawn(async move {
                    match auth::load_or_create_key().await {
                        Ok(key) => {
                            let pkce = Pkce::new();
                            auth::clear_pending();
                            auth::save_pending(&pkce, &key);
                            let url = auth::authorize_url(&key, &pkce);
                            sender2.input(AppMsg::BrowserOpened(pkce, url));
                        }
                        Err(err) => sender2.input(AppMsg::Toast(err.to_string())),
                    }
                });
            }
            AppMsg::BrowserOpened(pkce, url) => {
                self.pending = Some(pkce);
                self.phase = Phase::WaitingForBrowser;
                self.visible = "sign_in";
                self.error = None;
                sender.input(AppMsg::OpenWeb(url));
                let poll = sender.clone();
                relm4::spawn(async move {
                    for _ in 0..240 {
                        tokio::time::sleep(Duration::from_millis(500)).await;
                        poll.input(AppMsg::PollClipboard);
                    }
                });
            }
            AppMsg::CancelSignIn => {
                self.pending = None;
                auth::clear_pending();
                self.phase = Phase::SignedOut;
            }
            AppMsg::OpenUri(uri) => self.handle_callback(&uri, &sender),
            AppMsg::PasteCallback => {
                let text = self.paste_entry.text().to_string();
                self.handle_callback(&text, &sender);
            }
            AppMsg::PollClipboard => {
                if self.phase == Phase::WaitingForBrowser {
                    poll_clipboard(&sender);
                    let text = self.paste_entry.text().to_string();
                    if !text.is_empty() {
                        self.handle_callback(&text, &sender);
                    }
                }
            }
            AppMsg::Redeemed(Ok((user, client))) => {
                self.client = Some(client);
                self.set_user(Some(user));
                self.phase = Phase::SignedIn;
                self.error = None;
                sender.input(AppMsg::Select(Nav::Latest));
                sender.input(AppMsg::RefreshSuggested);
                self.load_safety_quiet(&sender);
                self.load_user_settings(&sender);
            }
            AppMsg::Redeemed(Err(err)) => {
                self.phase = Phase::SignedOut;
                self.error = Some(err.message);
            }
            AppMsg::SignOut => {
                auth::clear_pending();
                relm4::spawn(async {
                    let _ = auth::delete_key().await;
                });
                self.client = None;
                self.set_user(None);
                self.live_following.clear();
                self.hot_topics.clear();
                self.following.clear();
                self.suggested.clear();
                self.user_settings = None;
                self.user_card.clear();
                self.user_preview_loading.clear();
                self.translations.clear();
                self.showing_original.clear();
                self.requested_translation.clear();
                self.translation_loading.clear();
                self.notifications.clear();
                self.notif_groups.clear();
                ui::fill_live_following(&self.live_list, &[], sender.input_sender().clone());
                ui::fill_hot_topics(&self.hot_topics_list, &[], sender.input_sender().clone());
                self.phase = Phase::SignedOut;
                self.visible = "sign_in";
            }
            AppMsg::Select(nav) => {
                self.user_card.dismiss();
                self.nav = nav;
                self.capture_log_visit();
                self.expanded_replies.clear();
                self.nested_replies.clear();
                self.previewed_replies.clear();
                self.preview_loading.clear();
                self.replying_to = None;
                match nav {
                    Nav::Notifications => {
                        self.visible = "notifications";
                        self.load_notifications(&sender);
                    }
                    Nav::Remotes => {
                        self.visible = "remotes";
                        self.close_open_thread(&sender);
                        self.load_remotes(&sender);
                    }
                    Nav::Starred => {
                        self.visible = "log";
                        self.load_starred(&sender, false);
                    }
                    _ => {
                        self.visible = "log";
                        self.load_log(&sender, false);
                    }
                }
                self.poll_unread(&sender);
            }
            AppMsg::OpenSettings => {
                self.visible = "settings";
                self.draft_lane = None;
                self.render_settings(&sender);
            }
            AppMsg::SettingsTab(tab) => {
                if tab != SettingsTab::Profile {
                    self.draft_lane = None;
                }
                self.settings_tab = tab;
                if tab == SettingsTab::Safety {
                    self.load_safety(&sender);
                }
                self.render_settings(&sender);
            }
            AppMsg::SetTheme(theme) => {
                self.prefs.theme = theme;
                self.prefs.save();
                self.render_settings(&sender);
            }
            AppMsg::SetWhoToFollow(show) => {
                self.prefs.show_who_to_follow = show;
                self.prefs.save();
                if show {
                    sender.input(AppMsg::RefreshSuggested);
                }
                self.render_settings(&sender);
                if self.visible == "log" {
                    self.refresh_visible_feed(&sender, true);
                }
            }
            AppMsg::SetHighlightOnHover(on) => {
                self.prefs.highlight_on_hover = on;
                self.prefs.save();
                self.hover_enabled.set(on);
                if !on {
                    ui::clear_feed_hover(&self.log_list);
                    ui::clear_feed_hover(&self.profile_list);
                    ui::clear_feed_hover(&self.search_list);
                }
                self.render_settings(&sender);
            }
            AppMsg::SetTimezone(timezone) => {
                if let Some(client) = self.client.clone() {
                    self.begin_busy();
                    let sender2 = sender.clone();
                    relm4::spawn(async move {
                        sender2.input(AppMsg::TimezoneSaved(
                            client.set_timezone(timezone.as_deref()).await,
                        ));
                    });
                }
            }
            AppMsg::TimezoneSaved(result) => {
                self.end_busy();
                match result {
                    Ok(user) => {
                        self.set_user(Some(user));
                        self.toast("Timezone saved.");
                        self.render_settings(&sender);
                    }
                    Err(err) => self.toast(&err.message),
                }
            }
            AppMsg::SetAutomaticTranslation(enabled) => {
                self.patch_settings(|settings| settings.automatic_translation = enabled);
                self.render_settings(&sender);
                if enabled {
                    self.queue_auto_translations_visible(&sender);
                }
                self.refresh_visible_feed(&sender, true);
                if let Some(client) = self.client.clone() {
                    self.begin_busy();
                    let sender2 = sender.clone();
                    relm4::spawn(async move {
                        sender2.input(AppMsg::AutomaticTranslationSaved(
                            client.set_automatic_translation(enabled).await,
                        ));
                    });
                }
            }
            AppMsg::AutomaticTranslationSaved(result) => {
                self.end_busy();
                match result {
                    Ok(user) => {
                        self.set_user(Some(user));
                        self.render_settings(&sender);
                    }
                    Err(err) => {
                        self.toast(&err.message);
                        self.load_user_settings(&sender);
                    }
                }
            }
            AppMsg::SetTranslationLanguage(language) => {
                self.patch_settings(|settings| settings.translation_language = language.clone());
                self.translations.clear();
                self.showing_original.clear();
                self.requested_translation.clear();
                self.translation_loading.clear();
                self.render_settings(&sender);
                if self.automatic_translation() {
                    self.queue_auto_translations_visible(&sender);
                }
                self.refresh_visible_feed(&sender, true);
                if let Some(client) = self.client.clone() {
                    self.begin_busy();
                    let sender2 = sender.clone();
                    relm4::spawn(async move {
                        sender2.input(AppMsg::TranslationLanguageSaved(
                            client.set_translation_language(language.as_deref()).await,
                        ));
                    });
                }
            }
            AppMsg::TranslationLanguageSaved(result) => {
                self.end_busy();
                match result {
                    Ok(user) => {
                        self.set_user(Some(user));
                        self.toast("Translation language saved.");
                        self.render_settings(&sender);
                    }
                    Err(err) => {
                        self.toast(&err.message);
                        self.load_user_settings(&sender);
                    }
                }
            }
            AppMsg::UserSettingsLoaded(result) => match result {
                Ok(settings) => {
                    self.user_settings = Some(settings);
                    self.render_settings(&sender);
                    if self.automatic_translation() {
                        self.queue_auto_translations_visible(&sender);
                        self.refresh_visible_feed(&sender, true);
                    }
                }
                Err(_) => {
                    if self.user_settings.is_none() {
                        self.user_settings = Some(UserSettings::default());
                    }
                    self.render_settings(&sender);
                    if self.automatic_translation() {
                        self.queue_auto_translations_visible(&sender);
                        self.refresh_visible_feed(&sender, true);
                    }
                }
            },
            AppMsg::SetRemotes(enabled) => {
                if let Some(client) = self.client.clone() {
                    self.begin_busy();
                    let sender2 = sender.clone();
                    relm4::spawn(async move {
                        sender2.input(AppMsg::RemotesSaved(client.set_remotes(enabled).await));
                    });
                }
            }
            AppMsg::RemotesSaved(result) => {
                self.end_busy();
                match result {
                    Ok(user) => {
                        self.set_user(Some(user));
                        if !self.remotes_on() && self.visible == "remotes" {
                            sender.input(AppMsg::Select(Nav::Latest));
                        }
                        self.render_settings(&sender);
                    }
                    Err(err) => self.toast(&err.message),
                }
            }
            AppMsg::ExportData => {
                if let Some(client) = self.client.clone() {
                    self.begin_busy();
                    let sender2 = sender.clone();
                    relm4::spawn(async move {
                        sender2.input(AppMsg::ExportReady(client.export().await));
                    });
                }
            }
            AppMsg::ExportReady(result) => {
                self.end_busy();
                match result {
                    Ok(bytes) => ui::save_export(bytes, sender.input_sender().clone()),
                    Err(err) => self.toast(&err.message),
                }
            }
            AppMsg::DeleteAccount => {
                ui::confirm_delete_account(sender.input_sender().clone());
            }
            AppMsg::ConfirmDeleteAccount => {
                if let Some(client) = self.client.clone() {
                    self.begin_busy();
                    let sender2 = sender.clone();
                    relm4::spawn(async move {
                        sender2.input(AppMsg::AccountDeleted(client.delete_account().await));
                    });
                }
            }
            AppMsg::AccountDeleted(result) => {
                self.end_busy();
                match result {
                    Ok(()) => {
                        self.toast("Account deleted.");
                        sender.input(AppMsg::SignOut);
                    }
                    Err(err) => self.toast(&err.message),
                }
            }
            AppMsg::SafetyLoaded(muted, blocked) => {
                self.end_busy();
                match muted {
                    Ok(users) => self.muted = users,
                    Err(err) if self.visible == "settings" => self.toast_unless_limited(&err),
                    Err(_) => {}
                }
                match blocked {
                    Ok(users) => self.blocked = users,
                    Err(err) if self.visible == "settings" && self.muted.is_empty() => {
                        self.toast_unless_limited(&err);
                    }
                    Err(_) => {}
                }
                if self.visible == "settings" {
                    self.render_settings(&sender);
                }
            }
            AppMsg::Unmute(username) => {
                if let Some(client) = self.client.clone() {
                    self.begin_busy();
                    let sender2 = sender.clone();
                    relm4::spawn(async move {
                        let result = client.unmute(&username).await;
                        sender2.input(AppMsg::SafetyChanged(username, true, result));
                    });
                }
            }
            AppMsg::Unblock(username) => {
                if let Some(client) = self.client.clone() {
                    self.begin_busy();
                    let sender2 = sender.clone();
                    relm4::spawn(async move {
                        let result = client.unblock(&username).await;
                        sender2.input(AppMsg::SafetyChanged(username, false, result));
                    });
                }
            }
            AppMsg::SafetyChanged(username, muted, result) => {
                self.end_busy();
                match result {
                    Ok(()) => {
                        if muted {
                            self.user_card
                                .patch(&username, |preview| preview.is_muting = false);
                        } else {
                            self.user_card.patch(&username, |preview| {
                                preview.is_blocking = false;
                                preview.is_unavailable = false;
                            });
                        }
                        self.user_card.refresh(
                            self.user.as_ref().map(|user| user.user.id.as_str()),
                            sender.input_sender(),
                        );
                        self.toast(&format!(
                            "{} @{username}.",
                            if muted { "Unmuted" } else { "Unblocked" }
                        ));
                        self.load_safety(&sender);
                    }
                    Err(err) => self.toast(&err.message),
                }
            }
            AppMsg::FocusSearch => {
                if self.phase == Phase::SignedIn {
                    self.omnibox.open();
                }
            }
            AppMsg::OpenCompose => self.open_compose(),
            AppMsg::ComposeClosed => self.restore_composer(),
            AppMsg::OmniboxQuery(query) => {
                self.omnibox_gen = self.omnibox_gen.wrapping_add(1);
                let gen = self.omnibox_gen;
                let trimmed = query.trim().to_string();
                if trimmed.is_empty() {
                    self.omnibox.set_status(None);
                    self.omnibox.show_suggestions();
                    return;
                }
                if trimmed == "@" {
                    self.omnibox
                        .set_status(Some("Type a name or handle to find a developer"));
                    ui::clear_list(&self.omnibox.list);
                    return;
                }
                let sender2 = sender.clone();
                relm4::spawn(async move {
                    tokio::time::sleep(Duration::from_millis(250)).await;
                    sender2.input(AppMsg::OmniboxGo(gen, trimmed));
                });
            }
            AppMsg::OmniboxGo(gen, query) => {
                if gen != self.omnibox_gen || self.omnibox.entry.text().trim() != query {
                    return;
                }
                let Some(client) = self.client.clone() else {
                    return;
                };
                self.omnibox.show_skeletons();
                let sender2 = sender.clone();
                relm4::spawn(async move {
                    sender2.input(AppMsg::OmniboxLoaded(
                        gen,
                        query.clone(),
                        client.search(&query).await,
                    ));
                });
            }
            AppMsg::OmniboxLoaded(gen, query, result) => {
                if gen != self.omnibox_gen || self.omnibox.entry.text().trim() != query {
                    return;
                }
                match result {
                    Ok(results) => self.omnibox.show_results(&query, results),
                    Err(err) if err.status == 429 => self
                        .omnibox
                        .set_status(Some("error: too many searches, give it a few seconds")),
                    Err(err) => self.omnibox.set_status(Some(&err.message)),
                }
            }
            AppMsg::ToggleSidebar => {
                self.sidebar_collapsed = Some(!self.nav_collapsed());
            }
            AppMsg::NavCompact(compact) => {
                self.nav_compact = compact;
            }
            AppMsg::AsideCompact(compact) => {
                self.aside_compact = compact;
                if self.visible == "remotes" {
                    self.thread_header_stamp.clear();
                    self.fill_open_thread_header(
                        self.open_thread.as_ref(),
                        sender.input_sender().clone(),
                    );
                }
                if self.visible == "log" {
                    self.refresh_visible_feed(&sender, true);
                }
            }
            AppMsg::CloseThread => self.close_open_thread(&sender),
            AppMsg::ShellCompact(compact) => {
                self.shell_compact = compact;
                self.sync_chat_dock();
            }
            AppMsg::TogglePoll => {
                if self.composer_poll || !self.composer_images.is_empty() {
                    return;
                }
                self.composer_poll = true;
                self.composer_images.clear();
                self.poll_ui.reset();
                self.sync_image_previews(&sender);
            }
            AppMsg::RemovePoll => {
                self.composer_poll = false;
                self.poll_ui.reset();
            }
            AppMsg::ToggleSchedule => {
                self.composer_schedule = !self.composer_schedule;
                if self.composer_schedule {
                    self.schedule_ui.reset();
                }
            }
            AppMsg::TogglePreview => {
                self.composer_preview = !self.composer_preview;
                if self.composer_preview {
                    self.emoji.hide();
                    self.mentions.hide();
                } else {
                    self.sync_composer_suggests(&self.composer.clone(), &sender);
                }
            }
            AppMsg::SetPasteis(enabled) => {
                if let Some(client) = self.client.clone() {
                    self.begin_busy();
                    let sender2 = sender.clone();
                    relm4::spawn(async move {
                        sender2.input(AppMsg::PasteisSaved(client.set_pasteis_mode(enabled).await));
                    });
                }
            }
            AppMsg::PasteisSaved(result) => {
                self.end_busy();
                match result {
                    Ok(user) => {
                        self.set_user(Some(user));
                        if self.visible == "settings" {
                            self.render_settings(&sender);
                        }
                    }
                    Err(err) => self.toast(&err.message),
                }
            }
            AppMsg::OpenMe => {
                if let Some(user) = &self.user {
                    sender.input(AppMsg::OpenProfile(user.username().to_owned()));
                }
            }
            AppMsg::ComposerAvatar => {
                if self.can_post_as() {
                    self.author_menu.popup();
                } else if let Some(user) = &self.user {
                    sender.input(AppMsg::OpenProfile(user.username().to_owned()));
                }
            }
            AppMsg::PostAs(handle) => {
                self.composer_as = handle.and_then(|handle| {
                    self.user.as_ref().and_then(|user| {
                        user.organizations
                            .iter()
                            .find(|org| org.username.eq_ignore_ascii_case(&handle))
                            .cloned()
                    })
                });
                self.refresh_me_avatars();
            }
            AppMsg::StartRemote => {
                let Some(profile) = &self.profile else {
                    return;
                };
                let username = profile.username().to_owned();
                self.nav = Nav::Remotes;
                self.visible = "remotes";
                self.load_remotes(&sender);
                sender.input(AppMsg::OpenThread(ThreadRef::Direct { username }));
            }
            AppMsg::Back => {
                if self.visible == "connections" {
                    if self.connections_kind == ConnectionsKind::Stargazers {
                        self.connections_commit = None;
                        sender.input(AppMsg::Select(self.nav));
                        return;
                    }
                    if !self.connections_handle.is_empty() {
                        sender.input(AppMsg::OpenProfile(self.connections_handle.clone()));
                        return;
                    }
                }
                sender.input(AppMsg::Select(self.nav));
            }
            AppMsg::ComposerChanged => {
                self.sync_composer_suggests(&self.composer.clone(), &sender);
                self.composer_len = buffer_text(&self.composer).chars().count();
                self.sync_composer_ring();
            }
            AppMsg::ReplyComposerChanged => {
                self.sync_composer_suggests(&self.reply_view().clone(), &sender)
            }
            AppMsg::RemoteComposerChanged => {
                self.sync_composer_suggests(&self.remote_composer.clone(), &sender);
                self.remote_draft_len = buffer_text(&self.remote_composer).trim().chars().count();
                self.ping_remote_typing();
            }
            AppMsg::EmojiMove(step) => self.emoji.move_selection(step),
            AppMsg::EmojiPick => {
                if let Some(glyph) = self.emoji.active_emoji() {
                    self.insert_emoji(&glyph);
                }
            }
            AppMsg::EmojiDismiss => self.emoji.hide(),
            AppMsg::EmojiChosen(glyph) => self.insert_emoji(&glyph),
            AppMsg::LogLoaded(Ok(page)) => {
                self.end_busy();
                let first_seen = if self.log_cursor.is_none() {
                    self.rebuild_log_list();
                    self.log_last_day = None;
                    self.log_last_lane = None;
                    self.log_commits.clear();
                    self.log_first_seen = None;
                    self.clear_new_posts();
                    self.log_seen_id = page.data.first().map(|commit| commit.id.clone());
                    self.log_away = false;
                    self.log_scroll.vadjustment().set_value(0.0);
                    let seen = self.first_seen_id(&page.data);
                    if let Some(newest) = page.data.first().and_then(|commit| commit.created_at) {
                        if let Some(user) = &self.user {
                            if let Some(log) = self.nav.log() {
                                self.prefs.remember_visit(
                                    &user.user.id,
                                    log.as_str(),
                                    newest.to_rfc3339(),
                                );
                            }
                        }
                    }
                    seen
                } else {
                    None
                };
                self.log_loading_more = false;
                if page.data.is_empty() && self.log_commits.is_empty() {
                    let (fatal, detail) = match self.nav {
                        Nav::Following => (
                            "fatal: your log does not have any commits yet",
                            "Your log shows your commits and commits from people you follow. Find someone worth following on Trending.",
                        ),
                        Nav::Starred => (
                            "fatal: no starred commits yet",
                            "Star a commit and it shows up here.",
                        ),
                        _ => (
                            "fatal: no commits yet",
                            "Nobody has committed anything. Be the first.",
                        ),
                    };
                    self.log_list.append(&ui::empty_row(fatal, detail));
                } else {
                    let more = !self.log_commits.is_empty();
                    if more {
                        self.log_commits.extend(page.data.iter().cloned());
                    } else {
                        self.log_commits = page.data.clone();
                        self.log_first_seen = first_seen.clone();
                    }
                    if self.nav == Nav::Following {
                        for commit in &page.data {
                            if let Some(author) = &commit.author {
                                self.remember_followed(&author.username);
                            }
                        }
                    }
                    self.set_live_following(self.live_following.clone(), &sender);
                    self.queue_auto_translations(&page.data, &sender);
                    let ctx = self.feed_ctx(&sender);
                    let offset = if more {
                        self.log_commits.len().saturating_sub(page.data.len())
                    } else {
                        0
                    };
                    ui::append_commits(
                        &self.log_list,
                        &page.data,
                        &ctx,
                        &mut self.log_last_lane,
                        &mut self.log_last_day,
                        if more { None } else { first_seen.as_deref() },
                        Some(&ui::ThreadState {
                            nested: &self.nested_replies,
                            expanded: &self.expanded_replies,
                            replying_to: self.replying_to.as_deref(),
                            composer: &self.inline_reply_box,
                        }),
                        offset,
                    );
                    self.log_cursor = page.next_cursor();
                    if self.log_cursor.is_none() {
                        self.log_list.append(&ui::root_row(
                            "That's the root commit. There's nothing before it.",
                        ));
                    }
                    if self.log_cursor.is_some() && self.near_log_end() {
                        sender.input(AppMsg::LoadMore);
                    }
                    self.queue_reply_previews(&page.data, &sender);
                }
            }
            AppMsg::LogLoaded(Err(err)) => {
                self.end_busy();
                self.log_loading_more = false;
                if self.log_commits.is_empty() {
                    self.rebuild_log_list();
                    self.log_list.append(&ui::empty_row(
                        "fatal: could not load this log",
                        &err.message,
                    ));
                    if err.is_rate_limited() {
                        self.auto_translate_retry_at =
                            Some(Instant::now() + Duration::from_secs(60));
                    } else {
                        self.fail(err);
                    }
                } else {
                    self.toast_unless_limited(&err);
                }
            }
            AppMsg::LoadMore => {
                if self.log_loading_more || self.log_cursor.is_none() {
                    return;
                }
                self.log_loading_more = true;
                if self.nav == Nav::Starred {
                    self.load_starred(&sender, true);
                } else {
                    self.load_log(&sender, true);
                }
            }
            AppMsg::GivePastel(id) => {
                if let Some(client) = self.client.clone() {
                    self.begin_busy();
                    let sender2 = sender.clone();
                    relm4::spawn(async move {
                        sender2.input(AppMsg::PastelGiven(
                            id.clone(),
                            client.award_pastel(&id).await,
                        ));
                    });
                }
            }
            AppMsg::PastelGiven(id, result) => {
                self.end_busy();
                match result {
                    Ok(award) => {
                        if let Some(user) = &mut self.user {
                            user.pasteis_remaining = Some(award.remaining);
                        }
                        self.pasteis.insert(id.clone(), award.pasteis_count);
                        self.set_pasteis_count(&id, award.pasteis_count);
                        self.toast(&format!("pastel awarded · {} left today", award.remaining));
                    }
                    Err(err) => {
                        self.pasteis.remove(&id);
                        self.toast(&err.message);
                        self.refresh_visible_feed(&sender, true);
                    }
                }
            }
            AppMsg::StartQuote(commit) => {
                ui::quote_window(&commit, sender.input_sender().clone()).present();
            }
            AppMsg::SubmitQuote(id, body) => {
                if body.trim().is_empty() {
                    self.toast("fatal: nothing to commit");
                    return;
                }
                if let Some(client) = self.client.clone() {
                    self.begin_busy();
                    let sender2 = sender.clone();
                    let author = self.posting_as_handle();
                    relm4::spawn(async move {
                        sender2.input(AppMsg::CommitCreated(
                            client
                                .create_commit(
                                    &body,
                                    None,
                                    Some(&id),
                                    Vec::new(),
                                    None,
                                    None,
                                    author.as_deref(),
                                )
                                .await,
                        ));
                    });
                }
            }
            AppMsg::ReplyTo(id) => {
                if self.visible == "commit"
                    && self
                        .open_commit
                        .as_ref()
                        .is_some_and(|commit| commit.id == id)
                {
                    self.replying_to = None;
                    self.focus_reply = true;
                    self.reply_composer.grab_focus();
                    return;
                }
                if self.replying_to.as_deref() == Some(id.as_str()) {
                    self.replying_to = None;
                } else {
                    self.replying_to = Some(id);
                    self.inline_reply.buffer().set_text("");
                }
                self.refresh_visible_feed(&sender, true);
                if self.replying_to.is_some() {
                    self.inline_reply.grab_focus();
                }
            }
            AppMsg::CancelReply => {
                self.replying_to = None;
                self.inline_reply.buffer().set_text("");
                self.refresh_visible_feed(&sender, true);
            }
            AppMsg::TranslateCommit(id) => self.translate_commit(&id, &sender),
            AppMsg::ShowOriginal(id) => {
                self.requested_translation.remove(&id);
                self.showing_original.insert(id);
                self.refresh_visible_feed(&sender, true);
            }
            AppMsg::CommitTranslated(id, result) => {
                self.translation_loading.remove(&id);
                match result {
                    Ok(translation) => {
                        self.showing_original.remove(&id);
                        self.translations.insert(id, translation);
                        self.refresh_visible_feed(&sender, true);
                        if self.automatic_translation() && self.translation_loading.is_empty() {
                            self.queue_auto_translations_visible(&sender);
                        }
                    }
                    Err(err) => {
                        if err.is_rate_limited() {
                            self.translation_loading.clear();
                            self.auto_translate_retry_at =
                                Some(Instant::now() + Duration::from_secs(60));
                        }
                        if self.requested_translation.contains(&id) {
                            self.toast(&err.message);
                        }
                        self.translations.entry(id).or_default();
                    }
                }
            }
            AppMsg::ToggleReplies(id) => {
                if self.expanded_replies.contains(&id) {
                    self.expanded_replies.remove(&id);
                    self.refresh_visible_feed(&sender, true);
                    return;
                }
                let focused = self
                    .open_commit
                    .as_ref()
                    .is_some_and(|commit| commit.id == id);
                let have_full = (self.nested_replies.contains_key(&id)
                    && !self.previewed_replies.contains(&id))
                    || (focused && !self.thread_replies.is_empty());
                if have_full {
                    self.expanded_replies.insert(id);
                    self.refresh_visible_feed(&sender, true);
                    return;
                }
                self.expanded_replies.insert(id.clone());
                self.refresh_visible_feed(&sender, true);
                if let Some(client) = self.client.clone() {
                    let sender2 = sender.clone();
                    relm4::spawn(async move {
                        sender2.input(AppMsg::NestedRepliesLoaded(
                            id.clone(),
                            client.replies(&id, None, None).await,
                        ));
                    });
                }
            }
            AppMsg::NestedRepliesLoaded(id, result) => match result {
                Ok(page) => {
                    let replies = newest_replies(page.data);
                    self.queue_auto_translations(&replies, &sender);
                    self.nested_replies.insert(id.clone(), replies);
                    self.previewed_replies.remove(&id);
                    self.preview_loading.remove(&id);
                    self.expanded_replies.insert(id);
                    self.refresh_visible_feed(&sender, true);
                }
                Err(err) => {
                    self.expanded_replies.remove(&id);
                    self.preview_loading.remove(&id);
                    self.refresh_visible_feed(&sender, true);
                    self.toast(&err.message);
                }
            },
            AppMsg::PreviewRepliesLoaded(id, result) => {
                self.preview_loading.remove(&id);
                let Ok(page) = result else {
                    return;
                };
                if self.nested_replies.contains_key(&id) && !self.previewed_replies.contains(&id) {
                    return;
                }
                if page.data.len() == ui::PREVIEW_REPLIES {
                    self.previewed_replies.insert(id.clone());
                } else {
                    self.previewed_replies.remove(&id);
                }
                let replies = newest_replies(page.data);
                self.queue_auto_translations(&replies, &sender);
                self.nested_replies.insert(id, replies);
                self.refresh_visible_feed(&sender, true);
            }
            AppMsg::ToggleNotificationFilter(kind) => {
                if !self.notif_types.remove(&kind) {
                    self.notif_types.insert(kind);
                }
                ui::clear_list(&self.notif_list);
                self.load_notifications(&sender);
            }
            AppMsg::ShowAllNotificationFilters => {
                self.notif_types = NotificationFilter::enabled_all();
                ui::clear_list(&self.notif_list);
                self.load_notifications(&sender);
            }
            AppMsg::ToggleStar(id, star, count) => {
                self.stars.insert(id.clone(), (star, count));
                if let Some(client) = self.client.clone() {
                    let sender2 = sender.clone();
                    relm4::spawn(async move {
                        let result = if star {
                            client.star(&id).await
                        } else {
                            client.unstar(&id).await
                        };
                        sender2.input(AppMsg::StarToggled(id, star, count, result));
                    });
                }
            }
            AppMsg::StarToggled(id, star, count, result) => match result {
                Ok(()) => {}
                Err(err) => {
                    let previous = if star {
                        count.saturating_sub(1)
                    } else {
                        count + 1
                    };
                    self.stars.insert(id, (!star, previous));
                    self.toast(&err.message);
                }
            },
            AppMsg::OpenCommit(id) => {
                self.visible = "commit";
                self.open_commit = Some(Commit {
                    id: id.clone(),
                    ..Default::default()
                });
                self.ancestors.clear();
                self.related_commits.clear();
                self.thread_replies.clear();
                self.nested_replies.clear();
                self.expanded_replies.clear();
                self.previewed_replies.clear();
                self.preview_loading.clear();
                if !self.focus_reply {
                    self.reply_composer.buffer().set_text("");
                }
                self.begin_busy();
                ui::clear_list(&self.reply_list);
                ui::append_commit_skeletons(&self.reply_list, 2);
                if let Some(client) = self.client.clone() {
                    let sender2 = sender.clone();
                    let related_client = client.clone();
                    let related_id = id.clone();
                    let related_sender = sender2.clone();
                    relm4::spawn(async move {
                        let commit = client.commit(&id).await;
                        let replies = client.replies(&id, None, None).await;
                        sender2.input(AppMsg::CommitLoaded(commit, replies));
                    });
                    relm4::spawn(async move {
                        let related = related_client
                            .related_commits(&related_id)
                            .await
                            .unwrap_or_default();
                        related_sender.input(AppMsg::RelatedLoaded(related_id, related));
                    });
                }
            }
            AppMsg::CommitLoaded(commit, replies) => {
                self.end_busy();
                match commit {
                    Ok(page) => {
                        self.open_commit = Some(page.commit);
                        self.ancestors = page.ancestors;
                    }
                    Err(err) => {
                        self.fail(err);
                        return;
                    }
                }
                self.thread_replies = replies
                    .ok()
                    .map(|page| newest_replies(page.data))
                    .unwrap_or_default();
                self.queue_auto_translations_visible(&sender);
                self.render_thread(&sender);
                if self.focus_reply {
                    self.focus_reply = false;
                    self.reply_composer.grab_focus();
                }
            }
            AppMsg::RelatedLoaded(id, commits) => {
                if self
                    .open_commit
                    .as_ref()
                    .is_some_and(|commit| commit.id == id)
                {
                    self.related_commits = commits;
                    if self.visible == "commit" {
                        self.render_thread(&sender);
                    }
                }
            }
            AppMsg::OpenProfile(username) => {
                self.user_card.dismiss();
                self.visible = "profile";
                self.profile_tab = ProfileFeed::Commits;
                self.profile_readme = None;
                self.organization = None;
                self.profile_bar_scrolled = false;
                self.load_profile(&sender, username, false);
            }
            AppMsg::OpenOrganization(handle) => {
                self.user_card.dismiss();
                self.visible = "profile";
                self.profile = None;
                self.profile_tab = ProfileFeed::Commits;
                self.profile_readme = None;
                self.profile_bar_scrolled = false;
                self.load_organization(&sender, handle);
            }
            AppMsg::ProfileBarScrolled(scrolled) => {
                if self.profile_bar_scrolled != scrolled {
                    self.profile_bar_scrolled = scrolled;
                }
            }
            AppMsg::ProfileTab(tab) => {
                if self.organization.is_some() {
                    if !matches!(tab, ProfileFeed::Commits | ProfileFeed::Mentions) {
                        return;
                    }
                    let same = self.profile_tab == tab;
                    self.profile_tab = tab;
                    self.render_profile(&sender);
                    if !same {
                        if let Some(handle) =
                            self.organization.as_ref().map(|org| org.handle.clone())
                        {
                            self.load_organization(&sender, handle);
                        }
                    }
                    return;
                }
                let same = self.profile_tab == tab;
                self.profile_tab = tab;
                self.render_profile(&sender);
                match tab {
                    ProfileFeed::Readme => {
                        if !same || self.profile_readme.is_none() {
                            self.load_readme(&sender);
                        }
                    }
                    ProfileFeed::Commits | ProfileFeed::Replies => {
                        if let Some(username) =
                            self.profile.as_ref().map(|p| p.username().to_owned())
                        {
                            self.load_profile(&sender, username, tab == ProfileFeed::Replies);
                        }
                    }
                    ProfileFeed::Mentions => {}
                }
            }
            AppMsg::ProfileReadmeLoaded(username, result) => {
                self.end_busy();
                if self.profile.as_ref().map(|p| p.username()) != Some(username.as_str()) {
                    return;
                }
                self.profile_readme = Some((username, result));
                if self.profile_tab == ProfileFeed::Readme {
                    self.render_profile_readme(&sender);
                }
            }
            AppMsg::ProfileLoaded(user, commits) => {
                self.end_busy();
                self.reset_profile_list();
                match user {
                    Ok(u) => {
                        self.profile = Some(*u);
                        self.organization = None;
                    }
                    Err(err) => {
                        self.fail(err);
                        return;
                    }
                }
                self.render_profile(&sender);
                if let Ok(page) = commits {
                    let pinned = if self.profile_tab == ProfileFeed::Replies {
                        None
                    } else {
                        self.profile.as_ref().and_then(|p| p.pinned_commit.clone())
                    };
                    let mut commits: Vec<Commit> = Vec::new();
                    if let Some(pinned) = pinned.clone() {
                        commits.push(pinned);
                    }
                    commits.extend(
                        page.data
                            .into_iter()
                            .filter(|commit| pinned.as_ref().is_none_or(|pin| pin.id != commit.id)),
                    );
                    self.profile_commits = commits;
                }
                match self.profile_tab {
                    ProfileFeed::Readme => self.load_readme(&sender),
                    ProfileFeed::Commits | ProfileFeed::Replies | ProfileFeed::Mentions => {
                        self.queue_auto_translations_visible(&sender);
                        self.render_profile_feed(&sender, false);
                        let ids = reply_preview_ids(&self.profile_commits);
                        self.queue_reply_previews_ids(ids, &sender);
                    }
                }
            }
            AppMsg::OrganizationLoaded(organization, commits) => {
                self.end_busy();
                self.reset_profile_list();
                match organization {
                    Ok(organization) => {
                        self.organization = Some(*organization);
                        self.profile = None;
                    }
                    Err(err) => {
                        self.fail(err);
                        return;
                    }
                }
                self.render_profile(&sender);
                if let Ok(page) = commits {
                    self.profile_commits = page.data;
                } else {
                    self.profile_commits.clear();
                }
                self.queue_auto_translations_visible(&sender);
                self.render_profile_feed(&sender, false);
                let ids = reply_preview_ids(&self.profile_commits);
                self.queue_reply_previews_ids(ids, &sender);
            }
            AppMsg::GoLive => {
                if self.user.as_ref().is_some_and(|user| user.user.is_live()) {
                    sender.input(AppMsg::EndLive);
                    return;
                }
                let dialog = ui::go_live_dialog(sender.input_sender().clone());
                dialog.present(Some(&self.log_scroll));
                ui::apply_pointer_cursors(&dialog);
            }
            AppMsg::GoLiveSubmit { url, hours } => {
                let url = url.trim().to_owned();
                if url.is_empty() {
                    self.toast("fatal: a stream needs a YouTube or Twitch link");
                    return;
                }
                if let Some(client) = self.client.clone() {
                    self.begin_busy();
                    let sender2 = sender.clone();
                    relm4::spawn(async move {
                        sender2.input(AppMsg::LiveSaved(client.set_live(&url, hours).await));
                    });
                }
            }
            AppMsg::EndLive => {
                if let Some(client) = self.client.clone() {
                    self.begin_busy();
                    let sender2 = sender.clone();
                    relm4::spawn(async move {
                        sender2.input(AppMsg::LiveSaved(client.end_live().await));
                    });
                }
            }
            AppMsg::LiveSaved(result) => {
                self.end_busy();
                match result {
                    Ok(user) => {
                        let live = user.user.is_live();
                        self.apply_me(user, &sender);
                        self.toast(if live { "You're live" } else { "Stream ended" });
                    }
                    Err(err) => self.toast(&err.message),
                }
            }
            AppMsg::OpenConnections { handle, kind } => {
                self.visible = "connections";
                self.connections_kind = kind;
                self.connections_handle = handle.clone();
                self.connections_commit = None;
                self.connections_star_count = 0;
                self.connections_title = kind.title().into();
                self.connections_users.clear();
                self.connections_orgs.clear();
                self.connections_cursor = None;
                self.connections_orgs_cursor = None;
                self.connections_orgs_expanded = false;
                self.connections_loading = false;
                ui::clear_list(&self.connections_list);
                self.connections_list
                    .append(&ui::empty_row("Loading…", "Fetching this list."));
                self.load_connections(false, &sender);
            }
            AppMsg::OpenStargazers { commit_id, count } => {
                self.visible = "connections";
                self.connections_kind = ConnectionsKind::Stargazers;
                self.connections_handle.clear();
                self.connections_commit = Some(commit_id);
                self.connections_star_count = count;
                self.connections_title = ConnectionsKind::Stargazers.title().into();
                self.connections_users.clear();
                self.connections_orgs.clear();
                self.connections_cursor = None;
                self.connections_orgs_cursor = None;
                self.connections_orgs_expanded = false;
                self.connections_loading = false;
                ui::clear_list(&self.connections_list);
                self.connections_list
                    .append(&ui::empty_row("Loading…", "Fetching who starred this."));
                self.load_connections(false, &sender);
            }
            AppMsg::RetryStargazers => {
                if self.connections_kind != ConnectionsKind::Stargazers
                    || self.connections_commit.is_none()
                {
                    return;
                }
                self.connections_users.clear();
                self.connections_cursor = None;
                self.connections_loading = false;
                ui::clear_list(&self.connections_list);
                self.connections_list
                    .append(&ui::empty_row("Loading…", "Fetching who starred this."));
                self.load_connections(false, &sender);
            }
            AppMsg::ConnectionsLoaded {
                handle,
                kind,
                more,
                users,
                organizations,
            } => {
                let same = match kind {
                    ConnectionsKind::Stargazers => {
                        self.connections_kind == kind
                            && self.connections_commit.as_deref() == Some(handle.as_str())
                    }
                    _ => self.connections_handle == handle && self.connections_kind == kind,
                };
                if !same {
                    return;
                }
                self.connections_loading = false;
                match users {
                    Ok(page) => {
                        self.connections_cursor = page.next_cursor();
                        if more {
                            self.connections_users.extend(page.data);
                        } else {
                            self.connections_users = page.data;
                        }
                    }
                    Err(err) if !more => {
                        ui::clear_list(&self.connections_list);
                        if kind == ConnectionsKind::Stargazers {
                            self.connections_list
                                .append(&ui::stargazers_error_row(sender.input_sender().clone()));
                        } else {
                            self.connections_list.append(&ui::empty_row(
                                "fatal: could not load this list",
                                &err.message,
                            ));
                        }
                        return;
                    }
                    Err(err) => self.toast(&err.message),
                }
                if let Ok(page) = organizations {
                    self.connections_orgs_cursor = page.next_cursor();
                    if more {
                        self.connections_orgs.extend(page.data);
                    } else {
                        self.connections_orgs = page.data;
                    }
                }
                self.render_connections(&sender);
            }
            AppMsg::LoadMoreConnections => {
                if self.visible != "connections"
                    || self.connections_loading
                    || self.connections_cursor.is_none()
                {
                    return;
                }
                self.load_connections(true, &sender);
            }
            AppMsg::ExpandConnectionOrgs => {
                self.connections_orgs_expanded = true;
                self.render_connections(&sender);
            }
            AppMsg::VotePoll { commit, option } => {
                if let Some(client) = self.client.clone() {
                    let sender2 = sender.clone();
                    relm4::spawn(async move {
                        sender2.input(AppMsg::PollVoted(
                            commit.clone(),
                            client.vote_poll(&commit, &option).await,
                        ));
                    });
                }
            }
            AppMsg::PollVoted(id, result) => match result {
                Ok(commit) => {
                    if let Some(poll) = commit.poll {
                        self.apply_poll(&id, poll);
                    }
                    self.refresh_visible_feed(&sender, true);
                }
                Err(err) => self.toast(&err.message),
            },
            AppMsg::MentionMove(step) => self.mentions.move_selection(step),
            AppMsg::MentionPick => {
                if let Some(handle) = self.mentions.active_handle() {
                    self.insert_mention(&handle);
                }
            }
            AppMsg::MentionDismiss => self.mentions.hide(),
            AppMsg::MentionChosen(handle) => self.insert_mention(&handle),
            AppMsg::MentionsLoaded(gen, result) => {
                if gen != self.mention_gen {
                    return;
                }
                let Some(view) = self.mentions.view().or_else(|| self.mention_view()) else {
                    return;
                };
                match result {
                    Ok(suggestions) => {
                        let query = self.mention_query(&view).unwrap_or_default();
                        self.mentions.show(&view, &query, &suggestions);
                    }
                    Err(_) => self.mentions.hide(),
                }
            }
            AppMsg::ShareProfile => {
                let url = if let Some(organization) = &self.organization {
                    organization.html_url(&devnull_api::default_base_url())
                } else if let Some(profile) = &self.profile {
                    profile.html_url(&devnull_api::default_base_url())
                } else {
                    return;
                };
                if ui::copy_text(&url) {
                    self.toast("Profile link copied");
                } else {
                    self.toast("Could not share profile");
                }
            }
            AppMsg::CopyProfileId => {
                let id = self
                    .organization
                    .as_ref()
                    .map(|organization| organization.id.clone())
                    .filter(|id| !id.is_empty())
                    .or_else(|| {
                        self.profile
                            .as_ref()
                            .map(|profile| profile.user.id.clone())
                            .filter(|id| !id.is_empty())
                    });
                match id {
                    Some(id) if ui::copy_text(&id) => self.toast("Profile ID copied"),
                    Some(_) => self.toast("Could not copy profile ID"),
                    None => self.toast("This profile has no ID to copy"),
                }
            }
            AppMsg::PickEmoji => {
                let view = self.pick_emoji_view();
                let items = crate::emoji::catalog(24);
                self.mentions.hide();
                self.emoji.show(&view, "", &items);
            }
            AppMsg::ClaimOrganization => {
                let Some(handle) = self
                    .organization
                    .as_ref()
                    .map(|organization| organization.handle.clone())
                else {
                    return;
                };
                if let Some(client) = self.client.clone() {
                    let sender2 = sender.clone();
                    relm4::spawn(async move {
                        sender2.input(AppMsg::OrganizationClaimed(
                            handle.clone(),
                            client.claim_organization(&handle).await,
                        ));
                    });
                }
            }
            AppMsg::OrganizationClaimed(handle, result) => match result {
                Ok(()) => {
                    self.toast(&format!("You're an admin of @{handle}."));
                    sender.input(AppMsg::OpenOrganization(handle));
                }
                Err(err) => self.toast(&err.message),
            },
            AppMsg::FollowOrganization(handle, follow) => {
                if let Some(client) = self.client.clone() {
                    let sender2 = sender.clone();
                    relm4::spawn(async move {
                        let result = if follow {
                            client.follow_organization(&handle).await
                        } else {
                            client.unfollow_organization(&handle).await
                        };
                        match result {
                            Ok(()) => sender2.input(AppMsg::OrganizationFollowed(handle, follow)),
                            Err(err) => sender2.input(AppMsg::Toast(err.message)),
                        }
                    });
                }
            }
            AppMsg::OrganizationFollowed(handle, follow) => {
                self.toast(&format!(
                    "{} @{handle}",
                    if follow { "Following" } else { "Unfollowed" }
                ));
                if let Some(organization) = &mut self.organization {
                    if organization.handle.eq_ignore_ascii_case(&handle) {
                        organization.is_following = follow;
                        let count = organization.followers_count.unwrap_or(0);
                        organization.followers_count = Some(if follow {
                            count + 1
                        } else {
                            count.saturating_sub(1)
                        });
                    }
                }
                if self.visible == "connections" {
                    if let Some(organization) = self
                        .connections_orgs
                        .iter_mut()
                        .find(|organization| organization.handle.eq_ignore_ascii_case(&handle))
                    {
                        organization.is_following = follow;
                    }
                    self.render_connections(&sender);
                }
                if self.visible == "profile" && self.organization.is_some() {
                    self.render_profile(&sender);
                }
            }
            AppMsg::AppearanceChanged => {
                self.refresh_me_avatars();
                ui::refresh_mark_images(self.wordmark_btn.upcast_ref());
                if self.visible == "profile" {
                    self.render_profile(&sender);
                    if self.profile_tab == ProfileFeed::Readme {
                        self.render_profile_readme(&sender);
                    }
                }
            }
            AppMsg::CopyHomeLink => {
                let url = devnull_api::default_base_url();
                if ui::copy_text(&url) {
                    self.toast("Copied link");
                } else {
                    self.toast("fatal: could not copy link");
                }
            }
            AppMsg::CopyCommitLink => {
                if let Some(commit) = self.open_commit.clone() {
                    self.copy_permalink(&commit);
                }
            }
            AppMsg::CopyPermalink(id) => {
                if let Some(commit) = self.commit_by_id(&id).cloned() {
                    self.copy_permalink(&commit);
                }
            }
            AppMsg::ReportCommit(commit) => {
                ui::report_window(&commit, sender.input_sender().clone()).present();
            }
            AppMsg::SubmitReport {
                id,
                reason,
                details,
            } => {
                if let Some(client) = self.client.clone() {
                    let sender2 = sender.clone();
                    relm4::spawn(async move {
                        sender2.input(AppMsg::ReportSubmitted(
                            client
                                .report_commit(&id, &reason, Some(details.as_str()))
                                .await,
                        ));
                    });
                }
            }
            AppMsg::ReportSubmitted(Ok(())) => {
                self.toast("Report received. Moderators will review it privately.");
            }
            AppMsg::ReportSubmitted(Err(err)) => self.fail(err),
            AppMsg::ShowShortcuts => {
                let dialog = ui::shortcuts_dialog(self.remotes_on());
                dialog.present(Some(&self.log_scroll));
                ui::apply_pointer_cursors(&dialog);
            }
            AppMsg::ShortcutCompose => self.open_compose(),
            AppMsg::FocusComposer => {
                self.composer.grab_focus();
            }
            AppMsg::MoveCommit(step) => {
                ui::move_commit_focus(self.visible_commit_list(), step);
            }
            AppMsg::ShortcutOnCommit(key) => {
                let Some(id) = ui::focused_commit_id(self.visible_commit_list()) else {
                    return;
                };
                match key {
                    'y' => sender.input(AppMsg::CopyPermalink(id)),
                    'r' => sender.input(AppMsg::ReplyTo(id)),
                    'o' => sender.input(AppMsg::ToggleReplies(id)),
                    'q' => {
                        if let Some(commit) = self.commit_by_id(&id).cloned() {
                            sender.input(AppMsg::StartQuote(commit));
                        }
                    }
                    's' => self.shortcut_star(&id, &sender),
                    _ => {}
                }
            }
            AppMsg::EditOnGitHub => {
                sender.input(AppMsg::OpenWeb(
                    "https://github.com/settings/profile".into(),
                ));
            }
            AppMsg::SyncProfile => {
                if let Some(client) = self.client.clone() {
                    self.begin_busy();
                    let sender2 = sender.clone();
                    relm4::spawn(async move {
                        sender2.input(AppMsg::ProfileSynced(client.sync_github().await));
                    });
                }
            }
            AppMsg::ProfileSynced(result) => {
                self.end_busy();
                match result {
                    Ok(user) => {
                        self.set_user(Some(user));
                        self.toast("Synced from GitHub");
                        if self.visible == "settings" {
                            self.render_settings(&sender);
                        } else if let Some(username) =
                            self.user.as_ref().map(|u| u.username().to_owned())
                        {
                            sender.input(AppMsg::OpenProfile(username));
                        }
                    }
                    Err(err) => self.toast(&err.message),
                }
            }
            AppMsg::DraftLane(lane) => {
                self.draft_lane = Some(lane);
                self.render_settings(&sender);
            }
            AppMsg::SetLane(lane) => {
                if let Some(client) = self.client.clone() {
                    self.begin_busy();
                    let sender2 = sender.clone();
                    relm4::spawn(async move {
                        sender2.input(AppMsg::LaneSaved(client.set_lane(lane.as_ref()).await));
                    });
                }
            }
            AppMsg::LaneSaved(result) => {
                self.end_busy();
                match result {
                    Ok(user) => {
                        self.set_user(Some(user));
                        self.draft_lane = None;
                        if self.visible == "settings" {
                            self.render_settings(&sender);
                        } else if let Some(username) =
                            self.user.as_ref().map(|u| u.username().to_owned())
                        {
                            sender.input(AppMsg::OpenProfile(username));
                        }
                    }
                    Err(err) => self.toast(&err.message),
                }
            }
            AppMsg::InspectAchievements => {
                if let (Some(profile), Some(achievements)) = (
                    self.profile.as_ref(),
                    self.profile.as_ref().and_then(|p| p.achievements.clone()),
                ) {
                    let dark = ui::adw_is_dark();
                    let lane = ui::lane_color(Some(&profile.user), dark);
                    let lane_class = ui::lane_class(Some(&profile.user));
                    ui::achievements_dialog(&achievements, &profile.user, &lane, lane_class)
                        .present(Some(&self.log_scroll));
                }
            }
            AppMsg::RunSearch(q) => {
                let q = q.trim().to_string();
                if q.is_empty() {
                    return;
                }
                self.omnibox.close();
                self.visible = "search";
                ui::clear_list(&self.search_list);
                ui::append_commit_skeletons(&self.search_list, 3);
                if let Some(client) = self.client.clone() {
                    self.begin_busy();
                    let sender2 = sender.clone();
                    relm4::spawn(async move {
                        sender2.input(AppMsg::SearchLoaded(client.search(&q).await));
                    });
                }
            }
            AppMsg::SearchLoaded(result) => {
                self.end_busy();
                self.user_card.dismiss();
                match result {
                    Ok(results) => ui::append_search(
                        &self.search_list,
                        &results,
                        &self.search_feed_ctx(&sender),
                    ),
                    Err(err) => {
                        ui::clear_list(&self.search_list);
                        self.fail(err);
                    }
                }
            }
            AppMsg::RefreshMeAvatars => self.refresh_me_avatars(),
            AppMsg::RefreshSuggested => {
                if let Some(client) = self.client.clone() {
                    ui::fill_suggestion_skeletons(&self.suggested_list);
                    let live_client = client.clone();
                    let topics_client = client.clone();
                    let follow_client = client.clone();
                    let me = self.user.as_ref().map(|user| user.username().to_owned());
                    let known = self.following.clone();
                    let sender2 = sender.clone();
                    relm4::spawn(async move {
                        let following = follow_client.following_set(me.as_deref(), known).await;
                        sender2.input(AppMsg::SuggestedLoaded(
                            client.suggested_excluding(&following).await,
                            following,
                        ));
                    });
                    let sender2 = sender.clone();
                    relm4::spawn(async move {
                        sender2.input(AppMsg::LiveFollowingLoaded(
                            live_client.live_following().await,
                        ));
                    });
                    let sender2 = sender.clone();
                    relm4::spawn(async move {
                        sender2.input(AppMsg::HotTopicsLoaded(topics_client.hot_topics().await));
                    });
                }
            }
            AppMsg::SuggestedLoaded(result, following) => {
                self.following.extend(following);
                if let Ok(users) = result {
                    self.suggested = users;
                    self.render_suggestions(&sender);
                    self.maybe_refresh_who_to_follow(&sender);
                }
            }
            AppMsg::LiveFollowingLoaded(result) => {
                if let Ok(users) = result {
                    self.set_live_following(users, &sender);
                }
            }
            AppMsg::HotTopicsLoaded(result) => {
                if let Ok(topics) = result {
                    self.set_hot_topics(topics, &sender);
                }
            }
            AppMsg::DismissSuggested(username) => {
                self.suggested.retain(|user| user.username != username);
                self.render_suggestions(&sender);
                self.maybe_refresh_who_to_follow(&sender);
            }
            AppMsg::UserFollowed(username, follow) => {
                self.toast(&format!(
                    "{} @{username}",
                    if follow { "Following" } else { "Unfollowed" }
                ));
                self.user_card.patch(&username, |preview| {
                    preview.is_following = follow;
                    preview.user.is_followed = Some(follow);
                    let count = preview.user.followers_count.unwrap_or(0);
                    preview.user.followers_count = Some(if follow {
                        count + 1
                    } else {
                        count.saturating_sub(1)
                    });
                });
                self.user_card.refresh(
                    self.user.as_ref().map(|user| user.user.id.as_str()),
                    sender.input_sender(),
                );
                let name = username.trim().to_ascii_lowercase();
                if follow {
                    self.following.insert(name);
                    self.suggested
                        .retain(|user| !user.username.eq_ignore_ascii_case(&username));
                    self.render_suggestions(&sender);
                    self.maybe_refresh_who_to_follow(&sender);
                } else {
                    self.following.remove(&name);
                }
                sender.input(AppMsg::RefreshSuggested);
                if self.visible == "connections" {
                    if let Some(user) = self
                        .connections_users
                        .iter_mut()
                        .find(|user| user.username.eq_ignore_ascii_case(&username))
                    {
                        user.is_followed = Some(follow);
                    }
                    self.render_connections(&sender);
                }
                if self.profile.as_ref().map(|p| p.username()) == Some(username.as_str())
                    && self.visible == "profile"
                {
                    sender.input(AppMsg::OpenProfile(username));
                }
            }
            AppMsg::PreviewUser(username) => self.load_user_preview(&username, &sender),
            AppMsg::UserPreviewLoaded(username, result) => {
                self.user_preview_loading
                    .remove(&UserPreview::key(&username));
                if let Ok(mut preview) = result {
                    preview.is_blocking = self.is_blocked(&username);
                    preview.is_muting = preview.is_muting || self.is_muted(&username);
                    self.user_card.insert(preview.clone());
                    self.user_card.apply(
                        &preview,
                        self.user.as_ref().map(|user| user.user.id.as_str()),
                        sender.input_sender(),
                    );
                }
            }
            AppMsg::MuteUser(username) => {
                if let Some(client) = self.client.clone() {
                    self.begin_busy();
                    let sender2 = sender.clone();
                    relm4::spawn(async move {
                        sender2.input(AppMsg::UserMuted(
                            username.clone(),
                            client.mute(&username).await,
                        ));
                    });
                }
            }
            AppMsg::UserMuted(username, result) => {
                self.end_busy();
                match result {
                    Ok(()) => {
                        self.user_card
                            .patch(&username, |preview| preview.is_muting = true);
                        self.user_card.refresh(
                            self.user.as_ref().map(|user| user.user.id.as_str()),
                            sender.input_sender(),
                        );
                        self.toast(&format!("Muted @{username}."));
                        self.load_safety(&sender);
                    }
                    Err(err) => self.toast(&err.message),
                }
            }
            AppMsg::ConfirmBlockUser(username) => {
                ui::confirm_block_developer(&username, sender.input_sender().clone());
            }
            AppMsg::BlockUser(username) => {
                if let Some(client) = self.client.clone() {
                    self.begin_busy();
                    let sender2 = sender.clone();
                    relm4::spawn(async move {
                        sender2.input(AppMsg::UserBlocked(
                            username.clone(),
                            client.block(&username).await,
                        ));
                    });
                }
            }
            AppMsg::UserBlocked(username, result) => {
                self.end_busy();
                match result {
                    Ok(()) => {
                        self.user_card.patch(&username, |preview| {
                            preview.is_blocking = true;
                            preview.is_unavailable = true;
                            preview.is_following = false;
                            preview.user.is_followed = Some(false);
                        });
                        self.user_card.refresh(
                            self.user.as_ref().map(|user| user.user.id.as_str()),
                            sender.input_sender(),
                        );
                        self.toast(&format!("Blocked @{username}."));
                        self.load_safety(&sender);
                    }
                    Err(err) => self.toast(&err.message),
                }
            }
            AppMsg::FollowUser(username, follow) => {
                if let Some(client) = self.client.clone() {
                    let sender2 = sender.clone();
                    relm4::spawn(async move {
                        let result = if follow {
                            client.follow(&username).await
                        } else {
                            client.unfollow(&username).await
                        };
                        match result {
                            Ok(()) => sender2.input(AppMsg::UserFollowed(username, follow)),
                            Err(err) => sender2.input(AppMsg::Toast(err.message)),
                        }
                    });
                }
            }
            AppMsg::ToggleFollowCurrent => {
                if let Some(organization) = self.organization.clone() {
                    sender.input(AppMsg::FollowOrganization(
                        organization.handle,
                        !organization.is_following,
                    ));
                    return;
                }
                let Some(profile) = self.profile.clone() else {
                    return;
                };
                let follow = !profile.is_following;
                sender.input(AppMsg::FollowUser(profile.username().to_owned(), follow));
            }
            AppMsg::AttachImages => {
                if self.composer_poll || self.composer_images.len() >= 3 {
                    return;
                }
                pick_images(&sender);
            }
            AppMsg::ImagesPicked(files) => {
                if self.visible != "remotes" && self.composer_poll {
                    return;
                }
                let dest = if self.visible == "remotes" {
                    &mut self.remote_images
                } else {
                    &mut self.composer_images
                };
                let room = 3usize.saturating_sub(dest.len());
                if files.len() > room {
                    dest.extend(files.into_iter().take(room));
                    self.toast("Up to 3 images per commit.");
                } else {
                    dest.extend(files);
                }
                self.sync_image_previews(&sender);
                self.sync_composer_ring();
            }
            AppMsg::RemoveComposerImage(index) => {
                if index < self.composer_images.len() {
                    self.composer_images.remove(index);
                    self.sync_image_previews(&sender);
                    self.sync_composer_ring();
                }
            }
            AppMsg::RemoveRemoteImage(index) => {
                if index < self.remote_images.len() {
                    self.remote_images.remove(index);
                    self.sync_image_previews(&sender);
                }
            }
            AppMsg::SubmitReply => {
                let (parent, body) = if let Some(parent) = self.replying_to.clone() {
                    (parent, buffer_text(&self.inline_reply))
                } else if let Some(parent) = self.open_commit.as_ref().map(|c| c.id.clone()) {
                    (parent, buffer_text(&self.reply_composer))
                } else {
                    self.toast("fatal: no commit open");
                    return;
                };
                if body.trim().is_empty() {
                    self.toast("fatal: nothing to commit");
                    return;
                }
                if let Some(client) = self.client.clone() {
                    self.begin_busy();
                    let sender2 = sender.clone();
                    let author = self.posting_as_handle();
                    relm4::spawn(async move {
                        sender2.input(AppMsg::CommitCreated(
                            client
                                .create_commit(
                                    &body,
                                    Some(&parent),
                                    None,
                                    Vec::new(),
                                    None,
                                    None,
                                    author.as_deref(),
                                )
                                .await,
                        ));
                    });
                }
            }
            AppMsg::SubmitCommit => {
                let body = buffer_text(&self.composer);
                if body.trim().chars().count() < MIN_BODY && self.composer_images.is_empty() {
                    self.toast("fatal: nothing to commit");
                    return;
                }
                if body.chars().count() > MAX_BODY {
                    self.toast("fatal: commit is too long");
                    return;
                }
                let poll = if self.composer_poll {
                    match self.poll_ui.draft() {
                        Some(poll) => Some(poll),
                        None => {
                            self.toast("fatal: a poll needs two options");
                            return;
                        }
                    }
                } else {
                    None
                };
                let scheduled = if self.composer_schedule {
                    match self.schedule_ui.rfc3339() {
                        Ok(at) => Some(at),
                        Err(err) => {
                            self.toast(err);
                            return;
                        }
                    }
                } else {
                    None
                };
                if let Some(client) = self.client.clone() {
                    let images = if poll.is_some() {
                        Vec::new()
                    } else {
                        std::mem::take(&mut self.composer_images)
                    };
                    let sender2 = sender.clone();
                    let author = self.posting_as_handle();
                    self.begin_busy();
                    relm4::spawn(async move {
                        sender2.input(AppMsg::CommitCreated(
                            client
                                .create_commit(
                                    &body,
                                    None,
                                    None,
                                    images,
                                    poll,
                                    scheduled.as_deref(),
                                    author.as_deref(),
                                )
                                .await,
                        ));
                    });
                }
            }
            AppMsg::CommitCreated(Ok(commit)) => {
                self.end_busy();
                self.emoji.hide();
                self.composer.buffer().set_text("");
                self.reply_composer.buffer().set_text("");
                self.poll_ui.reset();
                self.schedule_ui.reset();
                self.composer_poll = false;
                self.composer_schedule = false;
                self.composer_preview = false;
                self.composer_len = 0;
                self.composer_as = None;
                self.refresh_me_avatars();
                self.sync_image_previews(&sender);
                self.sync_composer_ring();
                let hash = commit
                    .short_hash
                    .clone()
                    .unwrap_or_else(|| commit.id[..7.min(commit.id.len())].to_owned());
                let body = commit.body.clone().unwrap_or_default();
                let excerpt: String = body.chars().take(48).collect();
                self.toast_commit(&format!("[log {hash}] {excerpt}"), commit.id.clone());
                if self.compose_open {
                    self.compose_dialog.close();
                }
                if let Some(parent) = commit
                    .parent_id
                    .clone()
                    .or_else(|| self.replying_to.clone())
                {
                    self.bump_replies(&parent);
                    self.nested_replies
                        .entry(parent.clone())
                        .or_default()
                        .insert(0, commit.clone());
                    self.expanded_replies.insert(parent.clone());
                    if self
                        .open_commit
                        .as_ref()
                        .is_some_and(|open| open.id == parent)
                    {
                        self.thread_replies.insert(0, commit);
                    }
                    self.replying_to = None;
                    self.inline_reply.buffer().set_text("");
                    self.reply_composer.buffer().set_text("");
                    self.refresh_visible_feed(&sender, true);
                } else if let Some(id) = self.open_commit.as_ref().map(|c| c.id.clone()) {
                    sender.input(AppMsg::OpenCommit(id));
                } else {
                    self.load_log(&sender, false);
                }
            }
            AppMsg::CommitCreated(Err(err)) => {
                self.end_busy();
                if err.is_commit_rules() {
                    self.toast("Your first commit has to be on the web.");
                    sender.input(AppMsg::OpenWeb(format!(
                        "{}/",
                        devnull_api::default_base_url()
                    )));
                } else {
                    self.fail(err);
                }
            }
            AppMsg::NotificationsLoaded(Ok(page)) => {
                self.end_busy();
                self.notifications = page.data;
                self.notif_groups.clear();
                self.render_notifications(&sender);
            }
            AppMsg::ToggleNotificationGroup(id) => {
                let needs_fetch = {
                    let entry = self.notif_groups.entry(id.clone()).or_default();
                    if entry.open {
                        entry.open = false;
                        false
                    } else {
                        entry.open = true;
                        let fetch = entry.items.is_empty() && !entry.loading;
                        if fetch {
                            entry.loading = true;
                            entry.failed = false;
                        }
                        fetch
                    }
                };
                self.render_notifications(&sender);
                if needs_fetch {
                    self.fetch_notification_group(id, false, &sender);
                }
            }
            AppMsg::LoadMoreNotificationGroup(id) => {
                let more = {
                    let Some(entry) = self.notif_groups.get_mut(&id) else {
                        return;
                    };
                    if entry.loading {
                        return;
                    }
                    entry.loading = true;
                    entry.failed = false;
                    !entry.items.is_empty()
                };
                self.render_notifications(&sender);
                self.fetch_notification_group(id, more, &sender);
            }
            AppMsg::NotificationGroupLoaded(id, more, result) => {
                if let Some(entry) = self.notif_groups.get_mut(&id) {
                    entry.loading = false;
                    match result {
                        Ok(page) => {
                            entry.failed = false;
                            entry.cursor = page.next_cursor();
                            if more {
                                entry.items.extend(page.data);
                            } else {
                                entry.items = page.data;
                            }
                        }
                        Err(_) => entry.failed = true,
                    }
                }
                self.render_notifications(&sender);
            }
            AppMsg::NotificationsLoaded(Err(err)) => {
                self.end_busy();
                ui::clear_list(&self.notif_list);
                self.notif_list.append(&ui::empty_row(
                    "fatal: could not load notifications",
                    &err.message,
                ));
                self.fail(err);
            }
            AppMsg::RemotesLoaded(Ok(page)) => {
                self.alert_incoming_remotes(&page.data);
                self.unread_remotes = page.unread_count;
                self.inbox_requests = page.requests_count;
                self.inbox = page.data;
                self.sync_chat_dock();
                if self.inbox_query.trim().is_empty() {
                    self.inbox_hits = None;
                    self.inbox_people.clear();
                    self.render_inbox(&sender);
                }
            }
            AppMsg::RemotesLoaded(Err(err)) => {
                self.inbox.clear();
                self.inbox_hits = None;
                self.inbox_people.clear();
                ui::clear_list(&self.inbox_list);
                self.inbox_list.append(&ui::empty_row(
                    "fatal: could not load remotes",
                    &err.message,
                ));
                self.fail(err);
            }
            AppMsg::InboxTab(tab) => {
                if self.inbox_tab == tab {
                    return;
                }
                self.inbox_tab = tab;
                self.inbox_hits = None;
                self.inbox_people.clear();
                self.inbox.clear();
                ui::clear_list(&self.inbox_list);
                if self.inbox_query.trim().is_empty() {
                    self.load_remotes(&sender);
                } else {
                    self.search_inbox(&sender);
                }
            }
            AppMsg::InboxQuery(query) => {
                self.inbox_query = query;
                if self.inbox_query.trim().is_empty() {
                    self.inbox_hits = None;
                    self.inbox_people.clear();
                    self.render_inbox(&sender);
                } else {
                    self.inbox_hits = None;
                    self.render_inbox(&sender);
                    self.search_inbox(&sender);
                }
            }
            AppMsg::InboxSearchLoaded(gen, query, page, people) => {
                if gen != self.inbox_search_gen || query != self.inbox_query {
                    return;
                }
                if let Ok(page) = page {
                    self.unread_remotes = page.unread_count;
                    let ids: HashSet<_> = page
                        .data
                        .iter()
                        .filter_map(|item| item.partner.as_ref().map(|user| user.id.clone()))
                        .collect();
                    self.inbox_people = people
                        .into_iter()
                        .filter(|user| !ids.contains(&user.id))
                        .collect();
                    self.inbox_hits = Some(page.data);
                    self.render_inbox(&sender);
                }
            }
            AppMsg::ToggleChatDock => {
                self.prefs.chat_docked = !self.prefs.chat_docked;
                if self.prefs.chat_docked {
                    self.chat_dock_expanded = false;
                }
                self.prefs.save();
                self.sync_chat_dock();
            }
            AppMsg::ToggleDockExpand => {
                self.chat_dock_expanded = !self.chat_dock_expanded;
                self.sync_chat_dock();
            }
            AppMsg::PickRemote => self.people_picker.open_remote(),
            AppMsg::PickOrg => self.people_picker.open_org(),
            AppMsg::PeopleQuery(query) => {
                let trimmed = query.trim().trim_start_matches('@').to_string();
                if trimmed.is_empty() {
                    self.people_picker
                        .set_status(Some("Type a name or @handle"));
                    self.people_picker.clear_results();
                    return;
                }
                self.people_picker.show_skeletons();
                self.search_people(&sender, trimmed);
            }
            AppMsg::PeopleLoaded(gen, result) => {
                if gen != self.people_gen {
                    return;
                }
                match result {
                    Ok(people) => {
                        let me_id = self.user.as_ref().map(|user| user.user.id.as_str());
                        self.people_picker.show_people(me_id, &people);
                    }
                    Err(err) => self.people_picker.set_status(Some(&err.message)),
                }
            }
            AppMsg::CreateOrg { name, members } => {
                if let Some(client) = self.client.clone() {
                    self.begin_busy();
                    let sender2 = sender.clone();
                    relm4::spawn(async move {
                        sender2.input(AppMsg::OrgCreated(client.create_org(&name, &members).await));
                    });
                }
            }
            AppMsg::OrgCreated(result) => {
                self.end_busy();
                match result {
                    Ok(reference) => {
                        self.people_picker.close();
                        sender.input(AppMsg::OpenThread(reference));
                        self.load_remotes(&sender);
                    }
                    Err(err) => self.toast(&err.message),
                }
            }
            AppMsg::SetRemoteFrom(from) => {
                let pull_receipts = {
                    let Some(remotes) = self.user.as_ref().and_then(|user| user.remotes.as_ref())
                    else {
                        return;
                    };
                    if remotes.from == from {
                        return;
                    }
                    remotes.pull_receipts
                };
                self.save_remote_prefs(from, pull_receipts, &sender);
            }
            AppMsg::SetPullReceipts(pull_receipts) => {
                let from = {
                    let Some(remotes) = self.user.as_ref().and_then(|user| user.remotes.as_ref())
                    else {
                        return;
                    };
                    if remotes.pull_receipts == pull_receipts {
                        return;
                    }
                    if remotes.everyone {
                        remotes.from.clone()
                    } else {
                        "following".into()
                    }
                };
                self.save_remote_prefs(from, pull_receipts, &sender);
            }
            AppMsg::RemotePrefsSaved(result) => {
                self.end_busy();
                match result {
                    Ok(user) => {
                        self.set_user(Some(user));
                        self.toast("Remote settings saved.");
                    }
                    Err(err) => self.toast(&err.message),
                }
            }
            AppMsg::OpenThread(reference) => {
                if self.remotes_on() {
                    self.nav = Nav::Remotes;
                    self.visible = "remotes";
                }
                let switching = self.thread_ref.as_ref() != Some(&reference);
                self.thread_ref = Some(reference.clone());
                if switching {
                    self.thread_follow.set(true);
                    self.thread_typing.clear();
                    self.open_thread = None;
                    self.remote_reply = None;
                    self.last_thread_ping = None;
                    self.thread_header_stamp.clear();
                    self.fill_open_thread_header(None, sender.input_sender().clone());
                    ui::clear_list(&self.thread_list);
                    ui::append_thread_skeletons(&self.thread_list);
                }
                self.render_inbox(&sender);
                self.fetch_open_thread(&sender);
            }
            AppMsg::ThreadLoaded(gen, Ok(thread)) => {
                if gen != self.thread_gen {
                    return;
                }
                self.apply_open_thread(thread, &sender);
            }
            AppMsg::ThreadLoaded(gen, Err(err)) => {
                if gen != self.thread_gen {
                    return;
                }
                self.fail(err);
            }
            AppMsg::PinRemote => self.toggle_pin_remote(&sender),
            AppMsg::StashRemote => self.stash_open_remote(&sender),
            AppMsg::MuteRemote => self.mute_open_remote(&sender),
            AppMsg::LeaveOrg => {
                let title = self
                    .thread_ref
                    .as_ref()
                    .map(ThreadRef::title)
                    .unwrap_or_else(|| "this org".into());
                ui::confirm_leave_org(&title, sender.input_sender().clone());
            }
            AppMsg::BlockRemote => {
                let Some(ThreadRef::Direct { username }) = &self.thread_ref else {
                    return;
                };
                ui::confirm_block_user(username, sender.input_sender().clone());
            }
            AppMsg::ConfirmLeaveOrg => self.leave_open_org(&sender),
            AppMsg::ConfirmBlockRemote => self.block_open_remote(&sender),
            AppMsg::ThreadActionDone(result) => {
                self.end_busy();
                match result {
                    Ok(action) => {
                        self.toast(&action);
                        if action.starts_with("Stashed")
                            || action.starts_with("Popped")
                            || action.starts_with("Left")
                            || action.starts_with("Blocked")
                        {
                            if action.starts_with("Left") || action.starts_with("Blocked") {
                                self.thread_ref = None;
                                self.open_thread = None;
                                ui::clear_list(&self.thread_list);
                            }
                            self.load_remotes(&sender);
                            if self.thread_ref.is_some() {
                                self.fetch_open_thread(&sender);
                            }
                            self.fill_open_thread_header(
                                self.open_thread.as_ref(),
                                sender.input_sender().clone(),
                            );
                            self.render_inbox(&sender);
                        } else if action.starts_with("Muted") || action.starts_with("Unmuted") {
                            self.load_remotes(&sender);
                            if self.thread_ref.is_some() {
                                self.fetch_open_thread(&sender);
                            }
                            self.thread_header_stamp.clear();
                            self.fill_open_thread_header(
                                self.open_thread.as_ref(),
                                sender.input_sender().clone(),
                            );
                        }
                    }
                    Err(err) => {
                        self.toast(&err.message);
                        if self.thread_ref.is_some() {
                            self.load_remotes(&sender);
                            self.fetch_open_thread(&sender);
                        }
                    }
                }
            }
            AppMsg::PingMention => {
                let insert = self.ping_insert_text();
                self.remote_composer.buffer().insert_at_cursor(&insert);
                self.remote_composer.grab_focus();
            }
            AppMsg::StartCall => {
                let Some(reference) = &self.thread_ref else {
                    self.toast("fatal: no remote selected");
                    return;
                };
                sender.input(AppMsg::OpenWeb(
                    reference.html_url(&devnull_api::default_base_url()),
                ));
            }
            AppMsg::PinThreadMessage(id) => self.pin_thread_message(id, &sender),
            AppMsg::ConfirmPinThreadMessage { id, dismiss } => {
                if dismiss {
                    self.prefs.dismiss_pin_warning();
                }
                self.request_message_pin(id, true, &sender);
            }
            AppMsg::ThreadPinsLoaded(key, pinning, result) => {
                if self.thread_ref.as_ref().map(ThreadRef::key).as_deref() != Some(key.as_str()) {
                    return;
                }
                match result {
                    Ok(pinned) => {
                        if let Some(thread) = &mut self.open_thread {
                            thread.pinned = pinned;
                        }
                        self.toast(if pinning {
                            "Pinned message."
                        } else {
                            "Unpinned message."
                        });
                        self.thread_header_stamp.clear();
                        self.fill_open_thread_header(
                            self.open_thread.as_ref(),
                            sender.input_sender().clone(),
                        );
                        if let Some(thread) = &self.open_thread {
                            ui::refresh_thread_pins(
                                &self.thread_list,
                                &thread.pinned.iter().map(|message| message.id).collect(),
                                self.can_pin_org_messages(Some(thread)),
                            );
                        }
                    }
                    Err(err) => self.toast(&err.message),
                }
            }
            AppMsg::JumpToThreadMessage(id) => {
                ui::scroll_to_thread_message(&self.thread_scroll, &self.thread_list, id);
            }
            AppMsg::ReplyToRemote(mut message) => {
                if message.user.is_none() {
                    if let (Some(thread), Some(id)) =
                        (self.open_thread.as_ref(), message.user_id.as_deref())
                    {
                        message.user =
                            thread_people(thread, self.user.as_ref().map(|user| &user.user))
                                .get(id)
                                .cloned();
                    }
                }
                self.remote_reply = Some(message);
                self.remote_composer.grab_focus();
            }
            AppMsg::CancelRemoteReply => {
                self.remote_reply = None;
                self.remote_composer.grab_focus();
            }
            AppMsg::ReactToRemote { id, emoji } => {
                self.toggle_message_reaction(id, &emoji, &sender);
            }
            AppMsg::MessageReactionsLoaded(key, id, result) => {
                if self.thread_ref.as_ref().map(ThreadRef::key).as_deref() != Some(key.as_str()) {
                    return;
                }
                match result {
                    Ok(reactions) => {
                        self.set_message_reactions(id, reactions, &sender);
                    }
                    Err(err) => {
                        self.toast(&err.message);
                        self.fetch_open_thread(&sender);
                    }
                }
            }
            AppMsg::RecordVoice => {
                self.toast("Voice notes aren't on remotes yet.");
            }
            AppMsg::AttachRemoteImages => {
                if self.remote_images.len() >= 3 {
                    return;
                }
                pick_images(&sender);
            }
            AppMsg::PushRemote => {
                let body = buffer_text(&self.remote_composer);
                let Some(reference) = self.thread_ref.clone() else {
                    self.toast("fatal: no remote selected");
                    return;
                };
                if body.trim().is_empty() && self.remote_images.is_empty() {
                    self.toast("fatal: nothing to push");
                    return;
                }
                if let Some(client) = self.client.clone() {
                    let images = std::mem::take(&mut self.remote_images);
                    let reply_to_id = self.remote_reply.as_ref().map(|message| message.id);
                    let sender2 = sender.clone();
                    let client_id = uuid::Uuid::new_v4().to_string();
                    relm4::spawn(async move {
                        sender2.input(AppMsg::RemotePushed(
                            client
                                .push(&reference, &body, &client_id, images, reply_to_id)
                                .await,
                        ));
                    });
                }
            }
            AppMsg::RemotePushed(Ok(_)) => {
                self.remote_composer.buffer().set_text("");
                self.remote_draft_len = 0;
                self.remote_reply = None;
                self.sync_image_previews(&sender);
                self.fetch_open_thread(&sender);
            }
            AppMsg::RemotePushed(Err(err)) => self.fail(err),
            AppMsg::Toast(text) => self.toast(&text),
            AppMsg::Poll => {
                if self.phase != Phase::SignedIn {
                    return;
                }
                self.poll_unread(&sender);
                if self.remotes_on() {
                    self.load_remotes(&sender);
                }
                match self.nav {
                    Nav::Notifications => self.load_notifications(&sender),
                    Nav::Remotes => {
                        self.fetch_open_thread(&sender);
                    }
                    _ if self.tracks_new_posts() => {
                        if self.log_away {
                            self.poll_new_posts(&sender);
                        } else {
                            self.load_log(&sender, false);
                        }
                    }
                    _ => {}
                }
            }
            AppMsg::UnreadPolled(Ok((notifications, remotes))) => {
                self.apply_unread(notifications, remotes, true);
            }
            AppMsg::UnreadPolled(Err(_)) => {}
            AppMsg::LogPolled(Ok(page)) => {
                if !self.tracks_new_posts() || !self.log_away {
                    return;
                }
                let Some(seen) = self.log_seen_id.as_deref() else {
                    return;
                };
                let mut authors = Vec::new();
                let mut count = 0u32;
                for commit in &page.data {
                    if commit.id == seen {
                        break;
                    }
                    count += 1;
                    if let Some(author) = &commit.author {
                        if authors.len() < 3
                            && authors.iter().all(|known: &User| known.id != author.id)
                        {
                            authors.push(author.clone());
                        }
                    }
                }
                self.new_posts_count = count;
                self.new_authors = authors;
                self.paint_new_posts();
            }
            AppMsg::LogPolled(Err(_)) => {}
            AppMsg::JumpToNewest => {
                self.log_away = false;
                self.clear_new_posts();
                self.log_scroll.vadjustment().set_value(0.0);
                self.load_log(&sender, false);
            }
            AppMsg::LogScrolled => {
                self.user_card.dismiss();
                let away = self.log_scroll.vadjustment().value() > 48.0;
                self.log_away = away;
                if !away && self.new_posts_count > 0 {
                    sender.input(AppMsg::JumpToNewest);
                } else {
                    self.paint_new_posts();
                }
                if self.log_cursor.is_some() && !self.log_loading_more && self.near_log_end() {
                    sender.input(AppMsg::LoadMore);
                }
            }
            AppMsg::OpenWeb(url) => open_web(&url),
        }
    }

    fn shutdown(&mut self, _widgets: &mut Self::Widgets, _sender: relm4::Sender<()>) {
        self.logo_menu.unparent();
    }
}

impl App {
    fn header_title(&self) -> &str {
        if self.visible == "commit" {
            return "Commit";
        }
        if self.visible == "profile" {
            return "Profile";
        }
        if self.visible == "search" {
            return "Search";
        }
        if self.visible == "connections" {
            return self.connections_kind.title();
        }
        if self.visible == "settings" {
            return "Settings";
        }
        if self.nav == Nav::Starred && self.visible == "log" {
            return "Starred";
        }
        match self.phase {
            Phase::SignedIn => self.nav.title(),
            Phase::WaitingForBrowser => "Approve this device",
            _ => "/dev/null",
        }
    }

    fn sidebar(&self) -> Sidebar {
        match self.visible {
            "notifications" => Sidebar::Notifications,
            "remotes" => Sidebar::Remotes,
            "profile" | "connections" => Sidebar::Profile,
            "settings" => Sidebar::Settings,
            "search" => Sidebar::Search,
            _ if self.nav == Nav::Starred => Sidebar::Starred,
            _ => Sidebar::Log,
        }
    }

    const NAV_EXPAND_WIDTH: f64 = 1024.0;
    const ASIDE_WIDTH: f64 = 1280.0;
    const SHELL_COMPACT_WIDTH: f64 = 768.0;

    fn bind_width_breakpoints(root: &adw::ApplicationWindow, sender: &ComponentSender<Self>) {
        let aside = adw::Breakpoint::new(adw::BreakpointCondition::new_length(
            adw::BreakpointConditionLengthType::MaxWidth,
            Self::ASIDE_WIDTH - 1.0,
            adw::LengthUnit::Px,
        ));
        aside.connect_apply({
            let sender = sender.clone();
            move |_| sender.input(AppMsg::AsideCompact(true))
        });
        aside.connect_unapply({
            let sender = sender.clone();
            move |_| sender.input(AppMsg::AsideCompact(false))
        });
        root.add_breakpoint(aside);

        let nav = adw::Breakpoint::new(adw::BreakpointCondition::new_length(
            adw::BreakpointConditionLengthType::MaxWidth,
            Self::NAV_EXPAND_WIDTH - 1.0,
            adw::LengthUnit::Px,
        ));
        nav.connect_apply({
            let sender = sender.clone();
            move |_| sender.input(AppMsg::NavCompact(true))
        });
        nav.connect_unapply({
            let sender = sender.clone();
            move |_| sender.input(AppMsg::NavCompact(false))
        });
        root.add_breakpoint(nav);

        let shell = adw::Breakpoint::new(adw::BreakpointCondition::new_length(
            adw::BreakpointConditionLengthType::MaxWidth,
            Self::SHELL_COMPACT_WIDTH - 1.0,
            adw::LengthUnit::Px,
        ));
        shell.connect_apply({
            let sender = sender.clone();
            move |_| sender.input(AppMsg::ShellCompact(true))
        });
        shell.connect_unapply({
            let sender = sender.clone();
            move |_| sender.input(AppMsg::ShellCompact(false))
        });
        root.add_breakpoint(shell);
    }

    fn nav_collapsed(&self) -> bool {
        nav_is_collapsed(self.sidebar_collapsed, self.nav_compact)
    }

    fn nav_visible(&self) -> bool {
        self.phase == Phase::SignedIn && !self.shell_compact
    }

    fn nav_search_visible(&self) -> bool {
        nav_shows_search(self.aside_compact, self.nav_compact, self.shell_compact)
    }

    fn mobile_header_visible(&self) -> bool {
        self.phase == Phase::SignedIn && self.shell_compact
    }

    fn aside_visible(&self) -> bool {
        aside_is_visible(
            self.phase == Phase::SignedIn,
            self.aside_compact,
            self.nav_compact,
            self.shell_compact,
            self.visible,
        )
    }

    fn feed_width(&self) -> i32 {
        feed_column_width(self.visible, self.shell_compact)
    }

    fn feed_width_request(&self) -> i32 {
        feed_column_width_request(self.visible, self.shell_compact)
    }

    fn feed_fills(&self) -> bool {
        feed_column_fills(self.shell_compact)
    }

    fn feed_classes(&self) -> [&'static str; 2] {
        feed_column_classes(self.visible, self.shell_compact)
    }

    fn sidebar_width(&self) -> i32 {
        if self.nav_collapsed() {
            80
        } else {
            256
        }
    }

    fn sidebar_classes(&self) -> [&'static str; 2] {
        if self.nav_collapsed() {
            ["dn-sidebar", "collapsed"]
        } else {
            ["dn-sidebar", "expanded"]
        }
    }

    fn nav_rail_align(&self) -> gtk::Align {
        if self.nav_collapsed() {
            gtk::Align::Center
        } else {
            gtk::Align::Fill
        }
    }

    fn profile_bar_name(&self) -> String {
        if let Some(organization) = &self.organization {
            organization.display_name().to_owned()
        } else {
            self.profile
                .as_ref()
                .map(|profile| profile.user.display_name().to_owned())
                .unwrap_or_default()
        }
    }

    fn profile_bar_handle(&self) -> String {
        if let Some(organization) = &self.organization {
            format!("@{}", organization.handle)
        } else {
            self.profile
                .as_ref()
                .map(|profile| format!("@{}", profile.user.username))
                .unwrap_or_default()
        }
    }

    fn profile_bar_commits(&self) -> String {
        if let Some(organization) = &self.organization {
            let n = organization.followers_count.unwrap_or(0);
            if n == 1 {
                "1 follower".into()
            } else {
                format!("{n} followers")
            }
        } else {
            let n = self
                .profile
                .as_ref()
                .and_then(|profile| profile.user.commits_count)
                .unwrap_or(0);
            ui::commits_label(n)
        }
    }

    fn collapse_label(&self) -> &'static str {
        if self.nav_collapsed() {
            "Expand"
        } else {
            "Collapse"
        }
    }

    fn collapse_icon(&self) -> &'static str {
        if self.nav_collapsed() {
            "dn-panel-left-open-symbolic"
        } else {
            "dn-panel-left-close-symbolic"
        }
    }

    fn composer_icon_classes(&self, active: bool) -> [&'static str; 3] {
        if active {
            ["flat", "dn-composer-icon", "active"]
        } else {
            ["flat", "dn-composer-icon", "inactive"]
        }
    }

    fn composer_placeholder(&self) -> &'static str {
        if self.composer_poll {
            "Ask a question"
        } else {
            "What did you ship?"
        }
    }

    fn image_tooltip(&self) -> &'static str {
        if self.composer_poll {
            "A commit can have images or a poll, not both"
        } else if self.composer_images.len() >= 3 {
            "Up to 3 images per commit."
        } else {
            "Attach images"
        }
    }

    fn poll_tooltip(&self) -> &'static str {
        if !self.composer_images.is_empty() {
            "A commit can have images or a poll, not both"
        } else {
            "Add poll"
        }
    }

    fn schedule_tooltip(&self) -> &'static str {
        if self.composer_schedule {
            "Don't schedule"
        } else {
            "Schedule for later"
        }
    }

    fn composer_preview_markup(&self) -> String {
        let body = buffer_text(&self.composer);
        if body.trim().is_empty() {
            "<span foreground='#5b616b'>Nothing to preview yet.</span>".into()
        } else {
            ui::body_markup(&body)
        }
    }

    fn nav_classes(&self, item: Sidebar) -> [&'static str; 3] {
        if self.sidebar() == item {
            ["flat", "dn-nav-btn", "active"]
        } else {
            ["flat", "dn-nav-btn", "inactive"]
        }
    }

    fn tab_classes(&self, item: Nav) -> [&'static str; 3] {
        if self.nav == item && self.visible == "log" {
            ["flat", "dn-tab", "active"]
        } else {
            ["flat", "dn-tab", "inactive"]
        }
    }

    fn notif_filter_classes(&self, item: NotificationFilter) -> [&'static str; 2] {
        if self.notif_types.contains(&item) {
            ["dn-filter-pill", "active"]
        } else {
            ["dn-filter-pill", "inactive"]
        }
    }

    fn feed_ctx(&self, sender: &ComponentSender<App>) -> ui::FeedCtx {
        ui::FeedCtx {
            sender: sender.input_sender().clone(),
            pasteis_mode: self.user.as_ref().is_some_and(|u| u.pasteis_on()),
            me_id: self.user.as_ref().map(|u| u.user.id.clone()),
            me_username: self.user.as_ref().map(|user| user.username().to_owned()),
            stars: self.stars.clone(),
            pasteis: self.pasteis.clone(),
            expanded: self.expanded_replies.clone(),
            connected: false,
            join_staged: self.visible == "log" && self.nav.log().is_some() && self.user.is_some(),
            hover: Some(self.hover_enabled.clone()),
            automatic_translation: self.automatic_translation(),
            translations: self.translations.clone(),
            showing_original: self.showing_original.clone(),
            requested_translation: self.requested_translation.clone(),
            translation_loading: self.translation_loading.clone(),
            who_to_follow_after: self.who_to_follow_after(),
            suggested: self.suggested.clone(),
            following: self.following.clone(),
            user_card: Some(self.user_card.clone()),
            org_ids: self
                .user
                .as_ref()
                .map(CurrentUser::org_ids)
                .unwrap_or_default(),
        }
    }

    fn search_feed_ctx(&self, sender: &ComponentSender<App>) -> ui::FeedCtx {
        ui::FeedCtx {
            connected: true,
            join_staged: false,
            who_to_follow_after: None,
            ..self.feed_ctx(sender)
        }
    }

    fn thread_state(&self) -> ui::ThreadState<'_> {
        ui::ThreadState {
            nested: &self.nested_replies,
            expanded: &self.expanded_replies,
            replying_to: self.replying_to.as_deref(),
            composer: &self.inline_reply_box,
        }
    }

    fn queue_reply_previews(&mut self, commits: &[Commit], sender: &ComponentSender<App>) {
        self.queue_reply_previews_ids(reply_preview_ids(commits), sender);
    }

    fn queue_reply_previews_ids(&mut self, ids: Vec<String>, sender: &ComponentSender<App>) {
        let Some(client) = self.client.clone() else {
            return;
        };
        let limit = ui::PREVIEW_REPLIES as u32;
        for id in ids {
            if self.nested_replies.contains_key(&id) || !self.preview_loading.insert(id.clone()) {
                continue;
            }
            let sender2 = sender.clone();
            let client = client.clone();
            relm4::spawn(async move {
                sender2.input(AppMsg::PreviewRepliesLoaded(
                    id.clone(),
                    client.replies(&id, None, Some(limit)).await,
                ));
            });
        }
    }

    fn reply_view(&self) -> &gtk::TextView {
        if self.replying_to.is_some() {
            &self.inline_reply
        } else {
            &self.reply_composer
        }
    }

    fn near_log_end(&self) -> bool {
        let adj = self.log_scroll.vadjustment();
        adj.upper() - adj.value() - adj.page_size() < 240.0
    }

    fn bump_replies(&mut self, parent: &str) {
        let bump = |commit: &mut Commit| {
            if commit.id == parent {
                commit.replies_count = Some(commit.replies_count.unwrap_or(0) + 1);
                true
            } else {
                false
            }
        };
        if self.log_commits.iter_mut().any(bump) {
            return;
        }
        if self.profile_commits.iter_mut().any(bump) {
            return;
        }
        if self.thread_replies.iter_mut().any(bump) {
            return;
        }
        for children in self.nested_replies.values_mut() {
            if children.iter_mut().any(bump) {
                return;
            }
        }
        if let Some(commit) = &mut self.open_commit {
            bump(commit);
        }
    }

    fn refresh_visible_feed(&mut self, sender: &ComponentSender<App>, restore: bool) {
        match self.visible {
            "commit" => self.render_thread(sender),
            "profile" if self.profile_tab == ProfileFeed::Readme => {
                self.render_profile_readme(sender)
            }
            "profile" => self.render_profile_feed(sender, restore),
            _ => self.render_log(sender, restore),
        }
    }

    fn render_log(&mut self, sender: &ComponentSender<App>, restore: bool) {
        let value = self.log_scroll.vadjustment().value();
        self.rebuild_log_list();
        self.log_last_day = None;
        self.log_last_lane = None;
        if self.log_commits.is_empty() {
            return;
        }
        let ctx = self.feed_ctx(sender);
        let first_seen = self.log_first_seen.clone();
        ui::append_commits(
            &self.log_list,
            &self.log_commits,
            &ctx,
            &mut self.log_last_lane,
            &mut self.log_last_day,
            first_seen.as_deref(),
            Some(&ui::ThreadState {
                nested: &self.nested_replies,
                expanded: &self.expanded_replies,
                replying_to: self.replying_to.as_deref(),
                composer: &self.inline_reply_box,
            }),
            0,
        );
        if self.log_cursor.is_none() {
            self.log_list.append(&ui::root_row(
                "That's the root commit. There's nothing before it.",
            ));
        }
        if restore {
            self.log_scroll.vadjustment().set_value(value);
        }
    }

    fn rebuild_log_list(&mut self) {
        self.user_card.dismiss();
        ui::unparent(&self.inline_reply_box);
        if self.compose_open {
            ui::clear_list(&self.log_list);
        } else {
            ui::reset_log_list(&self.log_list, &self.log_composer);
        }
    }

    fn open_compose(&mut self) {
        if self.phase != Phase::SignedIn {
            return;
        }
        if self.compose_open {
            self.composer.grab_focus();
            return;
        }
        ui::detach_from_list_row(&self.log_composer);
        self.compose_host.append(&self.log_composer);
        self.log_composer.set_visible(true);
        self.compose_open = true;
        self.compose_dialog.present(Some(&self.log_scroll));
        ui::apply_pointer_cursors(&self.compose_dialog);
        self.composer.grab_focus();
    }

    fn restore_composer(&mut self) {
        if !self.compose_open {
            return;
        }
        self.compose_open = false;
        ui::pin_log_composer(&self.log_list, &self.log_composer);
    }

    fn render_profile_feed(&mut self, sender: &ComponentSender<App>, restore: bool) {
        let value =
            ui::enclosing_scroll(&self.profile_list).map(|scroll| scroll.vadjustment().value());
        self.reset_profile_list();
        if self.profile_commits.is_empty() {
            if let Some(organization) = &self.organization {
                let empty = if self.profile_tab == ProfileFeed::Mentions {
                    format!("Nobody has mentioned @{} yet.", organization.handle)
                } else {
                    format!("@{} hasn't committed yet.", organization.handle)
                };
                self.profile_list.append(&ui::root_row(&empty));
            }
            return;
        }
        let ctx = self.feed_ctx(sender);
        let threads = self.thread_state();
        let mut previous = None;
        let mut last_day = None;
        ui::append_commits(
            &self.profile_list,
            &self.profile_commits,
            &ctx,
            &mut previous,
            &mut last_day,
            None,
            Some(&threads),
            0,
        );
        let end = if let Some(organization) = &self.organization {
            if self.profile_tab == ProfileFeed::Mentions {
                format!(
                    "That's the first commit mentioning @{}.",
                    organization.handle
                )
            } else {
                format!("That's @{}'s first commit.", organization.handle)
            }
        } else {
            let username = self
                .profile
                .as_ref()
                .map(|p| p.username().to_owned())
                .unwrap_or_default();
            let own = self.user.as_ref().map(|u| u.username()) == Some(username.as_str());
            if self.profile_tab == ProfileFeed::Replies {
                if own {
                    "That was your first reply.".into()
                } else {
                    "That's their first reply.".into()
                }
            } else if own {
                "That was your first commit. Everyone starts somewhere.".into()
            } else {
                "That's the root commit. There's nothing before it.".into()
            }
        };
        self.profile_list.append(&ui::root_row(&end));
        if let Some(value) = value.filter(|_| restore) {
            if let Some(scroll) = ui::enclosing_scroll(&self.profile_list) {
                scroll.vadjustment().set_value(value);
            }
        }
    }

    fn set_user(&mut self, user: Option<CurrentUser>) {
        self.composer_as = self.composer_as.take().filter(|selected| {
            user.as_ref().is_some_and(|current| {
                current
                    .organizations
                    .iter()
                    .any(|org| org.username.eq_ignore_ascii_case(&selected.username))
            })
        });
        self.user = user;
        if let Some(user) = &self.user {
            self.unread_notifications = user.unread_notifications();
            self.unread_remotes = user.unread_remotes();
        } else {
            self.unread_notifications = 0;
            self.unread_remotes = 0;
            self.counts_ready = false;
        }
        if let Some(sender) = APP_SENDER.get() {
            ui::fill_remote_settings(
                &self.remote_settings,
                self.user.as_ref().and_then(|user| user.remotes.as_ref()),
                sender.clone(),
            );
        }
        // Fill once here — avoid a second RefreshMeAvatars that rebuilt the
        // widget and dropped in-flight picture fetches on Quartz.
        self.refresh_me_avatars();
        self.sync_chat_dock();
    }

    fn refresh_me_avatars(&self) {
        let user = self.user.as_ref().map(|u| &u.user);
        let composer_face = self.composer_as.as_ref().or(user);
        self.composer_avatar.set_visible(user.is_some());
        ui::fill_author_avatar(&self.composer_avatar, composer_face, 40, false);
        self.composer_avatar
            .set_tooltip_text(Some(if self.can_post_as() {
                "Post as"
            } else {
                "Your profile"
            }));
        if let Some(sender) = APP_SENDER.get() {
            if let Some(current) = &self.user {
                ui::fill_author_menu(
                    &self.author_menu,
                    Some(&current.user),
                    &current.organizations,
                    self.composer_as.as_ref(),
                    sender.clone(),
                );
            }
        }
        ui::fill_avatar(&self.nav_avatar, user, 36, false);
        ui::fill_avatar(&self.header_avatar, user, 32, false);
        self.user_menu.set_user(user);
        self.header_user_menu.set_user(user);
        ui::set_staged_user(&self.staged_paint, &self.composer_rail_area, composer_face);
        self.sync_composer_ring();
    }

    fn can_post_as(&self) -> bool {
        self.user
            .as_ref()
            .is_some_and(|user| user.can_post_as_organization && !user.organizations.is_empty())
    }

    fn posting_as_handle(&self) -> Option<String> {
        self.composer_as
            .as_ref()
            .map(|org| org.username.clone())
            .filter(|handle| !handle.is_empty())
    }

    fn render_suggestions(&self, sender: &ComponentSender<App>) {
        ui::fill_suggestions(
            &self.suggested_list,
            &self.suggested,
            self.user.as_ref().map(|u| u.username()),
            &self.following,
            sender.input_sender().clone(),
        );
    }

    fn remember_followed(&mut self, username: &str) {
        let name = username.trim().to_ascii_lowercase();
        if !name.is_empty() {
            self.following.insert(name);
        }
    }

    fn sync_composer_ring(&self) {
        ui::set_staged_fill(
            &self.composer_rail_area,
            &self.staged_paint,
            self.composer_fill_amount(),
        );
    }

    fn sync_image_previews(&self, sender: &ComponentSender<Self>) {
        let sender = sender.input_sender().clone();
        ui::fill_attach_previews(
            &self.image_previews,
            &self.composer_images,
            sender.clone(),
            false,
        );
        ui::fill_attach_previews(
            &self.remote_image_previews,
            &self.remote_images,
            sender,
            true,
        );
    }

    fn composer_fill_amount(&self) -> f64 {
        (self.composer_len as f64 / RING_FULL_AT as f64).clamp(0.0, 1.0)
    }

    fn sync_composer_suggests(&mut self, view: &gtk::TextView, sender: &ComponentSender<App>) {
        if self.composer_preview && view == &self.composer {
            self.emoji.hide();
            self.mentions.hide();
            return;
        }
        let buf = view.buffer();
        if buf.has_selection() {
            self.emoji.hide();
            self.mentions.hide();
            return;
        }
        let text = buffer_text(view);
        let caret = (buf.cursor_position() as usize).min(text.chars().count());
        let (next, next_caret) = crate::emoji::emojify_input(&text, caret);
        if next != text {
            set_buffer_text_caret(view, &next, next_caret);
            return;
        }
        let before: String = next.chars().take(next_caret).collect();
        if let Some((_, query)) = crate::body::mention_trigger_at(&before) {
            self.emoji.hide();
            self.fetch_mentions(view, query, sender);
            return;
        }
        self.mentions.hide();
        self.mention_gen = self.mention_gen.wrapping_add(1);
        match crate::emoji::trigger_at(&before) {
            Some((_, query)) => {
                let items = crate::emoji::search(&query, 8);
                self.emoji.show(view, &query, &items);
            }
            None => self.emoji.hide(),
        }
    }

    fn fetch_mentions(
        &mut self,
        view: &gtk::TextView,
        query: String,
        sender: &ComponentSender<App>,
    ) {
        let Some(client) = self.client.clone() else {
            self.mentions.hide();
            return;
        };
        self.mentions.prepare(view);
        if let Some(id) = self.mention_timeout.borrow_mut().take() {
            ui::try_remove_source(id);
        }
        self.mention_gen = self.mention_gen.wrapping_add(1);
        let gen = self.mention_gen;
        let (reply, quote, org) = self.mention_context();
        let sender2 = sender.clone();
        let slot = self.mention_timeout.clone();
        let id = glib::timeout_add_local_once(Duration::from_millis(140), move || {
            slot.borrow_mut().take();
            relm4::spawn(async move {
                sender2.input(AppMsg::MentionsLoaded(
                    gen,
                    client
                        .mention_suggestions(
                            &query,
                            reply.as_deref(),
                            quote.as_deref(),
                            org.as_deref(),
                        )
                        .await,
                ));
            });
        });
        *self.mention_timeout.borrow_mut() = Some(id);
    }

    fn mention_context(&self) -> (Option<String>, Option<String>, Option<String>) {
        if self.visible == "remotes" {
            let org = match &self.thread_ref {
                Some(ThreadRef::Org { id, .. }) => Some(id.clone()),
                _ => None,
            };
            return (None, None, org);
        }
        let reply = if self.visible == "commit" {
            self.open_commit.as_ref().map(|commit| commit.id.clone())
        } else {
            self.replying_to.clone()
        };
        (reply, None, self.posting_as_handle())
    }

    fn mention_query(&self, view: &gtk::TextView) -> Option<String> {
        let buf = view.buffer();
        let text = buffer_text(view);
        let caret = (buf.cursor_position() as usize).min(text.chars().count());
        let before: String = text.chars().take(caret).collect();
        crate::body::mention_trigger_at(&before).map(|(_, query)| query)
    }

    fn mention_view(&self) -> Option<gtk::TextView> {
        self.mentions.view()
    }

    fn pick_emoji_view(&self) -> gtk::TextView {
        if self.visible == "remotes" {
            self.remote_composer.clone()
        } else if self.visible == "commit" {
            self.reply_composer.clone()
        } else {
            self.composer.clone()
        }
    }

    fn insert_emoji(&mut self, glyph: &str) {
        let view = self.emoji.view().unwrap_or_else(|| self.pick_emoji_view());
        let buf = view.buffer();
        let text = buffer_text(&view);
        let caret = (buf.cursor_position() as usize).min(text.chars().count());
        let before: String = text.chars().take(caret).collect();
        let start = if buf.has_selection() {
            let (sel_start, _) = buf.selection_bounds().unwrap_or((
                buf.iter_at_offset(caret as i32),
                buf.iter_at_offset(caret as i32),
            ));
            sel_start.offset()
        } else {
            crate::emoji::trigger_at(&before)
                .map(|(start, _)| start as i32)
                .unwrap_or(caret as i32)
        };
        let mut start_iter = buf.iter_at_offset(start);
        let mut end_iter = buf.iter_at_offset(caret as i32);
        buf.begin_user_action();
        buf.delete(&mut start_iter, &mut end_iter);
        buf.insert(&mut start_iter, glyph);
        buf.end_user_action();
        self.emoji.hide();
    }

    fn insert_mention(&mut self, handle: &str) {
        let view = self
            .mentions
            .view()
            .unwrap_or_else(|| self.pick_emoji_view());
        let buf = view.buffer();
        let text = buffer_text(&view);
        let caret = (buf.cursor_position() as usize).min(text.chars().count());
        let before: String = text.chars().take(caret).collect();
        let start = crate::body::mention_trigger_at(&before)
            .map(|(start, _)| start as i32)
            .unwrap_or(caret as i32);
        let mut start_iter = buf.iter_at_offset(start);
        let mut end_iter = buf.iter_at_offset(caret as i32);
        buf.begin_user_action();
        buf.delete(&mut start_iter, &mut end_iter);
        buf.insert(&mut start_iter, &format!("@{handle} "));
        buf.end_user_action();
        self.mentions.hide();
        self.mention_gen = self.mention_gen.wrapping_add(1);
    }

    fn apply_me(&mut self, user: CurrentUser, sender: &ComponentSender<App>) {
        let live = user.user.live.clone();
        let username = user.username().to_owned();
        self.set_user(Some(user));
        if let Some(profile) = &mut self.profile {
            if profile.username() == username {
                profile.user.live = live;
            }
        }
        if self.visible == "profile" {
            self.render_profile(sender);
        }
        if let Some(client) = self.client.clone() {
            let topics_client = client.clone();
            let sender2 = sender.clone();
            relm4::spawn(async move {
                sender2.input(AppMsg::LiveFollowingLoaded(client.live_following().await));
            });
            let sender2 = sender.clone();
            relm4::spawn(async move {
                sender2.input(AppMsg::HotTopicsLoaded(topics_client.hot_topics().await));
            });
        }
    }

    fn apply_poll(&mut self, id: &str, poll: devnull_api::Poll) {
        let mut apply = |commit: &mut Commit| {
            if commit.id == id {
                commit.poll = Some(poll.clone());
            }
            if let Some(quoted) = commit.quoted_commit.as_mut() {
                if quoted.id == id {
                    quoted.poll = Some(poll.clone());
                }
            }
        };
        if let Some(commit) = &mut self.open_commit {
            apply(commit);
        }
        self.log_commits.iter_mut().for_each(&mut apply);
        self.profile_commits.iter_mut().for_each(&mut apply);
        self.thread_replies.iter_mut().for_each(&mut apply);
        self.ancestors.iter_mut().for_each(&mut apply);
        self.related_commits.iter_mut().for_each(&mut apply);
        for replies in self.nested_replies.values_mut() {
            replies.iter_mut().for_each(&mut apply);
        }
    }

    fn load_connections(&mut self, more: bool, sender: &ComponentSender<App>) {
        let Some(client) = self.client.clone().or_else(|| Client::unsigned().ok()) else {
            return;
        };
        if self.connections_loading {
            return;
        }
        self.connections_loading = true;
        let handle = self.connections_handle.clone();
        let kind = self.connections_kind;
        let commit_id = self.connections_commit.clone();
        let people_cursor = if more {
            self.connections_cursor.clone()
        } else {
            None
        };
        let orgs_cursor = if more {
            self.connections_orgs_cursor.clone()
        } else {
            None
        };
        let sender2 = sender.clone();
        relm4::spawn(async move {
            let users = match kind {
                ConnectionsKind::Following => {
                    client
                        .following_page(&handle, people_cursor.as_deref())
                        .await
                }
                ConnectionsKind::Followers => {
                    client.followers(&handle, people_cursor.as_deref()).await
                }
                ConnectionsKind::Stargazers => match commit_id.as_deref() {
                    Some(id) => client.commit_stars(id, people_cursor.as_deref()).await,
                    None => Err(ApiError::decode()),
                },
            };
            let organizations =
                if kind == ConnectionsKind::Following && !(more && orgs_cursor.is_none()) {
                    client
                        .following_organizations(&handle, orgs_cursor.as_deref())
                        .await
                } else {
                    Ok(Page {
                        data: Vec::new(),
                        links: None,
                        meta: None,
                    })
                };
            sender2.input(AppMsg::ConnectionsLoaded {
                handle: if kind == ConnectionsKind::Stargazers {
                    commit_id.unwrap_or(handle)
                } else {
                    handle
                },
                kind,
                more,
                users,
                organizations,
            });
        });
    }

    fn render_connections(&self, sender: &ComponentSender<App>) {
        ui::fill_connections(
            &self.connections_list,
            self.connections_kind,
            &self.connections_users,
            &self.connections_orgs,
            self.connections_orgs_expanded,
            self.connections_orgs_cursor.is_some(),
            self.connections_star_count,
            self.user.as_ref().map(|user| user.username()),
            self.user.as_ref().is_some_and(|user| {
                user.username()
                    .eq_ignore_ascii_case(&self.connections_handle)
            }),
            sender.input_sender().clone(),
        );
    }

    fn render_thread(&self, sender: &ComponentSender<App>) {
        self.user_card.dismiss();
        ui::unparent(&self.inline_reply_box);
        ui::clear_list(&self.reply_list);
        let Some(commit) = &self.open_commit else {
            return;
        };
        let ctx = ui::FeedCtx {
            hover: None,
            who_to_follow_after: None,
            ..self.feed_ctx(sender)
        };
        let threads = self.thread_state();
        ui::append_thread(
            &self.reply_list,
            &self.ancestors,
            commit,
            &self.thread_replies,
            &threads,
            &ctx,
        );
        ui::append_related(&self.reply_list, &self.related_commits, &ctx);
    }

    fn begin_busy(&mut self) {
        self.busy = self.busy.saturating_add(1);
    }

    fn end_busy(&mut self) {
        self.busy = self.busy.saturating_sub(1);
    }

    fn remotes_on(&self) -> bool {
        self.user.as_ref().is_some_and(CurrentUser::remotes_on)
    }

    fn visible_commit_list(&self) -> &gtk::ListBox {
        match self.visible {
            "commit" => &self.reply_list,
            "profile" => &self.profile_list,
            "search" => &self.search_list,
            _ => &self.log_list,
        }
    }

    fn commit_by_id(&self, id: &str) -> Option<&Commit> {
        self.open_commit
            .as_ref()
            .filter(|commit| commit.id == id)
            .or_else(|| self.log_commits.iter().find(|commit| commit.id == id))
            .or_else(|| self.profile_commits.iter().find(|commit| commit.id == id))
            .or_else(|| self.thread_replies.iter().find(|commit| commit.id == id))
            .or_else(|| self.ancestors.iter().find(|commit| commit.id == id))
            .or_else(|| self.related_commits.iter().find(|commit| commit.id == id))
            .or_else(|| {
                self.nested_replies
                    .values()
                    .flatten()
                    .find(|commit| commit.id == id)
            })
    }

    fn shortcut_star(&self, id: &str, sender: &ComponentSender<App>) {
        let Some(commit) = self.commit_by_id(id).cloned() else {
            return;
        };
        let me = self.user.as_ref().map(|user| user.user.id.as_str());
        let (starred, count) = self.stars.get(id).copied().unwrap_or((
            commit.starred.unwrap_or(false),
            commit.stars_count.unwrap_or(0),
        ));
        if commit.is_own(me) {
            if count > 0 {
                sender.input(AppMsg::OpenStargazers {
                    commit_id: id.to_owned(),
                    count,
                });
            }
            return;
        }
        let org_ids = self
            .user
            .as_ref()
            .map(CurrentUser::org_ids)
            .unwrap_or_default();
        if commit.is_owned_by(me, &org_ids)
            || commit
                .status
                .as_deref()
                .is_some_and(|status| status != "approved")
        {
            return;
        }
        let next = !starred;
        let next_count = if next {
            count + 1
        } else {
            count.saturating_sub(1)
        };
        ui::paint_focused_star(self.visible_commit_list(), next, next_count);
        sender.input(AppMsg::ToggleStar(id.to_owned(), next, next_count));
    }

    fn copy_permalink(&self, commit: &Commit) {
        let hash = ui::short_hash(commit);
        if ui::copy_text(&ui::commit_url(commit)) {
            self.toast(&format!("Copied link to {hash}"));
        } else {
            self.toast(&format!("fatal: could not copy link to {hash}"));
        }
    }

    fn set_pasteis_count(&mut self, id: &str, count: i64) {
        let mut apply = |commit: &mut Commit| {
            if commit.id == id {
                commit.pasteis_count = Some(count);
            }
        };
        if let Some(commit) = &mut self.open_commit {
            apply(commit);
        }
        self.log_commits.iter_mut().for_each(&mut apply);
        self.profile_commits.iter_mut().for_each(&mut apply);
        self.thread_replies.iter_mut().for_each(&mut apply);
        self.ancestors.iter_mut().for_each(&mut apply);
        for replies in self.nested_replies.values_mut() {
            replies.iter_mut().for_each(&mut apply);
        }
    }

    fn show_notif_badge(&self) -> bool {
        self.unread_notifications > 0 && self.sidebar() != Sidebar::Notifications
    }

    fn show_remotes_badge(&self) -> bool {
        self.unread_remotes > 0 && self.sidebar() != Sidebar::Remotes
    }

    fn apply_unread(&mut self, notifications: i64, remotes: i64, alert: bool) {
        if alert && self.counts_ready {
            let new_notifications = notifications > self.unread_notifications;
            let new_remotes = remotes > self.unread_remotes && self.remote_arrivals.is_empty();
            if new_notifications {
                ui::notify(
                    "activity",
                    "New activity on /dev/null",
                    "You have unread notifications.",
                );
            }
            if new_remotes {
                ui::notify("remote", "New remote", "You have a new message.");
            }
            if new_notifications || new_remotes {
                ui::play_notification_sound();
            }
        }
        self.unread_notifications = notifications;
        self.unread_remotes = remotes;
        if alert {
            self.counts_ready = true;
        }
        self.sync_chat_dock();
    }

    fn poll_unread(&self, sender: &ComponentSender<App>) {
        let Some(client) = self.client.clone() else {
            return;
        };
        let remotes = self.remotes_on();
        let live_client = client.clone();
        let topics_client = client.clone();
        let sender2 = sender.clone();
        relm4::spawn(async move {
            sender2.input(AppMsg::UnreadPolled(client.unread_counts(remotes).await));
        });
        let sender2 = sender.clone();
        relm4::spawn(async move {
            sender2.input(AppMsg::LiveFollowingLoaded(
                live_client.live_following().await,
            ));
        });
        let sender2 = sender.clone();
        relm4::spawn(async move {
            sender2.input(AppMsg::HotTopicsLoaded(topics_client.hot_topics().await));
        });
    }

    fn set_hot_topics(&mut self, topics: Vec<HotTopic>, sender: &ComponentSender<App>) {
        self.hot_topics = topics.into_iter().take(5).collect();
        ui::fill_hot_topics(
            &self.hot_topics_list,
            &self.hot_topics,
            sender.input_sender().clone(),
        );
    }

    fn set_live_following(&mut self, mut users: Vec<User>, sender: &ComponentSender<App>) {
        users.retain(User::is_live);
        users.truncate(3);
        if users.len() < 3 {
            let have: HashSet<_> = users.iter().map(|user| user.username.clone()).collect();
            for extra in self.harvest_live() {
                if have.contains(&extra.username) {
                    continue;
                }
                users.push(extra);
                if users.len() >= 3 {
                    break;
                }
            }
        }
        let known = self.following.len();
        for user in &users {
            self.remember_followed(&user.username);
        }
        self.live_following = users;
        ui::fill_live_following(
            &self.live_list,
            &self.live_following,
            sender.input_sender().clone(),
        );
        if self.following.len() != known {
            self.render_suggestions(sender);
        }
    }

    fn harvest_live(&self) -> Vec<User> {
        let mut seen = HashSet::new();
        let mut out = Vec::new();
        let authors = self
            .log_commits
            .iter()
            .filter_map(|commit| commit.author.as_ref());
        let suggested = self.suggested.iter();
        for author in authors.chain(suggested) {
            if author.is_live() && seen.insert(author.username.clone()) {
                out.push(author.clone());
            }
            if out.len() >= 3 {
                break;
            }
        }
        out
    }

    fn settings_tab_classes(&self, tab: SettingsTab) -> [&'static str; 3] {
        if self.settings_tab == tab {
            ["flat", "dn-tab", "active"]
        } else {
            ["flat", "dn-tab", "inactive"]
        }
    }

    fn load_safety(&mut self, sender: &ComponentSender<App>) {
        if let Some(client) = self.client.clone() {
            self.begin_busy();
            let sender2 = sender.clone();
            relm4::spawn(async move {
                sender2.input(AppMsg::SafetyLoaded(
                    client.mutes().await,
                    client.blocks().await,
                ));
            });
        }
    }

    fn commit_sha(&self) -> String {
        self.open_commit
            .as_ref()
            .map(ui::short_hash)
            .unwrap_or_default()
    }

    fn me_name(&self) -> String {
        self.user
            .as_ref()
            .map(|u| u.user.display_name().to_owned())
            .unwrap_or_default()
    }

    fn me_handle(&self) -> String {
        self.user
            .as_ref()
            .map(|u| format!("@{}", u.username()))
            .unwrap_or_default()
    }

    fn load_profile(&mut self, sender: &ComponentSender<App>, username: String, replies: bool) {
        self.reset_profile_list();
        ui::append_commit_skeletons(&self.profile_list, 3);
        let client = self.client.clone().or_else(|| Client::unsigned().ok());
        if let Some(client) = client {
            self.begin_busy();
            let sender2 = sender.clone();
            relm4::spawn(async move {
                match client.profile(&username).await {
                    Ok(profile) => {
                        let commits = client.user_commits(&username, replies, None).await;
                        sender2.input(AppMsg::ProfileLoaded(Ok(Box::new(profile)), commits));
                    }
                    Err(err) if err.status == 404 => {
                        let organization = client.organization(&username).await.map(Box::new);
                        let commits = client.organization_commits(&username, None).await;
                        sender2.input(AppMsg::OrganizationLoaded(organization, commits));
                    }
                    Err(err) => {
                        sender2.input(AppMsg::ProfileLoaded(Err(err), Err(ApiError::decode())));
                    }
                }
            });
        }
    }

    fn load_organization(&mut self, sender: &ComponentSender<App>, handle: String) {
        self.reset_profile_list();
        ui::append_commit_skeletons(&self.profile_list, 3);
        let mentions = self.profile_tab == ProfileFeed::Mentions;
        let client = self.client.clone().or_else(|| Client::unsigned().ok());
        if let Some(client) = client {
            self.begin_busy();
            let sender2 = sender.clone();
            relm4::spawn(async move {
                let organization = client.organization(&handle).await.map(Box::new);
                let commits = if mentions {
                    client.organization_mentions(&handle, None).await
                } else {
                    client.organization_commits(&handle, None).await
                };
                sender2.input(AppMsg::OrganizationLoaded(organization, commits));
            });
        }
    }

    fn load_readme(&mut self, sender: &ComponentSender<App>) {
        let Some(profile) = &self.profile else {
            return;
        };
        let handle = profile.username().to_owned();
        let github_login = profile.user.github_login().to_owned();
        if let Some((user, _)) = &self.profile_readme {
            if user == &handle {
                self.render_profile_readme(sender);
                return;
            }
        }
        self.reset_profile_list();
        self.profile_list.append(&ui::content_skeleton_row());
        let client = self.client.clone().or_else(|| Client::unsigned().ok());
        if let Some(client) = client {
            self.begin_busy();
            let sender2 = sender.clone();
            relm4::spawn(async move {
                sender2.input(AppMsg::ProfileReadmeLoaded(
                    handle.clone(),
                    client.profile_readme(&handle, &github_login).await,
                ));
            });
        }
    }

    fn render_profile_readme(&self, sender: &ComponentSender<App>) {
        self.reset_profile_list();
        let Some(profile) = &self.profile else {
            return;
        };
        let Some((_, result)) = &self.profile_readme else {
            self.profile_list.append(&ui::content_skeleton_row());
            return;
        };
        let own = self.user.as_ref().map(|u| u.username()) == Some(profile.username());
        self.profile_list.append(&ui::profile_readme_row(
            result,
            profile.username(),
            profile.user.github_login(),
            own,
            sender.input_sender().clone(),
        ));
    }

    fn render_settings(&self, sender: &ComponentSender<App>) {
        while let Some(child) = self.settings_box.first_child() {
            self.settings_box.remove(&child);
        }
        self.settings_box
            .append(&ui::settings_panel(ui::SettingsCtx {
                me: self.user.as_ref(),
                tab: self.settings_tab,
                theme: self.prefs.theme,
                show_who_to_follow: self.prefs.show_who_to_follow,
                highlight_on_hover: self.prefs.highlight_on_hover,
                muted: &self.muted,
                blocked: &self.blocked,
                draft_lane: self.draft_lane.clone(),
                device_name: auth::device_name(),
                fingerprint: self.client.as_ref().and_then(Client::fingerprint),
                automatic_translation: self.automatic_translation(),
                translation_language: self
                    .user_settings
                    .as_ref()
                    .and_then(|settings| settings.translation_language.clone()),
                translation_languages: self
                    .user_settings
                    .as_ref()
                    .map(|settings| settings.translation_languages.as_slice())
                    .unwrap_or(&[]),
                sender: sender.input_sender().clone(),
            }));
    }

    fn load_starred(&mut self, sender: &ComponentSender<App>, more: bool) {
        if !more {
            self.log_cursor = None;
        }
        let cursor = if more { self.log_cursor.clone() } else { None };
        if let Some(client) = self.client.clone() {
            self.begin_busy();
            if !more && self.log_commits.is_empty() {
                ui::append_commit_skeletons(&self.log_list, 3);
            }
            let sender2 = sender.clone();
            relm4::spawn(async move {
                sender2.input(AppMsg::LogLoaded(client.starred(cursor.as_deref()).await));
            });
        }
    }

    fn reset_profile_list(&self) {
        self.user_card.dismiss();
        ui::unparent(&self.inline_reply_box);
        ui::reset_profile_list(&self.profile_list, &self.profile_hero);
    }

    fn render_profile(&self, sender: &ComponentSender<App>) {
        while let Some(child) = self.profile_hero.first_child() {
            self.profile_hero.remove(&child);
        }
        while let Some(child) = self.profile_bar_avatar.first_child() {
            self.profile_bar_avatar.remove(&child);
        }
        if let Some(organization) = &self.organization {
            self.profile_bar_avatar
                .append(&ui::org_avatar_widget(organization, 24, true));
            ui::fill_organization_hero(
                &self.profile_hero,
                organization,
                self.user.is_some(),
                self.profile_tab,
                sender.input_sender().clone(),
            );
        } else if let Some(profile) = &self.profile {
            self.profile_bar_avatar
                .append(&ui::avatar_widget(Some(&profile.user), 24, true));
            ui::fill_profile_hero(
                &self.profile_hero,
                profile,
                self.user.as_ref(),
                self.follow_label(),
                self.profile_tab,
                sender.input_sender().clone(),
            );
        }
        ui::pin_profile_hero(&self.profile_list, &self.profile_hero);
    }

    fn can_commit(&self) -> bool {
        let n = buffer_text(&self.composer).trim().chars().count();
        (n >= MIN_BODY || !self.composer_images.is_empty()) && self.composer_len <= MAX_BODY
    }

    fn sign_in_hint(&self) -> &str {
        match self.phase {
            Phase::Loading => "Restoring this device…",
            Phase::WaitingForBrowser => {
                "Sign in on the web if asked, then Approve this device. If the browser offers to open /dev/null, allow it — this window comes back on its own."
            }
            _ => "Opens GitHub in your browser. This device gets its own key; revoke it later under Settings → Devices.",
        }
    }

    fn follow_label(&self) -> &str {
        if let Some(organization) = &self.organization {
            return if organization.is_following {
                "Following"
            } else {
                "Follow"
            };
        }
        let me = self.user.as_ref().map(|u| u.username());
        if self.profile.as_ref().map(|p| p.username()) == me {
            return "";
        }
        let Some(profile) = self.profile.as_ref() else {
            return "";
        };
        ui::preview_follow_label(profile.is_following, profile.follows_you)
    }

    fn toast(&self, text: &str) {
        self.show_toast(text, None);
    }

    fn toast_unless_limited(&self, err: &ApiError) {
        if !err.is_rate_limited() {
            self.toast(&err.message);
        }
    }

    fn toast_commit(&self, text: &str, id: String) {
        self.show_toast(text, Some(id));
    }

    fn show_toast(&self, text: &str, commit_id: Option<String>) {
        *self.toast_commit.borrow_mut() = commit_id.clone();
        self.aside_toast_label.set_label(text);
        if commit_id.is_some() {
            self.aside_toast.add_css_class("dn-toast-link");
            self.aside_toast.set_cursor_from_name(Some("pointer"));
        } else {
            self.aside_toast.remove_css_class("dn-toast-link");
            self.aside_toast.set_cursor_from_name(None);
        }
        self.aside_toast.add_css_class("visible");
        self.aside_toast.set_reveal_child(true);
        if let Some(id) = self.toast_hide.borrow_mut().take() {
            ui::try_remove_source(id);
        }
        let revealer = self.aside_toast.downgrade();
        let slot = self.toast_hide.clone();
        let commit_slot = self.toast_commit.clone();
        let id = glib::timeout_add_local_once(Duration::from_secs(4), move || {
            slot.borrow_mut().take();
            commit_slot.borrow_mut().take();
            if let Some(revealer) = revealer.upgrade() {
                revealer.remove_css_class("visible");
                revealer.remove_css_class("dn-toast-link");
                revealer.set_reveal_child(false);
            }
        });
        *self.toast_hide.borrow_mut() = Some(id);
    }

    fn handle_callback(&mut self, uri: &str, sender: &ComponentSender<App>) {
        if self.phase == Phase::SignedIn {
            return;
        }
        if self.pending.is_none() {
            if let Some((pkce, _)) = auth::load_pending() {
                self.pending = Some(pkce);
            }
        }
        let Some(extracted) = auth::extract_callback(uri) else {
            return;
        };
        let Ok(cb) = auth::parse_callback(&extracted) else {
            return;
        };
        let Some(pkce) = self.pending.clone() else {
            return;
        };
        if cb.state.as_ref().is_some_and(|state| state != &pkce.state) {
            return;
        }
        if cb.error.as_deref() == Some("access_denied") {
            self.pending = None;
            auth::clear_pending();
            self.phase = Phase::SignedOut;
            return;
        }
        let Some(code) = cb.code else {
            self.error = Some("The browser didn't approve this device.".into());
            self.pending = None;
            auth::clear_pending();
            self.phase = Phase::SignedOut;
            return;
        };
        self.pending = None;
        self.phase = Phase::Loading;
        let key = auth::take_pending().map(|(_, key)| key);
        let sender2 = sender.clone();
        relm4::spawn(async move {
            sender2.input(AppMsg::Redeemed(redeem(code, pkce.verifier, key).await));
        });
    }

    fn capture_log_visit(&mut self) {
        let Some(kind) = self
            .nav
            .log()
            .filter(|kind| !matches!(*kind, LogKind::Trending))
        else {
            self.log_visit_at = None;
            return;
        };
        self.log_visit_at = self.user.as_ref().and_then(|user| {
            self.prefs
                .last_visit(&user.user.id, kind.as_str())
                .map(str::to_owned)
        });
    }

    fn first_seen_id(&self, commits: &[Commit]) -> Option<String> {
        if !matches!(self.nav, Nav::Following | Nav::Latest) {
            return None;
        }
        ui::last_visit_id(
            commits,
            self.log_visit_at.as_deref(),
            self.user.as_ref().map(|user| user.user.id.as_str()),
        )
    }

    fn tracks_new_posts(&self) -> bool {
        self.visible == "log" && matches!(self.nav, Nav::Following | Nav::Latest)
    }

    fn poll_new_posts(&self, sender: &ComponentSender<App>) {
        let Some(kind) = self.nav.log() else {
            return;
        };
        let Some(client) = self.client.clone() else {
            return;
        };
        let sender2 = sender.clone();
        relm4::spawn(async move {
            sender2.input(AppMsg::LogPolled(client.log(kind, None).await));
        });
    }

    fn paint_new_posts(&self) {
        ui::paint_new_posts(
            &self.new_posts,
            self.new_posts_count,
            &self.new_authors,
            self.log_away,
        );
    }

    fn clear_new_posts(&mut self) {
        self.new_posts_count = 0;
        self.new_authors.clear();
        self.paint_new_posts();
    }

    fn load_log(&mut self, sender: &ComponentSender<App>, more: bool) {
        let Some(kind) = self.nav.log() else {
            return;
        };
        if !more {
            self.log_cursor = None;
        }
        let cursor = if more { self.log_cursor.clone() } else { None };
        if let Some(client) = self.client.clone() {
            self.begin_busy();
            if !more && self.log_commits.is_empty() {
                ui::append_commit_skeletons(&self.log_list, 3);
            }
            let sender2 = sender.clone();
            relm4::spawn(async move {
                sender2.input(AppMsg::LogLoaded(client.log(kind, cursor.as_deref()).await));
            });
        }
    }

    fn render_notifications(&self, sender: &ComponentSender<App>) {
        ui::clear_list(&self.notif_list);
        if self.notifications.is_empty() {
            let fatal = NotificationFilter::empty_fatal_for(&self.notif_types);
            let show_all = self.notif_types.len() != NotificationFilter::CATEGORIES.len();
            self.notif_list.append(&ui::notification_empty(
                &fatal,
                NotificationFilter::empty_detail_for(&self.notif_types),
                show_all,
                sender.input_sender().clone(),
            ));
            return;
        }
        ui::append_notifications(
            &self.notif_list,
            &self.notifications,
            &self.notif_groups,
            sender.input_sender().clone(),
        );
    }

    fn fetch_notification_group(&self, id: String, more: bool, sender: &ComponentSender<App>) {
        let Some(client) = self.client.clone() else {
            return;
        };
        let types = self.notif_types.clone();
        let cursor = if more {
            self.notif_groups
                .get(&id)
                .and_then(|group| group.cursor.clone())
        } else {
            None
        };
        let sender2 = sender.clone();
        relm4::spawn(async move {
            sender2.input(AppMsg::NotificationGroupLoaded(
                id.clone(),
                more,
                client
                    .notification_group(&id, &types, cursor.as_deref())
                    .await,
            ));
        });
    }

    fn load_notifications(&mut self, sender: &ComponentSender<App>) {
        if let Some(client) = self.client.clone() {
            self.begin_busy();
            if self.notif_list.row_at_index(0).is_none() {
                ui::append_notification_skeletons(&self.notif_list, 2);
            }
            let types = self.notif_types.clone();
            let sender2 = sender.clone();
            relm4::spawn(async move {
                sender2.input(AppMsg::NotificationsLoaded(
                    client.notifications(&types, None).await,
                ));
            });
        }
    }

    fn load_remotes(&self, sender: &ComponentSender<App>) {
        if let Some(client) = self.client.clone() {
            if self.inbox_list.row_at_index(0).is_none() {
                ui::append_inbox_skeletons(&self.inbox_list, 4);
            }
            let sender2 = sender.clone();
            let tab = self.inbox_tab;
            relm4::spawn(async move {
                sender2.input(AppMsg::RemotesLoaded(client.remotes(tab, None, None).await));
            });
        }
    }

    fn search_inbox(&mut self, sender: &ComponentSender<App>) {
        let Some(client) = self.client.clone() else {
            return;
        };
        let query = self.inbox_query.trim().trim_start_matches('@').to_string();
        if query.is_empty() {
            return;
        }
        self.inbox_search_gen = self.inbox_search_gen.saturating_add(1);
        let gen = self.inbox_search_gen;
        let sender2 = sender.clone();
        let tab = self.inbox_tab;
        relm4::spawn(async move {
            let page = client.remotes(tab, None, Some(&query)).await;
            let people = match client.remote_candidates(&query, "remote").await {
                Ok(candidates) => candidates
                    .into_iter()
                    .filter(RemoteCandidate::pickable)
                    .map(|candidate| candidate.user)
                    .collect(),
                Err(_) => Vec::new(),
            };
            sender2.input(AppMsg::InboxSearchLoaded(gen, query, page, people));
        });
    }

    fn search_people(&mut self, sender: &ComponentSender<App>, query: String) {
        let Some(client) = self.client.clone() else {
            return;
        };
        self.people_gen = self.people_gen.saturating_add(1);
        let gen = self.people_gen;
        let for_kind = self.people_picker.kind_for().to_owned();
        let sender2 = sender.clone();
        relm4::spawn(async move {
            let mut people = client
                .remote_candidates(&query, &for_kind)
                .await
                .unwrap_or_default();
            if people.is_empty() {
                if let Ok(results) = client.search(&query).await {
                    people = results
                        .users
                        .into_iter()
                        .map(|user| RemoteCandidate { user, reason: None })
                        .collect();
                }
            }
            sender2.input(AppMsg::PeopleLoaded(gen, Ok(people)));
        });
    }

    fn save_remote_prefs(
        &mut self,
        from: String,
        pull_receipts: bool,
        sender: &ComponentSender<App>,
    ) {
        let Some(client) = self.client.clone() else {
            return;
        };
        self.begin_busy();
        let sender2 = sender.clone();
        relm4::spawn(async move {
            sender2.input(AppMsg::RemotePrefsSaved(
                client.set_remote_preferences(&from, pull_receipts).await,
            ));
        });
    }

    fn conversation_for(&self, reference: &ThreadRef) -> Option<&ConversationSummary> {
        self.inbox
            .iter()
            .chain(self.inbox_hits.iter().flatten())
            .find(|item| ThreadRef::from_summary(item) == *reference)
    }

    fn conversation_for_mut(&mut self, reference: &ThreadRef) -> Option<&mut ConversationSummary> {
        let matches = |item: &ConversationSummary| ThreadRef::from_summary(item) == *reference;
        if let Some(index) = self.inbox.iter().position(matches) {
            return self.inbox.get_mut(index);
        }
        self.inbox_hits
            .as_mut()
            .and_then(|hits| hits.iter_mut().find(|item| matches(item)))
    }

    fn apply_local_mute(&mut self, reference: &ThreadRef, muted: bool) {
        let until = muted.then(|| chrono::Utc::now() + chrono::Duration::days(3650));
        if self.thread_ref.as_ref() == Some(reference) {
            if let Some(thread) = self.open_thread.as_mut() {
                thread.muted_until_at = until;
            }
        }
        if let Some(item) = self.conversation_for_mut(reference) {
            item.muted_until_at = until;
        }
    }

    fn alert_incoming_remotes(&mut self, conversations: &[ConversationSummary]) {
        let me_id = self.user.as_ref().map(|user| user.user.id.clone());
        let me = self.user.as_ref().map(|user| user.user.clone());
        let mut chime = false;
        let mut notify = false;
        let mut title = String::from("New remote");
        let mut body = String::from("You have a new message.");
        for item in conversations {
            let reference = ThreadRef::from_summary(item);
            let key = reference.key();
            let incoming = item.arrival_id(me_id.as_deref());
            let previous = self.remote_arrivals.get(&key).copied().unwrap_or(0);
            if incoming > 0 {
                self.remote_arrivals.insert(key, incoming.max(previous));
            }
            let reading = self.thread_ref.as_ref() == Some(&reference);
            if !item.should_alert_arrival(previous, reading, me_id.as_deref()) {
                continue;
            }
            chime = true;
            if reading {
                continue;
            }
            notify = true;
            title = item.title_for(me.as_ref());
            let preview = item.preview_for(me_id.as_deref());
            if !preview.is_empty() {
                body = preview;
            }
        }
        if notify {
            ui::notify("remote", &title, &body);
        }
        if chime {
            ui::play_notification_sound();
        }
    }

    fn header_stamp(&self, thread: Option<&ChatThread>) -> String {
        let Some(reference) = self.thread_ref.as_ref() else {
            return String::new();
        };
        let members = thread
            .map(|thread| {
                thread
                    .members
                    .iter()
                    .map(|member| member.user.id.as_str())
                    .collect::<Vec<_>>()
                    .join(",")
            })
            .unwrap_or_default();
        let pins = thread
            .map(|thread| {
                thread
                    .pinned
                    .iter()
                    .map(|message| message.id.to_string())
                    .collect::<Vec<_>>()
                    .join(",")
            })
            .unwrap_or_default();
        format!(
            "{}:{}:{}:{}:{}:{}:{}",
            reference.key(),
            thread.map(|thread| thread.is_archived).unwrap_or(false),
            self.is_remote_pinned(reference),
            self.is_remote_muted(reference),
            pins,
            members,
            self.aside_compact
        )
    }

    fn fill_open_thread_header(&self, thread: Option<&ChatThread>, sender: relm4::Sender<AppMsg>) {
        let Some(reference) = self.thread_ref.as_ref() else {
            return;
        };
        let item = self.conversation_for(reference);
        let archived = thread
            .map(|thread| thread.is_archived)
            .or_else(|| item.map(|item| item.is_archived))
            .unwrap_or(false);
        let pinned = self.is_remote_pinned(reference);
        let muted = self.is_remote_muted(reference);
        let pinned_messages = thread
            .or(self.open_thread.as_ref())
            .map(|thread| thread.pinned.as_slice())
            .unwrap_or(&[]);
        ui::fill_thread_header(
            &self.thread_header,
            ui::ThreadHeaderState {
                reference,
                item,
                thread,
                me: self.user.as_ref().map(|user| &user.user),
                archived,
                pinned,
                muted,
                pinned_messages,
                can_unpin: self.can_pin_org_messages(thread.or(self.open_thread.as_ref())),
                show_back: self.aside_compact,
            },
            sender,
        );
    }

    fn render_inbox(&self, sender: &ComponentSender<App>) {
        let me = self.user.as_ref().map(|user| &user.user);
        let active = self.thread_ref.as_ref();
        let input = sender.input_sender().clone();
        let conversations = self.visible_inbox();
        ui::clear_list(&self.inbox_list);
        ui::clear_list(&self.chat_dock.list);
        if conversations.is_empty() && self.inbox_people.is_empty() {
            let (title, body) = if !self.inbox_query.trim().is_empty() {
                ("fatal: nothing matches", "Try another name or @handle.")
            } else {
                match self.inbox_tab {
                    InboxTab::Fetched => ("no requests fetched", "Incoming remotes land here."),
                    InboxTab::Stashed => (
                        "nothing stashed",
                        "Stashed remotes wait here until someone writes.",
                    ),
                    InboxTab::Open => (
                        "fatal: no remotes",
                        "Direct messages land here. Fetch someone from their profile.",
                    ),
                }
            };
            self.inbox_list.append(&ui::empty_row(title, body));
            self.chat_dock.list.append(&ui::empty_row(
                "no conversations yet",
                "Start a remote from the compose button.",
            ));
        } else {
            for item in &conversations {
                let pinned = self.is_remote_pinned(&ThreadRef::from_summary(item));
                self.inbox_list.append(&ui::inbox_row(
                    item,
                    me,
                    active,
                    false,
                    pinned,
                    input.clone(),
                ));
                self.chat_dock.list.append(&ui::inbox_row(
                    item,
                    me,
                    active,
                    true,
                    pinned,
                    input.clone(),
                ));
            }
            for user in &self.inbox_people {
                self.inbox_list
                    .append(&ui::inbox_people_row(user, input.clone()));
            }
        }
        self.sync_chat_dock();
    }

    fn visible_inbox(&self) -> Vec<&ConversationSummary> {
        let me = self.user.as_ref().map(|user| &user.user);
        if let Some(hits) = &self.inbox_hits {
            let mut items: Vec<&ConversationSummary> = hits.iter().collect();
            items.sort_by(|left, right| {
                self.is_remote_pinned(&ThreadRef::from_summary(right))
                    .cmp(&self.is_remote_pinned(&ThreadRef::from_summary(left)))
            });
            return items;
        }
        let mut items: Vec<&ConversationSummary> = self
            .inbox
            .iter()
            .filter(|item| item.matches_query(&self.inbox_query, me))
            .collect();
        items.sort_by(|left, right| {
            self.is_remote_pinned(&ThreadRef::from_summary(right))
                .cmp(&self.is_remote_pinned(&ThreadRef::from_summary(left)))
        });
        items
    }

    fn sync_chat_dock(&self) {
        let docked = self.prefs.chat_docked
            && self.remotes_on()
            && !self.shell_compact
            && self.phase == Phase::SignedIn;
        self.chat_dock
            .sync(docked, self.chat_dock_expanded, self.unread_remotes);
    }

    fn can_push_remote(&self) -> bool {
        self.remote_draft_len > 0 || !self.remote_images.is_empty()
    }

    fn remotes_inbox_visible(&self) -> bool {
        remotes_inbox_visible(self.aside_compact, self.thread_ref.is_some())
    }

    fn remotes_thread_visible(&self) -> bool {
        remotes_thread_visible(self.aside_compact, self.thread_ref.is_some())
    }

    fn remote_placeholder(&self) -> String {
        match &self.thread_ref {
            Some(ThreadRef::Direct { username }) => format!("Message @{username}"),
            Some(ThreadRef::Org { name, .. }) => {
                let label = name
                    .as_deref()
                    .map(str::trim)
                    .filter(|name| !name.is_empty())
                    .or_else(|| {
                        self.open_thread
                            .as_ref()
                            .and_then(|thread| thread.org.as_ref())
                            .and_then(|org| org.name.as_deref())
                            .map(str::trim)
                            .filter(|name| !name.is_empty())
                    })
                    .unwrap_or("the org");
                format!("Message {label}")
            }
            None => String::new(),
        }
    }

    fn remote_reply_author(&self) -> String {
        self.remote_reply
            .as_ref()
            .and_then(|message| message.user.as_ref())
            .map(User::display_name)
            .unwrap_or("someone")
            .to_owned()
    }

    fn remote_reply_snippet(&self) -> String {
        let Some(message) = &self.remote_reply else {
            return String::new();
        };
        message
            .body
            .as_deref()
            .map(str::trim)
            .filter(|text| !text.is_empty())
            .map(str::to_owned)
            .unwrap_or_else(|| {
                if message.images.len() > 1 {
                    format!("{} images", message.images.len())
                } else if !message.images.is_empty() {
                    "Image".into()
                } else {
                    String::new()
                }
            })
    }

    fn ping_insert_text(&self) -> String {
        let me = self.user.as_ref().map(|user| user.user.id.as_str());
        if let Some(ThreadRef::Org { .. }) = &self.thread_ref {
            if let Some(thread) = &self.open_thread {
                let mentions: Vec<String> = thread
                    .members
                    .iter()
                    .filter(|member| me != Some(member.user.id.as_str()))
                    .map(|member| format!("@{}", member.user.username))
                    .collect();
                if !mentions.is_empty() {
                    return format!("{} ", mentions.join(" "));
                }
            }
        }
        if let Some(ThreadRef::Direct { username }) = &self.thread_ref {
            return format!("@{username} ");
        }
        "@".into()
    }

    fn thread_chrome(&self, thread: &ChatThread) -> ui::ThreadChrome {
        ui::ThreadChrome {
            pinned_ids: thread.pinned.iter().map(|message| message.id).collect(),
            can_pin: self.can_pin_org_messages(Some(thread)),
            people: thread_people(thread, self.user.as_ref().map(|user| &user.user)),
            bubbles: self.bubble_width.clone(),
        }
    }

    fn can_pin_org_messages(&self, thread: Option<&ChatThread>) -> bool {
        matches!(self.thread_ref, Some(ThreadRef::Org { .. }))
            && thread.is_some_and(|thread| {
                thread.is_maintainer_for(self.user.as_ref().map(|user| user.user.id.as_str()))
            })
    }

    fn pin_thread_message(&mut self, id: i64, sender: &ComponentSender<App>) {
        if !matches!(self.thread_ref, Some(ThreadRef::Org { .. })) {
            return;
        }
        let Some(thread) = self.open_thread.as_ref() else {
            return;
        };
        if !self.can_pin_org_messages(Some(thread)) {
            return;
        }
        let is_pinned = thread.pinned.iter().any(|message| message.id == id);
        if is_pinned {
            self.request_message_pin(id, false, sender);
            return;
        }
        if self.prefs.pin_warning_dismissed {
            self.request_message_pin(id, true, sender);
            return;
        }
        ui::confirm_pin_org_message(id, sender.input_sender().clone());
    }

    fn request_message_pin(&mut self, id: i64, pinned: bool, sender: &ComponentSender<App>) {
        let Some(reference) = self.thread_ref.clone() else {
            return;
        };
        let Some(client) = self.client.clone() else {
            return;
        };
        let key = reference.key();
        let sender2 = sender.clone();
        relm4::spawn(async move {
            sender2.input(AppMsg::ThreadPinsLoaded(
                key,
                pinned,
                client.set_message_pinned(id, pinned).await,
            ));
        });
    }

    fn toggle_message_reaction(&mut self, id: i64, emoji: &str, sender: &ComponentSender<App>) {
        let Some(reference) = self.thread_ref.clone() else {
            return;
        };
        let Some(me) = self.user.as_ref().map(|user| user.user.id.clone()) else {
            return;
        };
        let Some(client) = self.client.clone() else {
            return;
        };
        let Some(thread) = self.open_thread.as_mut() else {
            return;
        };
        let mut adding = true;
        let mut next = None;
        for message in thread.messages.iter_mut().chain(thread.pinned.iter_mut()) {
            if message.id != id {
                continue;
            }
            if let Some(index) = message
                .reactions
                .iter()
                .position(|reaction| reaction.user_id == me && reaction.emoji == emoji)
            {
                message.reactions.remove(index);
                adding = false;
            } else {
                message.reactions.push(MessageReaction {
                    user_id: me.clone(),
                    emoji: emoji.to_owned(),
                });
                adding = true;
            }
            next = Some(message.reactions.clone());
        }
        let Some(reactions) = next else {
            return;
        };
        let people = thread_people(thread, self.user.as_ref().map(|user| &user.user));
        ui::refresh_message_reactions(
            &self.thread_list,
            id,
            &reactions,
            Some(me.as_str()),
            &people,
            sender.input_sender().clone(),
        );
        let key = reference.key();
        let emoji = emoji.to_owned();
        let sender2 = sender.clone();
        relm4::spawn(async move {
            sender2.input(AppMsg::MessageReactionsLoaded(
                key,
                id,
                client.set_message_reaction(id, &emoji, adding).await,
            ));
        });
    }

    fn set_message_reactions(
        &mut self,
        id: i64,
        reactions: Vec<MessageReaction>,
        sender: &ComponentSender<App>,
    ) {
        let Some(thread) = self.open_thread.as_mut() else {
            return;
        };
        for message in thread.messages.iter_mut().chain(thread.pinned.iter_mut()) {
            if message.id == id {
                message.reactions = reactions.clone();
            }
        }
        let me = self.user.as_ref().map(|user| user.user.id.as_str());
        let people = thread_people(thread, self.user.as_ref().map(|user| &user.user));
        ui::refresh_message_reactions(
            &self.thread_list,
            id,
            &reactions,
            me,
            &people,
            sender.input_sender().clone(),
        );
    }

    fn is_remote_pinned(&self, reference: &ThreadRef) -> bool {
        self.prefs.is_remote_pinned(&reference.key())
            || self
                .conversation_for(reference)
                .is_some_and(ConversationSummary::is_pinned)
    }

    fn is_remote_muted(&self, reference: &ThreadRef) -> bool {
        if self.thread_ref.as_ref() == Some(reference)
            && self.open_thread.as_ref().is_some_and(ChatThread::is_muted)
        {
            return true;
        }
        if self
            .conversation_for(reference)
            .is_some_and(ConversationSummary::is_muted)
        {
            return true;
        }
        match reference {
            ThreadRef::Direct { username } => self
                .muted
                .iter()
                .any(|user| user.username.eq_ignore_ascii_case(username)),
            ThreadRef::Org { .. } => false,
        }
    }

    fn close_open_thread(&mut self, sender: &ComponentSender<App>) {
        self.thread_gen = self.thread_gen.wrapping_add(1);
        self.thread_ref = None;
        self.open_thread = None;
        self.remote_reply = None;
        self.last_thread_ping = None;
        self.thread_typing.clear();
        self.thread_header_stamp.clear();
        ui::clear_list(&self.thread_list);
        ui::clear_box(&self.thread_header);
        self.render_inbox(sender);
    }

    fn fetch_open_thread(&mut self, sender: &ComponentSender<App>) {
        let Some(reference) = self.thread_ref.clone() else {
            return;
        };
        let Some(client) = self.client.clone() else {
            return;
        };
        self.thread_gen = self.thread_gen.wrapping_add(1);
        let gen = self.thread_gen;
        let sender2 = sender.clone();
        relm4::spawn(async move {
            sender2.input(AppMsg::ThreadLoaded(gen, client.thread(&reference).await));
        });
    }

    fn apply_open_thread(&mut self, thread: ChatThread, sender: &ComponentSender<App>) {
        let me = self.user.as_ref().map(|user| user.user.id.clone());
        let is_org = matches!(self.thread_ref, Some(ThreadRef::Org { .. }));
        let pulls = ui::ThreadPulls::from_thread(&thread, me.as_deref());
        let chrome = self.thread_chrome(&thread);
        let previous = self
            .open_thread
            .as_ref()
            .map(|thread| thread.messages.clone());
        let opened = previous.is_none();
        let patched = previous.as_ref().is_some_and(|old| {
            ui::sync_thread_messages(
                &self.thread_list,
                old,
                &thread.messages,
                me.as_deref(),
                is_org,
                &pulls,
                &chrome,
                sender.input_sender().clone(),
            )
        });
        if !patched {
            ui::clear_list(&self.thread_list);
            if thread.messages.is_empty() {
                if let Some(reference) = &self.thread_ref {
                    self.thread_list
                        .append(&ui::thread_empty_history(reference));
                }
            } else {
                ui::append_thread_messages(
                    &self.thread_list,
                    &thread.messages,
                    me.as_deref(),
                    is_org,
                    &pulls,
                    &chrome,
                    sender.input_sender().clone(),
                );
            }
        }
        self.thread_typing = typing_copy(&thread, self.thread_ref.as_ref());
        ui::refresh_thread_pins(
            &self.thread_list,
            &thread.pinned.iter().map(|message| message.id).collect(),
            self.can_pin_org_messages(Some(&thread)),
        );
        let stamp = self.header_stamp(Some(&thread));
        if stamp != self.thread_header_stamp {
            self.fill_open_thread_header(Some(&thread), sender.input_sender().clone());
            self.thread_header_stamp = stamp;
        }
        if let (Some(last), Some(client), Some(reference)) = (
            thread.messages.last(),
            self.client.clone(),
            self.thread_ref.clone(),
        ) {
            let through = last.id;
            relm4::spawn(async move {
                let _ = client.mark_read(&reference, through).await;
            });
        }
        if let Some(id) = latest_ping_id(&thread.messages) {
            if self.last_thread_ping.is_some_and(|previous| id > previous) && !thread.is_muted() {
                ui::play_notification_sound();
            }
            self.last_thread_ping = Some(self.last_thread_ping.unwrap_or(0).max(id));
        }
        self.open_thread = Some(thread);
        if opened {
            self.thread_follow.set(true);
        }
        if self.thread_follow.get() {
            ui::scroll_to_end(&self.thread_scroll);
            let scroll = self.thread_scroll.clone();
            let follow = self.thread_follow.clone();
            glib::idle_add_local_once(move || {
                if follow.get() {
                    ui::scroll_to_end(&scroll);
                }
            });
        }
    }

    fn toggle_pin_remote(&mut self, sender: &ComponentSender<App>) {
        let Some(reference) = self.thread_ref.clone() else {
            return;
        };
        let key = reference.key();
        let latest = self.open_thread.as_ref().and_then(|thread| {
            thread
                .messages
                .iter()
                .rev()
                .find(|message| {
                    message
                        .event
                        .as_deref()
                        .is_none_or(|event| event.is_empty())
                })
                .or(thread.messages.last())
                .map(|message| message.id)
        });
        let pinned = self.prefs.toggle_remote_pin(&key, latest);
        self.toast(if pinned { "Pinned." } else { "Unpinned." });
        self.thread_header_stamp.clear();
        self.fill_open_thread_header(self.open_thread.as_ref(), sender.input_sender().clone());
        self.render_inbox(sender);
    }

    fn stash_open_remote(&mut self, sender: &ComponentSender<App>) {
        let Some(reference) = self.thread_ref.clone() else {
            return;
        };
        let archived = self
            .open_thread
            .as_ref()
            .map(|thread| thread.is_archived)
            .or_else(|| {
                self.conversation_for(&reference)
                    .map(|item| item.is_archived)
            })
            .unwrap_or(false);
        let next = !archived;
        let Some(client) = self.client.clone() else {
            return;
        };
        self.begin_busy();
        let title = reference.title();
        let sender2 = sender.clone();
        relm4::spawn(async move {
            sender2.input(AppMsg::ThreadActionDone(
                client.set_archived(&reference, next).await.map(|()| {
                    if next {
                        format!("Stashed {title}. It pops back when someone writes.")
                    } else {
                        format!("Popped {title} back into Open.")
                    }
                }),
            ));
        });
    }

    fn mute_open_remote(&mut self, sender: &ComponentSender<App>) {
        let Some(reference) = self.thread_ref.clone() else {
            return;
        };
        let muted = self.is_remote_muted(&reference);
        let Some(client) = self.client.clone() else {
            return;
        };
        self.apply_local_mute(&reference, !muted);
        self.thread_header_stamp.clear();
        self.fill_open_thread_header(self.open_thread.as_ref(), sender.input_sender().clone());
        self.begin_busy();
        let sender2 = sender.clone();
        relm4::spawn(async move {
            sender2.input(AppMsg::ThreadActionDone(
                client.set_remote_muted(&reference, !muted).await.map(|_| {
                    if muted {
                        "Unmuted.".into()
                    } else {
                        "Muted. You won't hear new messages from this remote.".into()
                    }
                }),
            ));
        });
    }

    fn leave_open_org(&mut self, sender: &ComponentSender<App>) {
        let Some(ThreadRef::Org { id, .. }) = self.thread_ref.clone() else {
            return;
        };
        let Some(username) = self.user.as_ref().map(|user| user.username().to_owned()) else {
            return;
        };
        let Some(client) = self.client.clone() else {
            return;
        };
        self.begin_busy();
        let sender2 = sender.clone();
        relm4::spawn(async move {
            sender2.input(AppMsg::ThreadActionDone(
                client
                    .leave_org(&id, &username)
                    .await
                    .map(|()| "Left the org.".into()),
            ));
        });
    }

    fn block_open_remote(&mut self, sender: &ComponentSender<App>) {
        let Some(ThreadRef::Direct { username }) = self.thread_ref.clone() else {
            return;
        };
        let Some(client) = self.client.clone() else {
            return;
        };
        self.begin_busy();
        let sender2 = sender.clone();
        relm4::spawn(async move {
            sender2.input(AppMsg::ThreadActionDone(
                client.block(&username).await.map(|()| "Blocked.".into()),
            ));
        });
    }

    fn load_safety_quiet(&self, sender: &ComponentSender<App>) {
        let Some(client) = self.client.clone() else {
            return;
        };
        let sender2 = sender.clone();
        relm4::spawn(async move {
            sender2.input(AppMsg::SafetyLoaded(
                client.mutes().await,
                client.blocks().await,
            ));
        });
    }

    fn automatic_translation(&self) -> bool {
        self.user_settings
            .as_ref()
            .map(|settings| settings.automatic_translation)
            .unwrap_or(true)
    }

    fn translation_language_code(&self) -> String {
        self.user_settings
            .as_ref()
            .and_then(|settings| settings.translation_language.as_deref())
            .map(str::trim)
            .filter(|code| !code.is_empty())
            .map(str::to_owned)
            .unwrap_or_else(device_language)
    }

    fn who_to_follow_after(&self) -> Option<usize> {
        if !self.prefs.show_who_to_follow || self.aside_visible() {
            return None;
        }
        if self.visible != "log" || self.nav.log().is_none() {
            return None;
        }
        ui::who_to_follow_index(self.log_commits.len())
    }

    fn maybe_refresh_who_to_follow(&mut self, sender: &ComponentSender<App>) {
        if self.who_to_follow_after().is_some() {
            self.refresh_visible_feed(sender, true);
        }
    }

    fn patch_settings(&mut self, patch: impl FnOnce(&mut UserSettings)) {
        patch(self.user_settings.get_or_insert_with(UserSettings::default));
    }

    fn is_muted(&self, username: &str) -> bool {
        self.muted
            .iter()
            .any(|user| user.username.eq_ignore_ascii_case(username))
    }

    fn is_blocked(&self, username: &str) -> bool {
        self.blocked
            .iter()
            .any(|user| user.username.eq_ignore_ascii_case(username))
    }

    fn load_user_preview(&mut self, username: &str, sender: &ComponentSender<App>) {
        let key = UserPreview::key(username);
        if key.is_empty() || self.user_card.cached(&key).is_some() {
            return;
        }
        if !self.user_preview_loading.insert(key) {
            return;
        }
        let Some(client) = self.client.clone() else {
            return;
        };
        let username = username.to_owned();
        let sender2 = sender.clone();
        relm4::spawn(async move {
            sender2.input(AppMsg::UserPreviewLoaded(
                username.clone(),
                client.user_preview(&username).await,
            ));
        });
    }

    fn load_user_settings(&self, sender: &ComponentSender<App>) {
        let Some(client) = self.client.clone() else {
            return;
        };
        let sender2 = sender.clone();
        relm4::spawn(async move {
            sender2.input(AppMsg::UserSettingsLoaded(client.user_settings().await));
        });
    }

    fn translate_commit(&mut self, id: &str, sender: &ComponentSender<App>) {
        self.showing_original.remove(id);
        self.requested_translation.insert(id.to_owned());
        if self
            .translations
            .get(id)
            .is_some_and(CommitTranslation::shows_translation)
        {
            self.refresh_visible_feed(sender, true);
            return;
        }
        let Some(client) = self.client.clone() else {
            return;
        };
        if !self.translation_loading.insert(id.to_owned()) {
            return;
        }
        self.refresh_visible_feed(sender, true);
        let language = self.translation_language_code();
        let id = id.to_owned();
        let sender2 = sender.clone();
        relm4::spawn(async move {
            sender2.input(AppMsg::CommitTranslated(
                id.clone(),
                client.translate_commit(&id, &language).await,
            ));
        });
    }

    fn queue_auto_translations_visible(&mut self, sender: &ComponentSender<App>) {
        let mut commits = match self.visible {
            "profile" => self.profile_commits.clone(),
            "commit" => {
                let mut all = self.ancestors.clone();
                if let Some(commit) = &self.open_commit {
                    all.push(commit.clone());
                }
                all.extend(self.thread_replies.iter().cloned());
                all
            }
            "log" => self.log_commits.clone(),
            _ => Vec::new(),
        };
        for replies in self.nested_replies.values() {
            commits.extend(replies.iter().cloned());
        }
        self.queue_auto_translations(&commits, sender);
    }

    fn queue_auto_translations(&mut self, commits: &[Commit], sender: &ComponentSender<App>) {
        if !self.automatic_translation() || self.user_settings.is_none() {
            return;
        }
        if self
            .auto_translate_retry_at
            .is_some_and(|until| Instant::now() < until)
        {
            return;
        }
        let Some(client) = self.client.clone() else {
            return;
        };
        let me_id = self.user.as_ref().map(|user| user.user.id.clone());
        let language = self.translation_language_code();
        let mut ids = Vec::new();
        for commit in commits {
            if ids.len() >= 8 {
                break;
            }
            if commit.is_own(me_id.as_deref()) {
                continue;
            }
            if commit.body.as_ref().is_none_or(|body| body.is_empty()) {
                continue;
            }
            if self.translations.contains_key(&commit.id) {
                continue;
            }
            if !self.translation_loading.insert(commit.id.clone()) {
                continue;
            }
            ids.push(commit.id.clone());
        }
        if ids.is_empty() {
            return;
        }
        let sender2 = sender.clone();
        relm4::spawn(async move {
            for id in ids {
                let result = client.translate_commit(&id, &language).await;
                sender2.input(AppMsg::CommitTranslated(id, result));
            }
        });
    }

    fn remotes_requests(&self) -> bool {
        self.user
            .as_ref()
            .and_then(|user| user.remotes.as_ref())
            .is_some_and(|remotes| remotes.requests)
    }

    fn fetched_tab_label(&self) -> String {
        if self.inbox_requests > 0 {
            format!("Fetched {}", self.inbox_requests)
        } else {
            "Fetched".into()
        }
    }

    fn inbox_tab_classes(&self, tab: InboxTab) -> &'static [&'static str] {
        if self.inbox_tab == tab {
            &["flat", "dn-tab", "dn-inbox-tab", "active"]
        } else {
            &["flat", "dn-tab", "dn-inbox-tab", "inactive"]
        }
    }

    fn ping_remote_typing(&mut self) {
        if self.remote_draft_len == 0 {
            return;
        }
        let now = Instant::now();
        if self
            .last_typing_ping
            .is_some_and(|at| now.duration_since(at) < Duration::from_secs(3))
        {
            return;
        }
        let Some(reference) = self.thread_ref.clone() else {
            return;
        };
        let Some(client) = self.client.clone() else {
            return;
        };
        self.last_typing_ping = Some(now);
        relm4::spawn(async move {
            let _ = client.ping_typing(&reference).await;
        });
    }

    fn dock_toggle_classes(&self) -> &'static [&'static str] {
        if self.prefs.chat_docked {
            &["dn-dock-toggle", "docked"]
        } else {
            &["dn-dock-toggle"]
        }
    }

    fn fail(&mut self, err: ApiError) {
        if err.requires_sign_in() {
            self.phase = Phase::SignedOut;
            self.visible = "sign_in";
            self.client = None;
            self.set_user(None);
        }
        self.toast(&err.message);
    }
}

async fn restore() -> Result<(CurrentUser, Client), ApiError> {
    let key = auth::load_key()
        .await
        .map_err(|e| ApiError {
            status: 0,
            code: None,
            message: e.to_string(),
        })?
        .ok_or_else(|| ApiError {
            status: 401,
            code: Some("device_not_approved".into()),
            message: "Not signed in.".into(),
        })?;
    let client = Client::signed(key)?;
    let user = client.me().await?;
    Ok((user, client))
}

async fn redeem(
    code: String,
    verifier: String,
    key: Option<DeviceKey>,
) -> Result<(CurrentUser, Client), ApiError> {
    let key = match key {
        Some(key) => key,
        None => auth::load_or_create_key().await.map_err(|e| ApiError {
            status: 0,
            code: None,
            message: e.to_string(),
        })?,
    };
    let client = Client::signed(key)?;
    let user = client.redeem(&code, &verifier).await?;
    Ok((user, client))
}

fn typing_copy(thread: &ChatThread, reference: Option<&ThreadRef>) -> String {
    if !thread.typing.is_empty() {
        return match thread.typing.as_slice() {
            [one] => format!("@{one} is typing…"),
            [one, two] => format!("@{one} and @{two} are typing…"),
            rest => format!("{} people are typing…", rest.len()),
        };
    }
    if thread.partner_typing {
        if let Some(ThreadRef::Direct { username }) = reference {
            return format!("@{username} is typing…");
        }
        return "typing…".into();
    }
    String::new()
}

fn buffer_text(view: &gtk::TextView) -> String {
    let buf = view.buffer();
    buf.text(&buf.start_iter(), &buf.end_iter(), false)
        .to_string()
}

fn set_buffer_text_caret(view: &gtk::TextView, text: &str, caret: usize) {
    let buf = view.buffer();
    buf.set_text(text);
    let offset = i32::try_from(caret)
        .unwrap_or(i32::MAX)
        .min(buf.end_iter().offset());
    buf.place_cursor(&buf.iter_at_offset(offset));
}

#[derive(Clone, Copy)]
enum ComposerBind {
    Log,
    Reply,
    Remote,
}

fn bind_composer_keys(
    view: &gtk::TextView,
    sender: ComponentSender<App>,
    emoji_open: Rc<Cell<bool>>,
    mention_open: Rc<Cell<bool>>,
    kind: ComposerBind,
) {
    let sync = Rc::new({
        let sender = sender.clone();
        move || {
            sender.input(match kind {
                ComposerBind::Log => AppMsg::ComposerChanged,
                ComposerBind::Reply => AppMsg::ReplyComposerChanged,
                ComposerBind::Remote => AppMsg::RemoteComposerChanged,
            });
        }
    });
    view.buffer().connect_changed({
        let sync = sync.clone();
        move |_| sync()
    });
    view.buffer()
        .connect_notify_local(Some("cursor-position"), {
            let sync = sync.clone();
            move |_, _| sync()
        });
    let keys = gtk::EventControllerKey::new();
    keys.set_propagation_phase(gtk::PropagationPhase::Capture);
    keys.connect_key_pressed(move |_, key, _, mods| {
        let ctrl = mods.contains(gtk::gdk::ModifierType::CONTROL_MASK);
        let shift = mods.contains(gtk::gdk::ModifierType::SHIFT_MASK);
        let enter = key == gtk::gdk::Key::Return || key == gtk::gdk::Key::KP_Enter;
        if enter && ctrl {
            match kind {
                ComposerBind::Log => sender.input(AppMsg::SubmitCommit),
                ComposerBind::Reply => sender.input(AppMsg::SubmitReply),
                ComposerBind::Remote => sender.input(AppMsg::PushRemote),
            }
            return glib::Propagation::Stop;
        }
        if ctrl
            && shift
            && matches!(kind, ComposerBind::Log)
            && (key == gtk::gdk::Key::p || key == gtk::gdk::Key::P)
        {
            sender.input(AppMsg::TogglePreview);
            return glib::Propagation::Stop;
        }
        if mention_open.get() {
            if key == gtk::gdk::Key::Down || key == gtk::gdk::Key::KP_Down {
                sender.input(AppMsg::MentionMove(1));
                return glib::Propagation::Stop;
            }
            if key == gtk::gdk::Key::Up || key == gtk::gdk::Key::KP_Up {
                sender.input(AppMsg::MentionMove(-1));
                return glib::Propagation::Stop;
            }
            if (enter || key == gtk::gdk::Key::Tab) && !ctrl && !shift {
                sender.input(AppMsg::MentionPick);
                return glib::Propagation::Stop;
            }
            if key == gtk::gdk::Key::Escape {
                sender.input(AppMsg::MentionDismiss);
                return glib::Propagation::Stop;
            }
        }
        if emoji_open.get() {
            if key == gtk::gdk::Key::Down || key == gtk::gdk::Key::KP_Down {
                sender.input(AppMsg::EmojiMove(1));
                return glib::Propagation::Stop;
            }
            if key == gtk::gdk::Key::Up || key == gtk::gdk::Key::KP_Up {
                sender.input(AppMsg::EmojiMove(-1));
                return glib::Propagation::Stop;
            }
            if (enter || key == gtk::gdk::Key::Tab) && !ctrl && !shift {
                sender.input(AppMsg::EmojiPick);
                return glib::Propagation::Stop;
            }
            if key == gtk::gdk::Key::Escape {
                sender.input(AppMsg::EmojiDismiss);
                return glib::Propagation::Stop;
            }
        }
        if matches!(kind, ComposerBind::Remote) && key == gtk::gdk::Key::Escape {
            sender.input(AppMsg::CancelRemoteReply);
            return glib::Propagation::Stop;
        }
        glib::Propagation::Proceed
    });
    view.add_controller(keys);
}

fn is_text_input(widget: &gtk::Widget) -> bool {
    widget.is::<gtk::Text>()
        || widget.is::<gtk::TextView>()
        || widget.is::<gtk::Entry>()
        || widget.is::<gtk::PasswordEntry>()
        || widget.is::<gtk::SearchEntry>()
        || widget.ancestor(gtk::TextView::static_type()).is_some()
        || widget.ancestor(gtk::Entry::static_type()).is_some()
}

fn device_language() -> String {
    std::env::var("LC_ALL")
        .ok()
        .or_else(|| std::env::var("LC_MESSAGES").ok())
        .or_else(|| std::env::var("LANG").ok())
        .and_then(|value| language_code(&value))
        .or_else(|| {
            glib::language_names()
                .into_iter()
                .find_map(|name| language_code(&name))
        })
        .unwrap_or_else(|| "en".to_owned())
}

fn language_code(name: &str) -> Option<String> {
    let code = name
        .split(['_', '-', '.', '@'])
        .next()
        .unwrap_or(name)
        .trim()
        .to_ascii_lowercase();
    (code.len() == 2 && code.chars().all(|c| c.is_ascii_alphabetic())).then_some(code)
}

fn open_web(url: &str) {
    #[cfg(target_os = "macos")]
    {
        let _ = std::process::Command::new("/usr/bin/open").arg(url).spawn();
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = gio::AppInfo::launch_default_for_uri(url, None::<&gio::AppLaunchContext>);
    }
}

fn poll_clipboard(sender: &ComponentSender<App>) {
    if let Some(uri) = auth::take_callback_uri() {
        sender.input(AppMsg::OpenUri(uri));
        return;
    }
    let Some(display) = gtk::gdk::Display::default() else {
        return;
    };
    let clipboard = display.clipboard();
    let sender = sender.clone();
    clipboard.read_text_async(None::<&gio::Cancellable>, move |result| {
        if let Ok(Some(text)) = result {
            if auth::extract_callback(&text).is_some() {
                sender.input(AppMsg::OpenUri(text.to_string()));
            }
        }
    });
}

fn pick_images(sender: &ComponentSender<App>) {
    let dialog = gtk::FileDialog::new();
    let filter = gtk::FileFilter::new();
    filter.set_name(Some("Images"));
    filter.add_mime_type("image/jpeg");
    filter.add_mime_type("image/png");
    filter.add_mime_type("image/webp");
    let filters = gio::ListStore::new::<gtk::FileFilter>();
    filters.append(&filter);
    dialog.set_filters(Some(&filters));
    dialog.set_default_filter(Some(&filter));
    let sender = sender.clone();
    let parent = relm4::main_application().active_window();
    dialog.open_multiple(parent.as_ref(), None::<&gio::Cancellable>, move |result| {
        let Ok(files) = result else {
            return;
        };
        let mut uploads = Vec::new();
        let mut skipped = 0;
        for i in 0..files.n_items() {
            if let Some(file) = files.item(i).and_then(|o| o.downcast::<gio::File>().ok()) {
                if let Some(upload) = ui::read_upload_file(&file) {
                    uploads.push(upload);
                } else {
                    skipped += 1;
                }
            }
        }
        if skipped > 0 {
            sender.input(AppMsg::Toast(
                "Only JPEG, PNG, or WebP images up to 10 MB.".into(),
            ));
        }
        if !uploads.is_empty() {
            sender.input(AppMsg::ImagesPicked(uploads));
        }
    });
}

const FEED_MAX: i32 = 620;
const REMOTES_FEED_MAX: i32 = 940;
const FLUID_FEED_MAX: i32 = 4096;

fn thread_people(thread: &ChatThread, me: Option<&User>) -> HashMap<String, User> {
    let mut people = HashMap::new();
    if let Some(me) = me {
        if !me.id.is_empty() {
            people.insert(me.id.clone(), me.clone());
        }
    }
    if let Some(partner) = &thread.partner {
        if !partner.id.is_empty() {
            people.insert(partner.id.clone(), partner.clone());
        }
    }
    for member in &thread.members {
        if !member.user.id.is_empty() {
            people.insert(member.user.id.clone(), member.user.clone());
        }
    }
    for message in &thread.messages {
        if let Some(user) = &message.user {
            if !user.id.is_empty() {
                people.insert(user.id.clone(), user.clone());
            }
        }
        if let Some(subject) = &message.subject {
            if !subject.id.is_empty() {
                people.insert(subject.id.clone(), subject.clone());
            }
        }
    }
    people
}

fn newest_replies(mut replies: Vec<Commit>) -> Vec<Commit> {
    ui::sort_replies_newest_first(&mut replies);
    replies
}

fn reply_preview_ids(commits: &[Commit]) -> Vec<String> {
    commits
        .iter()
        .filter(|commit| !commit.is_deleted() && commit.replies_count.unwrap_or(0) > 0)
        .map(|commit| commit.id.clone())
        .collect()
}

fn fill_feed_list(list: &gtk::ListBox) {
    list.set_hexpand(true);
    list.set_vexpand(true);
}

fn feed_column_width(visible: &str, shell_compact: bool) -> i32 {
    if shell_compact {
        FLUID_FEED_MAX
    } else if visible == "remotes" {
        REMOTES_FEED_MAX
    } else {
        FEED_MAX
    }
}

fn feed_column_width_request(visible: &str, shell_compact: bool) -> i32 {
    if feed_column_fills(shell_compact) {
        -1
    } else {
        feed_column_width(visible, shell_compact)
    }
}

fn feed_column_fills(shell_compact: bool) -> bool {
    shell_compact
}

fn feed_column_classes(visible: &str, shell_compact: bool) -> [&'static str; 2] {
    if shell_compact {
        ["dn-main", "fluid"]
    } else if visible == "remotes" {
        ["dn-main", "remotes"]
    } else {
        ["dn-main", "feed"]
    }
}

fn width_hides_aside(aside_compact: bool, nav_compact: bool, shell_compact: bool) -> bool {
    aside_compact || nav_compact || shell_compact
}

fn remotes_inbox_visible(aside_compact: bool, thread_open: bool) -> bool {
    !aside_compact || !thread_open
}

fn remotes_thread_visible(aside_compact: bool, thread_open: bool) -> bool {
    thread_open || !aside_compact
}

fn aside_is_visible(
    signed_in: bool,
    aside_compact: bool,
    nav_compact: bool,
    shell_compact: bool,
    visible: &str,
) -> bool {
    signed_in
        && !width_hides_aside(aside_compact, nav_compact, shell_compact)
        && visible != "remotes"
}

fn nav_shows_search(aside_compact: bool, nav_compact: bool, shell_compact: bool) -> bool {
    width_hides_aside(aside_compact, nav_compact, shell_compact) && !shell_compact
}

fn nav_is_collapsed(sidebar_collapsed: Option<bool>, nav_compact: bool) -> bool {
    sidebar_collapsed.unwrap_or(nav_compact)
}

fn latest_ping_id(messages: &[Message]) -> Option<i64> {
    messages
        .iter()
        .filter(|message| message.event.as_deref() == Some("ping"))
        .map(|message| message.id)
        .max()
}

#[cfg(test)]
fn commit_button_expands(sidebar_collapsed: Option<bool>, nav_compact: bool) -> bool {
    !nav_is_collapsed(sidebar_collapsed, nav_compact)
}

#[cfg(test)]
mod shell_tests {
    use super::*;

    #[test]
    fn collapse_follows_the_breakpoint_until_the_user_pins_it() {
        assert!(nav_is_collapsed(None, true));
        assert!(!nav_is_collapsed(None, false));
    }

    #[test]
    fn user_expand_stays_open_under_the_icon_rail_breakpoint() {
        assert!(!nav_is_collapsed(Some(false), true));
        assert!(nav_is_collapsed(Some(true), false));
    }

    #[test]
    fn commit_button_fills_the_rail_only_when_labels_are_showing() {
        assert!(commit_button_expands(None, false));
        assert!(!commit_button_expands(Some(true), false));
        assert!(commit_button_expands(Some(false), true));
    }

    #[test]
    fn latest_ping_is_the_newest_ping_event() {
        assert_eq!(
            latest_ping_id(&[
                Message {
                    id: 1,
                    event: Some("ping".into()),
                    ..Message::default()
                },
                Message {
                    id: 2,
                    body: Some("hi".into()),
                    ..Message::default()
                },
                Message {
                    id: 4,
                    event: Some("ping".into()),
                    ..Message::default()
                },
            ]),
            Some(4)
        );
        assert_eq!(
            latest_ping_id(&[Message {
                id: 3,
                body: Some("hi".into()),
                ..Message::default()
            }]),
            None
        );
    }

    #[test]
    fn feed_stays_capped_until_the_sidebars_are_gone() {
        assert_eq!(feed_column_width("log", false), 620);
        assert_eq!(feed_column_width("profile", false), 620);
        assert_eq!(feed_column_width("notifications", false), 620);
        assert_eq!(feed_column_width("remotes", false), 940);
        assert_eq!(feed_column_width("log", true), 4096);
        assert_eq!(feed_column_width("remotes", true), 4096);
        assert!(!feed_column_fills(false));
        assert!(feed_column_fills(true));
        assert_eq!(feed_column_classes("log", false), ["dn-main", "feed"]);
        assert_eq!(
            feed_column_classes("remotes", false),
            ["dn-main", "remotes"]
        );
        assert_eq!(feed_column_classes("log", true), ["dn-main", "fluid"]);
        assert_eq!(feed_column_width_request("log", false), 620);
        assert_eq!(feed_column_width_request("profile", false), 620);
        assert_eq!(feed_column_width_request("remotes", false), 940);
        assert_eq!(feed_column_width_request("log", true), -1);
    }

    #[test]
    fn aside_stays_hidden_once_a_narrower_breakpoint_wins() {
        assert!(aside_is_visible(true, false, false, false, "log"));
        assert!(!aside_is_visible(true, true, false, false, "log"));
        assert!(!aside_is_visible(true, false, true, false, "log"));
        assert!(!aside_is_visible(true, false, false, true, "log"));
        assert!(!aside_is_visible(true, false, false, false, "remotes"));
        assert!(!aside_is_visible(false, false, false, false, "log"));
        assert!(nav_shows_search(true, false, false));
        assert!(nav_shows_search(false, true, false));
        assert!(!nav_shows_search(false, false, true));
        assert!(!nav_shows_search(false, false, false));
    }

    #[test]
    fn device_language_code_takes_the_first_two_letters() {
        assert_eq!(language_code("en_US.UTF-8").as_deref(), Some("en"));
        assert_eq!(language_code("pt-BR").as_deref(), Some("pt"));
        assert_eq!(language_code("C").as_deref(), None);
        assert_eq!(language_code("POSIX").as_deref(), None);
    }

    #[test]
    fn remotes_hides_the_org_pane_before_the_nav_rail() {
        assert!(remotes_inbox_visible(false, true));
        assert!(remotes_inbox_visible(false, false));
        assert!(remotes_inbox_visible(true, false));
        assert!(!remotes_inbox_visible(true, true));
        assert!(remotes_thread_visible(false, false));
        assert!(remotes_thread_visible(false, true));
        assert!(remotes_thread_visible(true, true));
        assert!(!remotes_thread_visible(true, false));
    }
}
