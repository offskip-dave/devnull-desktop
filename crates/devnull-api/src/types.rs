use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::date::deserialize_opt_datetime;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(untagged)]
pub enum Lane {
    Palette(u8),
    Custom(String),
}

impl Lane {
    pub fn css_color(&self, dark: bool) -> String {
        match self {
            Lane::Custom(hex) => hex.clone(),
            Lane::Palette(i) => {
                let colors: [&str; 6] = if dark {
                    [
                        "#ff8a4c", "#51cf66", "#da77f2", "#3bc9db", "#f06595", "#fcc419",
                    ]
                } else {
                    [
                        "#d9480f", "#2f9e44", "#9c36b5", "#0c8599", "#c2255c", "#b7791f",
                    ]
                };
                colors[(*i as usize) % 6].to_owned()
            }
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct User {
    pub id: String,
    pub username: String,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub avatar_url: Option<String>,
    #[serde(default)]
    pub bio: Option<String>,
    #[serde(default)]
    pub company: Option<String>,
    #[serde(default)]
    pub location: Option<String>,
    #[serde(default)]
    pub website: Option<String>,
    #[serde(default)]
    pub github_url: Option<String>,
    #[serde(default)]
    pub html_url: Option<String>,
    #[serde(default)]
    pub feed_url: Option<String>,
    #[serde(default, deserialize_with = "deserialize_opt_datetime")]
    pub created_at: Option<DateTime<Utc>>,
    #[serde(default)]
    pub commits_count: Option<i64>,
    #[serde(default)]
    pub followers_count: Option<i64>,
    #[serde(default)]
    pub following_count: Option<i64>,
    #[serde(default)]
    pub lane: Option<Lane>,
    #[serde(default)]
    pub is_followed: Option<bool>,
    #[serde(default)]
    pub follows_you: Option<bool>,
    #[serde(default)]
    pub has_sponsors_listing: Option<bool>,
    #[serde(default)]
    pub timezone: Option<String>,
}

impl User {
    pub fn display_name(&self) -> &str {
        match &self.name {
            Some(name) if !name.trim().is_empty() => name,
            _ => &self.username,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RemotesSettings {
    pub from: String,
    #[serde(default)]
    pub pull_receipts: bool,
    #[serde(default)]
    pub requests: bool,
    #[serde(default)]
    pub everyone: bool,
    #[serde(default)]
    pub unread_count: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CurrentUser {
    #[serde(flatten)]
    pub user: User,
    #[serde(default)]
    pub pasteis_mode: Option<bool>,
    #[serde(default)]
    pub pasteis_remaining: Option<i64>,
    #[serde(default)]
    pub pasteis_received: Option<i64>,
    #[serde(default)]
    pub remotes: Option<RemotesSettings>,
    #[serde(default, alias = "unreadNotificationsCount")]
    pub unread_notifications_count: Option<i64>,
    #[serde(default, alias = "unreadMessagesCount")]
    pub unread_messages_count: Option<i64>,
    #[serde(default)]
    pub preferred_lane: Option<Lane>,
    #[serde(default)]
    pub automatic_lane: Option<u8>,
    #[serde(default, deserialize_with = "deserialize_opt_datetime")]
    pub github_synced_at: Option<DateTime<Utc>>,
    #[serde(default)]
    pub timezone: Option<String>,
}

impl CurrentUser {
    pub fn username(&self) -> &str {
        &self.user.username
    }

    pub fn pasteis_on(&self) -> bool {
        self.pasteis_mode.unwrap_or(false)
    }

    pub fn remotes_on(&self) -> bool {
        self.remotes.is_some()
    }

    pub fn unread_notifications(&self) -> i64 {
        self.unread_notifications_count.unwrap_or(0)
    }

    pub fn unread_remotes(&self) -> i64 {
        self.unread_messages_count
            .or_else(|| self.remotes.as_ref().map(|remotes| remotes.unread_count))
            .unwrap_or(0)
    }

    pub fn timezone(&self) -> Option<&str> {
        self.timezone
            .as_deref()
            .or(self.user.timezone.as_deref())
            .filter(|value| !value.is_empty())
    }

    pub fn chosen_lane(&self) -> Option<&Lane> {
        self.preferred_lane.as_ref().or(self.user.lane.as_ref())
    }
}

impl User {
    pub fn list_from_value(value: serde_json::Value) -> Vec<Self> {
        let data = value.get("data").cloned().unwrap_or(value.clone());
        if let Some(users) = json_vec::<User>(&data, "users") {
            if !users.is_empty() {
                return users;
            }
        }
        for key in ["muted", "blocked", "mutes", "blocks"] {
            if let Some(users) = json_vec::<User>(&data, key).or_else(|| json_vec(&value, key)) {
                return users;
            }
        }
        if let Ok(page) = serde_json::from_value::<Page<User>>(value.clone()) {
            return page.data;
        }
        if data.is_array() {
            return serde_json::from_value(data).unwrap_or_default();
        }
        Vec::new()
    }
}

#[derive(Debug, Clone, Default)]
pub struct Profile {
    pub user: User,
    pub is_following: bool,
    pub follows_you: bool,
    pub is_muting: bool,
    pub pinned_commit: Option<Commit>,
    pub streaks: Streaks,
    pub activity: Activity,
    pub achievements: Option<Achievements>,
    pub pasteis_received: Option<i64>,
}

impl Profile {
    pub fn from_value(value: serde_json::Value) -> Result<Self, serde_json::Error> {
        let data = value.get("data").cloned().unwrap_or(value.clone());
        let user_value = data.get("user").cloned().unwrap_or_else(|| data.clone());
        let mut user: User = serde_json::from_value(user_value)?;
        let counts = data
            .get("counts")
            .cloned()
            .unwrap_or(serde_json::Value::Null);
        if user.commits_count.is_none() {
            user.commits_count =
                json_i64(&data, "commits_count").or_else(|| json_i64(&counts, "commits"));
        }
        if user.followers_count.is_none() {
            user.followers_count =
                json_i64(&data, "followers_count").or_else(|| json_i64(&counts, "followers"));
        }
        if user.following_count.is_none() {
            user.following_count =
                json_i64(&data, "following_count").or_else(|| json_i64(&counts, "following"));
        }
        let is_following = json_bool(&value, "is_following")
            .or_else(|| json_bool(&data, "is_following"))
            .or(user.is_followed)
            .unwrap_or(false);
        let follows_you = json_bool(&value, "follows_you")
            .or_else(|| json_bool(&data, "follows_you"))
            .or(user.follows_you)
            .unwrap_or(false);
        user.is_followed = Some(is_following);
        user.follows_you = Some(follows_you);
        let pinned = nested(&value, &data, "pinned_commit")
            .cloned()
            .and_then(|v| serde_json::from_value(v).ok());
        let streaks = nested(&value, &data, "streaks")
            .cloned()
            .and_then(|v| serde_json::from_value(v).ok())
            .unwrap_or_default();
        let activity = nested(&value, &data, "activity")
            .cloned()
            .and_then(|v| serde_json::from_value(v).ok())
            .unwrap_or_default();
        let achievements = nested(&value, &data, "achievements")
            .cloned()
            .and_then(|v| serde_json::from_value::<Achievements>(v).ok())
            .filter(|a| !a.rank.is_empty());
        let pasteis_received =
            json_i64(&value, "pasteis_received").or_else(|| json_i64(&data, "pasteis_received"));
        Ok(Self {
            user,
            is_following,
            follows_you,
            is_muting: json_bool(&value, "is_muting")
                .or_else(|| json_bool(&data, "is_muting"))
                .unwrap_or(false),
            pinned_commit: pinned,
            streaks,
            activity,
            achievements,
            pasteis_received,
        })
    }

    pub fn username(&self) -> &str {
        &self.user.username
    }

    pub fn html_url(&self, base: &str) -> String {
        self.user
            .html_url
            .clone()
            .unwrap_or_else(|| format!("{}/@{}", base.trim_end_matches('/'), self.user.username))
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Activity {
    #[serde(default)]
    pub scale: String,
    #[serde(default)]
    pub buckets: Vec<ActivityBucket>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ActivityBucket {
    pub date: String,
    #[serde(default)]
    pub count: i64,
}

impl Activity {
    pub fn window(&self) -> &'static str {
        match self.scale.as_str() {
            "day" => "in the last 24 hours",
            "week" => "in the last 7 days",
            "month" => "in the last 30 days",
            _ => "in the last year",
        }
    }

    pub fn cells(&self) -> Vec<ActivityCell> {
        let max = self
            .buckets
            .iter()
            .map(|b| b.count)
            .max()
            .unwrap_or(0)
            .max(1);
        self.buckets
            .iter()
            .cloned()
            .map(|bucket| {
                let level = if bucket.count == 0 {
                    0
                } else {
                    ((bucket.count * 4 + max - 1) / max).clamp(1, 4) as u8
                };
                ActivityCell {
                    count: bucket.count,
                    level,
                    date: bucket.date,
                }
            })
            .collect()
    }
}

#[derive(Debug, Clone)]
pub struct ActivityCell {
    pub count: i64,
    pub level: u8,
    pub date: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Streaks {
    #[serde(default)]
    pub longest: i64,
    #[serde(default)]
    pub current: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Achievements {
    #[serde(default)]
    pub level: i64,
    #[serde(default)]
    pub rank: String,
    #[serde(default)]
    pub xp: i64,
    #[serde(default)]
    pub next_level_xp: Option<i64>,
    #[serde(default)]
    pub level_progress: i64,
    #[serde(default)]
    pub ranks: Vec<Rank>,
    #[serde(default)]
    pub badges: Vec<Badge>,
}

impl Achievements {
    pub fn label(&self) -> String {
        format!("Lv. {} · {}", self.level, self.rank)
    }

    pub fn xp_label(&self) -> String {
        match self.next_level_xp {
            Some(next) => format!("{} / {next} XP", self.xp),
            None => format!("{} XP", self.xp),
        }
    }

    pub fn next_rank_label(&self) -> String {
        match self.ranks.iter().find(|rank| rank.level == self.level + 1) {
            Some(next) => format!("{} XP to {}", (next.min_xp - self.xp).max(0), next.rank),
            None => "Maximum rank (UID 0)".into(),
        }
    }

    pub fn unlocked_count(&self) -> usize {
        self.badges.iter().filter(|b| b.unlocked).count()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Rank {
    #[serde(default)]
    pub rank: String,
    #[serde(default)]
    pub level: i64,
    #[serde(default)]
    pub min_xp: i64,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub achieved: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Badge {
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub icon: String,
    #[serde(default)]
    pub unlocked: bool,
    #[serde(default)]
    pub progress: i64,
    #[serde(default)]
    pub target: i64,
}

#[derive(Debug, Clone, Default)]
pub struct SearchResults {
    pub users: Vec<User>,
    pub commits: Vec<Commit>,
}

impl SearchResults {
    pub fn from_value(value: serde_json::Value) -> Self {
        let data = value.get("data").cloned().unwrap_or(value.clone());
        let users = json_vec::<User>(&data, "users").unwrap_or_else(|| {
            if data.is_array() {
                serde_json::from_value(data.clone()).unwrap_or_default()
            } else {
                Vec::new()
            }
        });
        let commits = json_vec::<Commit>(&data, "commits").unwrap_or_default();
        Self { users, commits }
    }

    pub fn is_empty(&self) -> bool {
        self.users.is_empty() && self.commits.is_empty()
    }
}

fn json_i64(value: &serde_json::Value, key: &str) -> Option<i64> {
    value.get(key).and_then(|v| {
        v.as_i64()
            .or_else(|| v.as_u64().map(|n| n as i64))
            .or_else(|| v.as_f64().map(|n| n as i64))
    })
}

fn json_bool(value: &serde_json::Value, key: &str) -> Option<bool> {
    value.get(key).and_then(serde_json::Value::as_bool)
}

fn nested<'a>(
    root: &'a serde_json::Value,
    data: &'a serde_json::Value,
    key: &str,
) -> Option<&'a serde_json::Value> {
    root.get(key)
        .or_else(|| data.get(key))
        .filter(|v| !v.is_null())
}

fn json_vec<T: serde::de::DeserializeOwned>(
    value: &serde_json::Value,
    key: &str,
) -> Option<Vec<T>> {
    value
        .get(key)
        .cloned()
        .and_then(|v| serde_json::from_value(v).ok())
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Embed {
    #[serde(default)]
    pub provider: Option<String>,
    #[serde(default)]
    pub kind: Option<String>,
    #[serde(default)]
    pub src: Option<String>,
    #[serde(default)]
    pub poster: Option<String>,
    #[serde(default)]
    pub aspect_ratio: Option<String>,
    #[serde(default)]
    pub height: Option<i32>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct LinkPreview {
    pub url: String,
    #[serde(default)]
    pub title: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub image_url: Option<String>,
    #[serde(default)]
    pub site_name: Option<String>,
    #[serde(default)]
    pub is_pending: Option<bool>,
    #[serde(default)]
    pub embed: Option<Embed>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct CommitImage {
    pub id: String,
    #[serde(default)]
    pub url: Option<String>,
    #[serde(default)]
    pub width: Option<i32>,
    #[serde(default)]
    pub height: Option<i32>,
    #[serde(default)]
    pub description: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Commit {
    pub id: String,
    #[serde(default)]
    pub user_id: Option<String>,
    #[serde(default)]
    pub short_hash: Option<String>,
    #[serde(default)]
    pub body: Option<String>,
    #[serde(default)]
    pub parent_id: Option<String>,
    #[serde(default)]
    pub html_url: Option<String>,
    #[serde(default, deserialize_with = "deserialize_opt_datetime")]
    pub created_at: Option<DateTime<Utc>>,
    #[serde(default)]
    pub stars_count: Option<i64>,
    #[serde(default)]
    pub replies_count: Option<i64>,
    #[serde(default)]
    pub link_preview: Option<LinkPreview>,
    #[serde(default)]
    pub images: Vec<CommitImage>,
    #[serde(default, alias = "user")]
    pub author: Option<User>,
    #[serde(default)]
    pub status: Option<String>,
    #[serde(default)]
    pub starred: Option<bool>,
    #[serde(default)]
    pub is_pinned: Option<bool>,
    #[serde(default)]
    pub pasteis_count: Option<i64>,
    #[serde(default, deserialize_with = "deserialize_opt_datetime")]
    pub deleted_at: Option<DateTime<Utc>>,
    #[serde(default)]
    pub quoted_commit_id: Option<String>,
    #[serde(default)]
    pub quoted_commit: Option<Box<Commit>>,
    #[serde(default)]
    pub parent: Option<Box<Commit>>,
}

impl Commit {
    pub fn is_deleted(&self) -> bool {
        self.deleted_at.is_some() && self.body.is_none()
    }

    pub fn author_id(&self) -> Option<&str> {
        self.user_id
            .as_deref()
            .or_else(|| self.author.as_ref().map(|a| a.id.as_str()))
    }

    pub fn is_own(&self, me: Option<&str>) -> bool {
        match (self.author_id(), me) {
            (Some(id), Some(me)) => id == me,
            _ => false,
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct CommitPage {
    pub commit: Commit,
    pub ancestors: Vec<Commit>,
    pub series: Vec<Commit>,
}

impl CommitPage {
    pub fn from_value(value: serde_json::Value) -> Result<Self, serde_json::Error> {
        let data = value.get("data").cloned().unwrap_or(value.clone());
        let commit_value = data.get("commit").cloned().unwrap_or_else(|| data.clone());
        let ancestors = value
            .get("ancestors")
            .or_else(|| data.get("ancestors"))
            .or_else(|| value.pointer("/meta/ancestors"))
            .cloned()
            .unwrap_or(serde_json::Value::Array(Vec::new()));
        let series = value
            .get("series")
            .or_else(|| data.get("series"))
            .cloned()
            .unwrap_or(serde_json::Value::Array(Vec::new()));
        Ok(Self {
            commit: serde_json::from_value(commit_value)?,
            ancestors: serde_json::from_value(ancestors).unwrap_or_default(),
            series: serde_json::from_value(series).unwrap_or_default(),
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct PastelAward {
    #[serde(default)]
    pub pasteis_count: i64,
    #[serde(default)]
    pub remaining: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct CursorMeta {
    #[serde(default)]
    pub next_cursor: Option<String>,
    #[serde(default)]
    pub prev_cursor: Option<String>,
    #[serde(default)]
    pub per_page: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct CursorLinks {
    #[serde(default)]
    pub next: Option<String>,
    #[serde(default)]
    pub prev: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Page<T> {
    pub data: Vec<T>,
    #[serde(default)]
    pub links: Option<CursorLinks>,
    #[serde(default)]
    pub meta: Option<CursorMeta>,
}

impl<T> Page<T> {
    pub fn next_cursor(&self) -> Option<String> {
        self.meta
            .as_ref()
            .and_then(|m| m.next_cursor.clone())
            .or_else(|| {
                self.links.as_ref().and_then(|l| {
                    l.next.as_ref().and_then(|url| {
                        url::Url::parse(url).ok().and_then(|u| {
                            u.query_pairs()
                                .find(|(k, _)| k == "cursor")
                                .map(|(_, v)| v.into_owned())
                        })
                    })
                })
            })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DataEnvelope<T> {
    pub data: T,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogPage {
    pub data: Vec<Commit>,
    #[serde(default)]
    pub links: Option<CursorLinks>,
    #[serde(default)]
    pub meta: Option<CursorMeta>,
    #[serde(default)]
    pub new_commits_count: Option<i64>,
}

impl LogPage {
    pub fn next_cursor(&self) -> Option<String> {
        self.meta
            .as_ref()
            .and_then(|m| m.next_cursor.clone())
            .or_else(|| {
                self.links.as_ref().and_then(|l| {
                    l.next.as_ref().and_then(|url| {
                        url::Url::parse(url).ok().and_then(|u| {
                            u.query_pairs()
                                .find(|(k, _)| k == "cursor")
                                .map(|(_, v)| v.into_owned())
                        })
                    })
                })
            })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LogKind {
    Following,
    Latest,
    Trending,
}

impl LogKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Following => "following",
            Self::Latest => "latest",
            Self::Trending => "trending",
        }
    }

    pub fn title(self) -> &'static str {
        match self {
            Self::Following => "Following",
            Self::Latest => "Latest",
            Self::Trending => "Trending",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Notification {
    pub id: String,
    #[serde(default)]
    pub type_name: Option<String>,
    #[serde(rename = "type", default)]
    pub kind: Option<String>,
    #[serde(default)]
    pub subject_id: Option<String>,
    #[serde(default)]
    pub reason: Option<String>,
    #[serde(default, deserialize_with = "deserialize_opt_datetime")]
    pub read_at: Option<DateTime<Utc>>,
    #[serde(default, deserialize_with = "deserialize_opt_datetime")]
    pub created_at: Option<DateTime<Utc>>,
    #[serde(default)]
    pub actor: Option<User>,
    #[serde(default)]
    pub actors: Vec<User>,
    #[serde(default)]
    pub group_id: Option<String>,
    #[serde(default)]
    pub event_count: Option<i64>,
    #[serde(default)]
    pub actors_count: Option<i64>,
    #[serde(default)]
    pub unread_count: Option<i64>,
    #[serde(default)]
    pub commit: Option<Commit>,
}

impl Notification {
    pub fn title(&self) -> String {
        let actor = self
            .actor
            .as_ref()
            .map(User::display_name)
            .unwrap_or("someone");
        let kind = self
            .kind
            .as_deref()
            .or(self.reason.as_deref())
            .unwrap_or("");
        match kind {
            "follow" | "followed" => format!("{actor} followed you"),
            "star" | "starred" => format!("{actor} starred your commit"),
            "reply" | "replied" => format!("{actor} replied to your commit"),
            "quote" | "quoted" => format!("{actor} quoted your commit"),
            "mention" | "mentioned" => format!("{actor} mentioned you"),
            "approved" => "Your commit was approved.".into(),
            "rejected" => "Your commit was rejected.".into(),
            other if !other.is_empty() => format!("{actor}: {other}"),
            _ => format!("{actor} notified you"),
        }
    }

    pub fn kind(&self) -> &str {
        self.kind
            .as_deref()
            .or(self.type_name.as_deref())
            .or(self.reason.as_deref())
            .unwrap_or("")
    }

    pub fn is_unread(&self) -> bool {
        self.read_at.is_none() || self.unread_count.unwrap_or(0) > 0
    }
}

pub fn unread_counts_from_value(value: &serde_json::Value) -> (Option<i64>, Option<i64>) {
    let root = value
        .get("data")
        .filter(|data| data.is_object())
        .unwrap_or(value);
    let auth = root
        .get("auth")
        .filter(|data| data.is_object())
        .unwrap_or(root);
    let notifications = json_count(
        auth,
        &["unread_notifications_count", "unreadNotificationsCount"],
    )
    .or_else(|| {
        json_count(
            root,
            &["unread_notifications_count", "unreadNotificationsCount"],
        )
    });
    let remotes = json_count(auth, &["unread_messages_count", "unreadMessagesCount"])
        .or_else(|| json_count(root, &["unread_messages_count", "unreadMessagesCount"]))
        .or_else(|| {
            root.get("remotes")
                .and_then(|remotes| json_count(remotes, &["unread_count", "unreadCount"]))
        });
    (notifications, remotes)
}

fn json_count(value: &serde_json::Value, names: &[&str]) -> Option<i64> {
    names.iter().find_map(|name| {
        value.get(*name).and_then(|item| {
            item.as_i64()
                .or_else(|| item.as_u64().map(|n| n as i64))
                .or_else(|| item.as_f64().map(|n| n as i64))
        })
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum NotificationFilter {
    #[default]
    All,
    Replies,
    Quotes,
    Stars,
    Mentions,
    Follows,
    Moderation,
}

impl NotificationFilter {
    pub const ALL: [Self; 7] = [
        Self::All,
        Self::Replies,
        Self::Quotes,
        Self::Stars,
        Self::Mentions,
        Self::Follows,
        Self::Moderation,
    ];

    pub fn as_str(self) -> &'static str {
        match self {
            Self::All => "all",
            Self::Replies => "replies",
            Self::Quotes => "quotes",
            Self::Stars => "stars",
            Self::Mentions => "mentions",
            Self::Follows => "follows",
            Self::Moderation => "moderation",
        }
    }

    pub fn query(self) -> Option<&'static str> {
        match self {
            Self::All => None,
            other => Some(other.as_str()),
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::All => "All",
            Self::Replies => "Replies",
            Self::Quotes => "Quotes",
            Self::Stars => "Stars",
            Self::Mentions => "Mentions",
            Self::Follows => "Follows",
            Self::Moderation => "Moderation",
        }
    }

    pub fn empty_fatal(self) -> String {
        match self {
            Self::All => "$ cat /dev/notifications".into(),
            other => format!("$ cat /dev/notifications | grep {}", other.as_str()),
        }
    }

    pub fn empty_detail(self) -> &'static str {
        match self {
            Self::All => {
                "Nothing here yet. Follows, stars, replies, quotes, mentions, and moderation updates for your commits will show up here."
            }
            Self::Replies => {
                "No replies yet. When someone replies to one of your commits, it shows up here."
            }
            Self::Quotes => {
                "No quotes yet. When someone quotes one of your commits, it shows up here."
            }
            Self::Stars => {
                "No stars yet. When someone stars one of your commits, it shows up here."
            }
            Self::Mentions => {
                "No mentions yet. When someone mentions you in a commit, it shows up here."
            }
            Self::Follows => "No followers yet. When someone follows you, it shows up here.",
            Self::Moderation => {
                "No moderation updates. Approved and rejected commits show up here."
            }
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum ConversationKind {
    #[default]
    Direct,
    Org,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Member {
    pub user: User,
    #[serde(default)]
    pub role: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct MessageImage {
    pub id: String,
    #[serde(default)]
    pub width: Option<i32>,
    #[serde(default)]
    pub height: Option<i32>,
    #[serde(default)]
    pub description: Option<String>,
    pub url: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Message {
    pub id: i64,
    #[serde(default)]
    pub user_id: Option<String>,
    #[serde(default)]
    pub client_id: Option<String>,
    #[serde(default)]
    pub body: Option<String>,
    #[serde(default)]
    pub reply_to_id: Option<i64>,
    #[serde(default)]
    pub link_preview: Option<LinkPreview>,
    #[serde(default)]
    pub images: Vec<MessageImage>,
    #[serde(default)]
    pub event: Option<String>,
    #[serde(default)]
    pub user: Option<User>,
    #[serde(default, deserialize_with = "deserialize_opt_datetime")]
    pub created_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ConversationSummary {
    pub id: String,
    #[serde(default)]
    pub kind: ConversationKind,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub partner: Option<User>,
    #[serde(default)]
    pub members: Vec<Member>,
    #[serde(default)]
    pub last_message: Option<Message>,
    #[serde(default, deserialize_with = "deserialize_opt_datetime")]
    pub last_message_at: Option<DateTime<Utc>>,
    #[serde(default)]
    pub ahead: i64,
    #[serde(default)]
    pub last_incoming_message_id: Option<i64>,
    #[serde(default)]
    pub is_archived: bool,
    #[serde(default, deserialize_with = "deserialize_opt_datetime")]
    pub pinned_at: Option<DateTime<Utc>>,
    #[serde(default, deserialize_with = "deserialize_opt_datetime")]
    pub muted_until_at: Option<DateTime<Utc>>,
}

impl ConversationSummary {
    pub fn title(&self) -> String {
        if self.kind == ConversationKind::Org {
            self.name.clone().unwrap_or_else(|| "org".into())
        } else {
            self.partner
                .as_ref()
                .map(|u| u.display_name().to_owned())
                .unwrap_or_else(|| "remote".into())
        }
    }

    pub fn preview(&self) -> String {
        self.last_message
            .as_ref()
            .and_then(|m| m.body.clone())
            .unwrap_or_default()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct InboxPage {
    #[serde(default)]
    pub data: Vec<ConversationSummary>,
    #[serde(default)]
    pub has_more: bool,
    #[serde(default)]
    pub unread_count: i64,
    #[serde(default)]
    pub requests_count: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct OrgInfo {
    pub id: String,
    #[serde(default)]
    pub name: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ChatThread {
    #[serde(default)]
    pub messages: Vec<Message>,
    #[serde(default)]
    pub has_older: bool,
    #[serde(default)]
    pub read_through_id: i64,
    #[serde(default)]
    pub is_archived: bool,
    #[serde(default)]
    pub access: Option<String>,
    #[serde(default)]
    pub partner: Option<User>,
    #[serde(default)]
    pub org: Option<OrgInfo>,
    #[serde(default)]
    pub members: Vec<Member>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ThreadRef {
    Direct { username: String },
    Org { id: String, name: Option<String> },
}

impl ThreadRef {
    pub fn from_summary(summary: &ConversationSummary) -> Self {
        match &summary.partner {
            Some(partner) if summary.kind != ConversationKind::Org => Self::Direct {
                username: partner.username.clone(),
            },
            _ => Self::Org {
                id: summary.id.clone(),
                name: summary.name.clone(),
            },
        }
    }

    pub fn title(&self) -> String {
        match self {
            Self::Direct { username } => format!("@{username}"),
            Self::Org { name, id } => name.clone().unwrap_or_else(|| id.clone()),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InboxTab {
    Open,
    Fetched,
    Stashed,
}

impl InboxTab {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Open => "open",
            Self::Fetched => "fetched",
            Self::Stashed => "stashed",
        }
    }
}

#[cfg(test)]
mod unread_count_tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn reads_snake_case_counts_from_me() {
        let value = json!({
            "id": "1",
            "username": "nuno",
            "unread_notifications_count": 4,
            "remotes": { "from": "followers", "unread_count": 2 }
        });
        assert_eq!(unread_counts_from_value(&value), (Some(4), Some(2)));
    }

    #[test]
    fn reads_camel_case_counts_from_a_wrapped_payload() {
        let value = json!({
            "data": {
                "unreadNotificationsCount": 12,
                "unreadMessagesCount": 99
            }
        });
        assert_eq!(unread_counts_from_value(&value), (Some(12), Some(99)));
    }

    #[test]
    fn treats_a_group_with_unread_items_as_unread() {
        let notice = Notification {
            read_at: Some(Utc::now()),
            unread_count: Some(2),
            ..Default::default()
        };
        assert!(notice.is_unread());
    }
}
