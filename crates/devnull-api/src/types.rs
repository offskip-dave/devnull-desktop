use std::collections::HashMap;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Deserializer, Serialize};

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
    #[serde(alias = "handle")]
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
    pub has_github_account: Option<bool>,
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
    #[serde(default, alias = "is_following")]
    pub is_followed: Option<bool>,
    #[serde(default)]
    pub follows_you: Option<bool>,
    #[serde(default)]
    pub has_sponsors_listing: Option<bool>,
    #[serde(default)]
    pub timezone: Option<String>,
    #[serde(default, deserialize_with = "deserialize_opt_live")]
    pub live: Option<LiveStream>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
pub struct LiveStream {
    #[serde(default)]
    pub url: String,
    #[serde(default)]
    pub thumbnail_url: Option<String>,
}

fn deserialize_opt_live<'de, D>(deserializer: D) -> Result<Option<LiveStream>, D::Error>
where
    D: Deserializer<'de>,
{
    let value = Option::<serde_json::Value>::deserialize(deserializer)?;
    Ok(match value {
        None | Some(serde_json::Value::Null) => None,
        Some(serde_json::Value::String(url)) => {
            let url = url.trim().to_owned();
            if url.is_empty() {
                None
            } else {
                Some(LiveStream {
                    url,
                    thumbnail_url: None,
                })
            }
        }
        Some(value) => serde_json::from_value::<LiveStream>(value)
            .ok()
            .filter(|stream| !stream.url.is_empty()),
    })
}

impl User {
    pub fn display_name(&self) -> &str {
        match &self.name {
            Some(name) if !name.trim().is_empty() => name,
            _ => &self.username,
        }
    }

    pub fn live_url(&self) -> Option<&str> {
        self.live
            .as_ref()
            .map(|stream| stream.url.as_str())
            .filter(|url| !url.is_empty())
    }

    pub fn is_live(&self) -> bool {
        self.live_url().is_some()
    }

    pub fn github_login(&self) -> &str {
        self.github_url
            .as_deref()
            .and_then(github_login_from_url)
            .unwrap_or(&self.username)
    }

    pub fn is_followed_by_me(&self) -> bool {
        self.is_followed.unwrap_or(false)
    }
}

fn github_login_from_url(url: &str) -> Option<&str> {
    let url = url.trim().trim_end_matches('/');
    let rest = url
        .strip_prefix("https://github.com/")
        .or_else(|| url.strip_prefix("http://github.com/"))
        .or_else(|| url.strip_prefix("https://www.github.com/"))
        .or_else(|| url.strip_prefix("http://www.github.com/"))?;
    let login = rest.split('/').next()?;
    (!login.is_empty()).then_some(login)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RemotesSettings {
    #[serde(default)]
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
        for key in [
            "liveFollowing",
            "live_following",
            "muted",
            "blocked",
            "mutes",
            "blocks",
        ] {
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
        self.unlocked_badges().count()
    }

    pub fn unlocked_badges(&self) -> impl Iterator<Item = &Badge> {
        self.badges.iter().filter(|badge| badge.unlocked)
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

fn deserialize_mentions<'de, D>(
    deserializer: D,
) -> Result<Option<HashMap<String, String>>, D::Error>
where
    D: Deserializer<'de>,
{
    let value = Option::<serde_json::Value>::deserialize(deserializer)?;
    Ok(match value {
        None | Some(serde_json::Value::Null) => None,
        Some(serde_json::Value::Object(map)) => {
            let mut out = HashMap::new();
            for (key, item) in map {
                if let Some(id) = item.as_str() {
                    out.insert(key, id.to_owned());
                }
            }
            Some(out)
        }
        Some(_) => None,
    })
}

fn deserialize_opt_kind<'de, D>(deserializer: D) -> Result<Option<String>, D::Error>
where
    D: Deserializer<'de>,
{
    let value = Option::<serde_json::Value>::deserialize(deserializer)?;
    Ok(value.and_then(|value| match value {
        serde_json::Value::String(kind) => Some(kind),
        serde_json::Value::Object(map) => map
            .get("value")
            .or_else(|| map.get("name"))
            .and_then(serde_json::Value::as_str)
            .map(str::to_owned),
        _ => None,
    }))
}

fn deserialize_vec<'de, D, T>(deserializer: D) -> Result<Vec<T>, D::Error>
where
    D: Deserializer<'de>,
    T: Deserialize<'de>,
{
    Ok(Option::<Vec<T>>::deserialize(deserializer)?.unwrap_or_default())
}

fn deserialize_opt_i64<'de, D>(deserializer: D) -> Result<Option<i64>, D::Error>
where
    D: Deserializer<'de>,
{
    let value = Option::<serde_json::Value>::deserialize(deserializer)?;
    Ok(value.and_then(|value| {
        value
            .as_i64()
            .or_else(|| value.as_u64().map(|n| n as i64))
            .or_else(|| value.as_f64().map(|n| n as i64))
            .or_else(|| value.as_str().and_then(|s| s.parse().ok()))
    }))
}

fn deserialize_opt_commit<'de, D>(deserializer: D) -> Result<Option<Commit>, D::Error>
where
    D: Deserializer<'de>,
{
    let value = Option::<serde_json::Value>::deserialize(deserializer)?;
    Ok(value.and_then(|value| {
        if value.is_null() {
            None
        } else {
            serde_json::from_value(value).ok()
        }
    }))
}

fn json_list<'a>(value: &'a serde_json::Value) -> Option<&'a Vec<serde_json::Value>> {
    value.as_array().or_else(|| {
        value
            .get("data")
            .and_then(serde_json::Value::as_array)
            .or_else(|| value.get("notifications").and_then(json_list))
    })
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
    /// Captured `@username` → user id, lowercased keys. Only these mentions are linkified.
    #[serde(default, deserialize_with = "deserialize_mentions")]
    pub mentions: Option<HashMap<String, String>>,
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
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub type_name: Option<String>,
    #[serde(rename = "type", default, deserialize_with = "deserialize_opt_kind")]
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
    #[serde(default, deserialize_with = "deserialize_vec")]
    pub actors: Vec<User>,
    #[serde(default)]
    pub group_id: Option<String>,
    #[serde(default, deserialize_with = "deserialize_opt_i64")]
    pub event_count: Option<i64>,
    #[serde(default, deserialize_with = "deserialize_opt_i64")]
    pub actors_count: Option<i64>,
    #[serde(default, deserialize_with = "deserialize_opt_i64")]
    pub unread_count: Option<i64>,
    #[serde(default, deserialize_with = "deserialize_opt_commit")]
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

    pub fn page_from_value(value: serde_json::Value) -> Page<Self> {
        let root = value.get("data").unwrap_or(&value);
        let items = json_list(&value)
            .or_else(|| json_list(root))
            .cloned()
            .unwrap_or_default();
        let data = items
            .into_iter()
            .filter_map(|item| serde_json::from_value(item).ok())
            .collect();
        let links =
            serde_json::from_value(nested(&value, root, "links").cloned().unwrap_or_default()).ok();
        let meta =
            serde_json::from_value(nested(&value, root, "meta").cloned().unwrap_or_default()).ok();
        Page { data, links, meta }
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
    /// Captured `@username` → user id, lowercased keys. Only these mentions are linkified.
    #[serde(default, deserialize_with = "deserialize_mentions")]
    pub mentions: Option<HashMap<String, String>>,
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

    #[test]
    fn current_user_reads_handle_the_way_devices_and_me_send_it() {
        let value = json!({
            "id": "01a13fa8-6cd4-70ef-b8e5-f46a3ca70ca8",
            "handle": "nunomaduro",
            "name": "Nuno Maduro",
            "avatar_url": "https://avatars.githubusercontent.com/u/5457236?v=4",
            "created_at": "2026-10-15T13:02:44Z",
            "commits_count": 355,
            "lane": 3,
            "pasteis_mode": false,
            "pasteis_remaining": 10,
            "pasteis_received": 0,
            "preferred_lane": null,
            "automatic_lane": 3,
            "sponsors_url": null,
            "github_synced_at": null,
            "terms_version": "2026-10-01",
            "terms_accepted": true,
            "unread_notifications_count": 4,
            "remotes": {
                "from": "following",
                "pull_receipts": true,
                "requests": true,
                "everyone": false,
                "unread_count": 2
            }
        });
        let user: CurrentUser = serde_json::from_value(value).unwrap();
        assert_eq!(user.username(), "nunomaduro");
        assert_eq!(user.unread_notifications(), 4);
        assert!(user.remotes_on());
        assert_eq!(user.unread_remotes(), 2);
    }

    #[test]
    fn user_still_reads_the_older_username_field() {
        let user: User = serde_json::from_value(json!({
            "id": "1",
            "username": "local_dev"
        }))
        .unwrap();
        assert_eq!(user.username, "local_dev");
    }

    #[test]
    fn user_reads_a_live_stream_object() {
        let user: User = serde_json::from_value(json!({
            "id": "1",
            "handle": "nuno",
            "live": {
                "url": "https://www.youtube.com/watch?v=dQw4w9WgXcQ",
                "thumbnail_url": "https://i.ytimg.com/vi/dQw4w9WgXcQ/hqdefault_live.jpg"
            }
        }))
        .unwrap();
        assert_eq!(user.username, "nuno");
        assert_eq!(
            user.live_url(),
            Some("https://www.youtube.com/watch?v=dQw4w9WgXcQ")
        );
    }

    #[test]
    fn live_following_list_reads_wrapped_users() {
        let users = User::list_from_value(json!({
            "data": [{
                "id": "1",
                "handle": "ada",
                "name": "Ada",
                "live": { "url": "https://www.twitch.tv/ada" }
            }]
        }));
        assert_eq!(users.len(), 1);
        assert!(users[0].is_live());
    }

    #[test]
    fn github_login_comes_from_github_url_when_present() {
        let user = User {
            username: "ada".into(),
            github_url: Some("https://github.com/AdaLovelace/".into()),
            ..Default::default()
        };
        assert_eq!(user.github_login(), "AdaLovelace");
        let fallback = User {
            username: "ada".into(),
            ..Default::default()
        };
        assert_eq!(fallback.github_login(), "ada");
    }

    #[test]
    fn is_followed_reads_is_following_alias() {
        let user: User = serde_json::from_value(json!({
            "id": "1",
            "handle": "ada",
            "is_following": true
        }))
        .unwrap();
        assert!(user.is_followed_by_me());
    }

    fn badge(id: &str, unlocked: bool) -> Badge {
        Badge {
            id: id.into(),
            name: id.into(),
            unlocked,
            progress: if unlocked { 1 } else { 0 },
            target: 1,
            ..Default::default()
        }
    }

    #[test]
    fn shelf_only_counts_unlocked_badges() {
        let achievements = Achievements {
            level: 4,
            rank: "maintainer".into(),
            xp: 916,
            next_level_xp: Some(1000),
            level_progress: 84,
            ranks: vec![Rank {
                rank: "wheel".into(),
                level: 5,
                min_xp: 1000,
                ..Default::default()
            }],
            badges: vec![
                badge("init", true),
                badge("syntax", true),
                badge("uptime", false),
                badge("luddite", false),
            ],
        };
        assert_eq!(achievements.label(), "Lv. 4 · maintainer");
        assert_eq!(achievements.xp_label(), "916 / 1000 XP");
        assert_eq!(achievements.next_rank_label(), "84 XP to wheel");
        assert_eq!(achievements.unlocked_count(), 2);
        let names: Vec<&str> = achievements
            .unlocked_badges()
            .map(|badge| badge.name.as_str())
            .collect();
        assert_eq!(names, ["init", "syntax"]);
    }

    #[test]
    fn max_rank_has_no_next_level() {
        let achievements = Achievements {
            level: 0,
            rank: "root".into(),
            xp: 9001,
            next_level_xp: None,
            ..Default::default()
        };
        assert_eq!(achievements.xp_label(), "9001 XP");
        assert_eq!(achievements.next_rank_label(), "Maximum rank (UID 0)");
        assert_eq!(achievements.unlocked_count(), 0);
    }
}
