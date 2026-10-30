use std::collections::{HashMap, HashSet};

use chrono::{DateTime, Utc};
use serde::{Deserialize, Deserializer, Serialize};

use crate::date::{deserialize_opt_datetime, parse_date};

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
    #[serde(default, deserialize_with = "deserialize_stringish")]
    pub id: String,
    #[serde(default, alias = "handle")]
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
    #[serde(
        default,
        alias = "is_following",
        deserialize_with = "deserialize_opt_boolish"
    )]
    pub is_followed: Option<bool>,
    #[serde(default)]
    pub follows_you: Option<bool>,
    #[serde(default)]
    pub has_sponsors_listing: Option<bool>,
    #[serde(default)]
    pub timezone: Option<String>,
    #[serde(default)]
    pub followed_by_following_count: Option<i64>,
    #[serde(default, deserialize_with = "deserialize_opt_live")]
    pub live: Option<LiveStream>,
}

impl User {
    pub fn suggestion_reason(&self) -> Option<String> {
        if let Some(count) = self.followed_by_following_count.filter(|count| *count > 0) {
            return Some(format!("Followed by {count} you follow"));
        }
        match self.followers_count.unwrap_or(0) {
            0 => None,
            1 => Some("1 follower".into()),
            count => Some(format!("{count} followers")),
        }
    }

    pub fn bio_line(&self) -> Option<&str> {
        self.bio
            .as_deref()
            .map(str::trim)
            .filter(|bio| !bio.is_empty())
            .and_then(|bio| bio.lines().map(str::trim).find(|line| !line.is_empty()))
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
pub struct LiveStream {
    #[serde(default)]
    pub url: String,
    #[serde(default)]
    pub thumbnail_url: Option<String>,
    #[serde(default)]
    pub platform: Option<String>,
    #[serde(default, deserialize_with = "deserialize_opt_datetime")]
    pub ends_at: Option<DateTime<Utc>>,
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
                    ..Default::default()
                })
            }
        }
        Some(value) => serde_json::from_value::<LiveStream>(value)
            .ok()
            .filter(|stream| !stream.url.is_empty()),
    })
}

impl LiveStream {
    pub fn is_current(&self) -> bool {
        match self.ends_at {
            Some(ends_at) => ends_at > Utc::now(),
            None => !self.url.is_empty(),
        }
    }
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
            .filter(|stream| stream.is_current())
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

    pub fn is_already_followed(&self, following: &HashSet<String>) -> bool {
        self.is_followed_by_me() || following.contains(&self.username.trim().to_ascii_lowercase())
    }

    pub fn following_names(users: &[Self]) -> HashSet<String> {
        users
            .iter()
            .map(|user| user.username.trim().to_ascii_lowercase())
            .filter(|name| !name.is_empty())
            .collect()
    }

    pub fn not_followed(users: Vec<Self>, following: &HashSet<String>) -> Vec<Self> {
        users
            .into_iter()
            .filter(|user| !user.is_already_followed(following))
            .collect()
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

/// A GitHub organization that commits can mention, as `/api/v1/organizations/{handle}` returns it.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Organization {
    #[serde(default, alias = "username")]
    pub handle: String,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub avatar_url: Option<String>,
    #[serde(default)]
    pub bio: Option<String>,
    #[serde(default)]
    pub location: Option<String>,
    #[serde(default)]
    pub website: Option<String>,
    #[serde(default)]
    pub github_url: Option<String>,
    #[serde(default)]
    pub html_url: Option<String>,
    #[serde(default)]
    pub lane: Option<Lane>,
    #[serde(default, deserialize_with = "deserialize_vec")]
    pub admins: Vec<User>,
}

impl Organization {
    pub fn display_name(&self) -> &str {
        match &self.name {
            Some(name) if !name.trim().is_empty() => name,
            _ => &self.handle,
        }
    }

    pub fn html_url(&self, base: &str) -> String {
        self.html_url
            .clone()
            .unwrap_or_else(|| format!("{}/@{}", base.trim_end_matches('/'), self.handle))
    }

    pub fn github_url(&self) -> String {
        self.github_url.clone().unwrap_or_else(|| {
            if self.handle.is_empty() {
                String::new()
            } else {
                format!("https://github.com/{}", self.handle)
            }
        })
    }

    pub fn from_value(value: serde_json::Value) -> Result<Self, serde_json::Error> {
        let data = value.get("data").cloned().unwrap_or(value.clone());
        let mut organization: Organization = serde_json::from_value(data.clone())?;
        if organization.handle.trim().is_empty() {
            if let Some(handle) = data
                .get("username")
                .and_then(serde_json::Value::as_str)
                .or_else(|| value.get("handle").and_then(serde_json::Value::as_str))
            {
                organization.handle = handle.to_owned();
            }
        }
        if organization.lane.is_none() {
            organization.lane = value
                .get("lane")
                .cloned()
                .or_else(|| data.get("lane").cloned())
                .and_then(|lane| serde_json::from_value(lane).ok());
        }
        if organization.admins.is_empty() {
            let admins = value
                .get("admins")
                .cloned()
                .or_else(|| data.get("admins").cloned())
                .unwrap_or(serde_json::Value::Null);
            organization.admins = User::list_from_value(admins);
        }
        Ok(organization)
    }

    pub fn list_from_value(value: &serde_json::Value) -> Vec<Self> {
        let data = value.get("data").unwrap_or(value);
        let items = data
            .as_array()
            .or_else(|| data.get("organizations").and_then(serde_json::Value::as_array))
            .or_else(|| value.get("organizations").and_then(|orgs| {
                orgs.as_array()
                    .or_else(|| orgs.get("data").and_then(serde_json::Value::as_array))
            }));
        items
            .map(|items| {
                items
                    .iter()
                    .filter_map(|item| serde_json::from_value::<Organization>(item.clone()).ok())
                    .filter(|organization| !organization.handle.trim().is_empty())
                    .collect()
            })
            .unwrap_or_default()
    }
}

/// Composer `@` suggestions: developers first, then known organizations.
#[derive(Debug, Clone, Default)]
pub struct MentionSuggestions {
    pub users: Vec<User>,
    pub organizations: Vec<Organization>,
}

impl MentionSuggestions {
    pub fn from_value(value: serde_json::Value) -> Self {
        let organizations = value
            .get("organizations")
            .map(Organization::list_from_value)
            .unwrap_or_default();
        Self {
            users: User::list_from_value(value),
            organizations,
        }
    }

    pub fn is_empty(&self) -> bool {
        self.users.is_empty() && self.organizations.is_empty()
    }
}

/// Captured `@handle`s in a commit: developers and GitHub organizations, from either
/// handle→id maps or the signed API's mention positions array.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct MentionMaps {
    pub users: Option<HashMap<String, String>>,
    pub organizations: Option<HashMap<String, String>>,
}

impl MentionMaps {
    fn from_values(
        mentions: Option<&serde_json::Value>,
        organization_mentions: Option<&serde_json::Value>,
    ) -> Self {
        let (users, orgs_in_mentions) = split_mention_value(mentions, false);
        let (_, orgs) = split_mention_value(organization_mentions, true);
        Self {
            users,
            organizations: merge_mention_maps(orgs_in_mentions, orgs),
        }
    }
}

impl Serialize for MentionMaps {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        use serde::ser::SerializeMap;
        let mut map = serializer.serialize_map(Some(2))?;
        map.serialize_entry("mentions", &self.users)?;
        map.serialize_entry("organization_mentions", &self.organizations)?;
        map.end()
    }
}

impl<'de> Deserialize<'de> for MentionMaps {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        #[derive(Deserialize)]
        struct Raw {
            #[serde(default)]
            mentions: Option<serde_json::Value>,
            #[serde(default)]
            organization_mentions: Option<serde_json::Value>,
        }
        let raw = Raw::deserialize(deserializer)?;
        Ok(Self::from_values(
            raw.mentions.as_ref(),
            raw.organization_mentions.as_ref(),
        ))
    }
}

fn split_mention_value(
    value: Option<&serde_json::Value>,
    object_is_org: bool,
) -> (Option<HashMap<String, String>>, Option<HashMap<String, String>>) {
    match value {
        None | Some(serde_json::Value::Null) => (None, None),
        Some(serde_json::Value::Object(map)) => {
            let parsed = map_from_mention_object(map);
            if object_is_org {
                (None, Some(parsed))
            } else {
                (Some(parsed), None)
            }
        }
        Some(serde_json::Value::Array(items)) if items.is_empty() => (None, None),
        Some(serde_json::Value::Array(items)) => {
            let mut users = HashMap::new();
            let mut organizations = HashMap::new();
            for item in items {
                let Some(handle) = mention_handle(item) else {
                    continue;
                };
                let id = mention_id(item);
                if mention_is_organization(item) {
                    organizations.insert(handle, id);
                } else {
                    users.insert(handle, id);
                }
            }
            (Some(users), Some(organizations).filter(|map| !map.is_empty()))
        }
        Some(_) => (None, None),
    }
}

fn merge_mention_maps(
    left: Option<HashMap<String, String>>,
    right: Option<HashMap<String, String>>,
) -> Option<HashMap<String, String>> {
    match (left, right) {
        (None, right) => right,
        (left, None) => left,
        (Some(mut left), Some(right)) => {
            for (handle, id) in right {
                left.entry(handle).or_insert(id);
            }
            Some(left)
        }
    }
}

fn map_from_mention_object(map: &serde_json::Map<String, serde_json::Value>) -> HashMap<String, String> {
    let mut out = HashMap::new();
    for (key, item) in map {
        if let Some(id) = item.as_str() {
            out.insert(key.to_ascii_lowercase(), id.to_owned());
        } else if let Some(id) = mention_id_opt(item) {
            out.insert(key.to_ascii_lowercase(), id);
        }
    }
    out
}

fn mention_handle(item: &serde_json::Value) -> Option<String> {
    item.get("handle")
        .or_else(|| item.get("username"))
        .and_then(serde_json::Value::as_str)
        .map(|handle| handle.trim().to_ascii_lowercase())
        .filter(|handle| !handle.is_empty())
}

fn mention_id(item: &serde_json::Value) -> String {
    mention_id_opt(item).unwrap_or_default()
}

fn mention_id_opt(item: &serde_json::Value) -> Option<String> {
    item.get("id")
        .and_then(|value| {
            value
                .as_str()
                .map(str::to_owned)
                .or_else(|| value.as_i64().map(|id| id.to_string()))
                .or_else(|| value.as_u64().map(|id| id.to_string()))
        })
        .filter(|id| !id.is_empty())
}

fn mention_is_organization(item: &serde_json::Value) -> bool {
    item.get("kind")
        .and_then(serde_json::Value::as_str)
        .is_some_and(|kind| kind.eq_ignore_ascii_case("organization"))
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserSettings {
    #[serde(default)]
    pub timezone: Option<String>,
    #[serde(default)]
    pub show_local_time: bool,
    #[serde(default, alias = "translationLanguage")]
    pub translation_language: Option<String>,
    #[serde(default = "default_true", alias = "automaticTranslation")]
    pub automatic_translation: bool,
    #[serde(default, alias = "translationLanguages")]
    pub translation_languages: Vec<TranslationLanguage>,
}

impl Default for UserSettings {
    fn default() -> Self {
        Self {
            timezone: None,
            show_local_time: false,
            translation_language: None,
            automatic_translation: true,
            translation_languages: Vec::new(),
        }
    }
}

fn default_true() -> bool {
    true
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
pub struct TranslationLanguage {
    #[serde(default)]
    pub code: String,
    #[serde(default)]
    pub name: String,
    #[serde(default, alias = "native_name", alias = "nativeName")]
    pub native_name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct CommitTranslation {
    #[serde(default)]
    pub language: String,
    #[serde(default)]
    pub source_language: Option<String>,
    #[serde(default)]
    pub source_name: Option<String>,
    #[serde(default)]
    pub available: bool,
    #[serde(default)]
    pub body: Option<String>,
}

impl CommitTranslation {
    pub fn from_value(value: serde_json::Value) -> Self {
        let root = value
            .get("translation")
            .or_else(|| value.get("data").and_then(|data| data.get("translation")))
            .or_else(|| value.get("data"))
            .unwrap_or(&value);
        let body = root
            .get("translated")
            .and_then(segments_text)
            .or_else(|| {
                root.get("translated")
                    .and_then(serde_json::Value::as_str)
                    .map(str::to_owned)
            });
        Self {
            language: root
                .get("language")
                .and_then(serde_json::Value::as_str)
                .unwrap_or_default()
                .to_owned(),
            source_language: root
                .get("source_language")
                .or_else(|| root.get("sourceLanguage"))
                .and_then(serde_json::Value::as_str)
                .map(str::to_owned),
            source_name: root
                .get("source_name")
                .or_else(|| root.get("sourceName"))
                .and_then(serde_json::Value::as_str)
                .map(str::to_owned),
            available: json_bool(root, "available").unwrap_or(false),
            body,
        }
    }

    pub fn shows_translation(&self) -> bool {
        self.available && self.body.as_ref().is_some_and(|body| !body.is_empty())
    }
}

fn segments_text(value: &serde_json::Value) -> Option<String> {
    let segments = value
        .get("body_segments")
        .or_else(|| value.get("bodySegments"))
        .and_then(serde_json::Value::as_array)?;
    let text = segments
        .iter()
        .filter_map(|segment| segment.get("text").and_then(serde_json::Value::as_str))
        .collect::<String>();
    if text.is_empty() { None } else { Some(text) }
}

impl User {
    pub fn list_from_value(value: serde_json::Value) -> Vec<Self> {
        let data = value.get("data").cloned().unwrap_or(value.clone());
        if let Some(users) = json_users(&data, "users").or_else(|| json_users(&value, "users")) {
            return users;
        }
        for key in [
            "suggestedUsers",
            "suggested_users",
            "suggested",
            "following",
            "followers",
            "liveFollowing",
            "live_following",
            "muted",
            "blocked",
            "mutes",
            "blocks",
        ] {
            if let Some(users) = json_users(&data, key).or_else(|| json_users(&value, key)) {
                return users;
            }
        }
        if let Ok(page) = serde_json::from_value::<Page<User>>(value.clone()) {
            if !page.data.is_empty() {
                return page.data;
            }
        }
        if let Some(users) = users_from_array(&data) {
            return users;
        }
        Vec::new()
    }

    /// Suggested-user payloads from Inertia/`suggestedUsers`, not a following `users` list.
    pub fn suggested_from_value(value: &serde_json::Value) -> Vec<Self> {
        let data = value.get("data").unwrap_or(value);
        let sources = [
            Some(data),
            Some(value),
            value.get("props"),
            data.get("props"),
        ];
        for source in sources.into_iter().flatten() {
            for key in ["suggestedUsers", "suggested_users", "suggested"] {
                if let Some(users) = json_users(source, key) {
                    return users;
                }
            }
        }
        Vec::new()
    }

    /// Unique commit authors who are not already followed. Used when the suggested payload is empty.
    pub fn unique_unfollowed_authors(commits: &[Commit], following: &HashSet<String>) -> Vec<Self> {
        let mut seen = HashSet::new();
        let mut users = Vec::new();
        for commit in commits {
            let Some(author) = commit.author.clone() else {
                continue;
            };
            let name = author.username.trim().to_ascii_lowercase();
            if name.is_empty() || !seen.insert(name) || author.is_already_followed(following) {
                continue;
            }
            users.push(author);
        }
        users
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

#[derive(Debug, Clone, Default)]
pub struct UserPreview {
    pub user: User,
    pub is_following: bool,
    pub follows_you: bool,
    pub is_muting: bool,
    pub is_blocking: bool,
    pub is_unavailable: bool,
}

impl UserPreview {
    pub fn from_profile(profile: Profile) -> Self {
        Self {
            is_following: profile.is_following,
            follows_you: profile.follows_you,
            is_muting: profile.is_muting,
            is_blocking: false,
            is_unavailable: false,
            user: profile.user,
        }
    }

    pub fn from_user(user: User) -> Self {
        let is_following = user.is_followed_by_me();
        let follows_you = user.follows_you.unwrap_or(false);
        Self {
            is_following,
            follows_you,
            is_muting: false,
            is_blocking: false,
            is_unavailable: false,
            user,
        }
    }

    pub fn username(&self) -> &str {
        &self.user.username
    }

    pub fn key(username: &str) -> String {
        username.trim().to_ascii_lowercase()
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
        let users = User::list_from_value(value.clone());
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

fn json_message_list(value: &serde_json::Value) -> Vec<Message> {
    let items = value
        .as_array()
        .or_else(|| value.get("data").and_then(serde_json::Value::as_array));
    items
        .map(|items| {
            items
                .iter()
                .filter_map(|item| serde_json::from_value(item.clone()).ok())
                .collect()
        })
        .unwrap_or_default()
}

fn json_users(value: &serde_json::Value, key: &str) -> Option<Vec<User>> {
    users_from_array(value.get(key)?)
}

fn users_from_array(value: &serde_json::Value) -> Option<Vec<User>> {
    let users: Vec<User> = value
        .as_array()?
        .iter()
        .filter_map(|item| serde_json::from_value::<User>(item.clone()).ok())
        .filter(|user| !user.username.trim().is_empty())
        .collect();
    (!users.is_empty()).then_some(users)
}

fn deserialize_opt_boolish<'de, D>(deserializer: D) -> Result<Option<bool>, D::Error>
where
    D: Deserializer<'de>,
{
    let value = Option::<serde_json::Value>::deserialize(deserializer)?;
    Ok(value.and_then(|value| match value {
        serde_json::Value::Null => None,
        serde_json::Value::Bool(flag) => Some(flag),
        serde_json::Value::Number(number) => number
            .as_i64()
            .or_else(|| number.as_u64().map(|n| n as i64))
            .or_else(|| number.as_f64().map(|n| n as i64))
            .map(|n| n != 0),
        serde_json::Value::String(text) => match text.trim().to_ascii_lowercase().as_str() {
            "true" | "1" | "yes" => Some(true),
            "false" | "0" | "no" | "" => Some(false),
            _ => None,
        },
        _ => None,
    }))
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
    T: serde::de::DeserializeOwned,
{
    let value = Option::<serde_json::Value>::deserialize(deserializer)?;
    Ok(match value {
        None | Some(serde_json::Value::Null) => Vec::new(),
        Some(serde_json::Value::Array(items)) => items
            .into_iter()
            .filter_map(|item| serde_json::from_value(item).ok())
            .collect(),
        Some(_) => Vec::new(),
    })
}

fn deserialize_reactions<'de, D>(deserializer: D) -> Result<Vec<MessageReaction>, D::Error>
where
    D: Deserializer<'de>,
{
    let value = Option::<serde_json::Value>::deserialize(deserializer)?;
    Ok(value
        .map(|value| MessageReaction::from_list(&value))
        .unwrap_or_default())
}

fn value_as_string(value: &serde_json::Value) -> Option<String> {
    match value {
        serde_json::Value::String(text) => Some(text.clone()),
        serde_json::Value::Number(number) => Some(number.to_string()),
        serde_json::Value::Bool(flag) => Some(flag.to_string()),
        _ => None,
    }
}

fn deserialize_stringish<'de, D>(deserializer: D) -> Result<String, D::Error>
where
    D: Deserializer<'de>,
{
    let value = serde_json::Value::deserialize(deserializer)?;
    value_as_string(&value).ok_or_else(|| serde::de::Error::custom("expected id"))
}

fn deserialize_i64ish<'de, D>(deserializer: D) -> Result<i64, D::Error>
where
    D: Deserializer<'de>,
{
    let value = serde_json::Value::deserialize(deserializer)?;
    value
        .as_i64()
        .or_else(|| value.as_u64().map(|n| n as i64))
        .or_else(|| value.as_f64().map(|n| n as i64))
        .or_else(|| value.as_str().and_then(|text| text.parse().ok()))
        .ok_or_else(|| serde::de::Error::custom("expected id"))
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

fn json_list(value: &serde_json::Value) -> Option<&Vec<serde_json::Value>> {
    value.as_array().or_else(|| {
        ["data", "conversations", "notifications", "messages"]
            .iter()
            .find_map(|key| value.get(*key).and_then(json_list))
    })
}

fn deserialize_opt_ok<'de, D, T>(deserializer: D) -> Result<Option<T>, D::Error>
where
    D: Deserializer<'de>,
    T: serde::de::DeserializeOwned,
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
    /// Next commit in an author continuation, when the API sends one.
    #[serde(default)]
    pub continuation_id: Option<String>,
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
    #[serde(default, deserialize_with = "deserialize_vec")]
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
    /// Captured `@handle`s: developers and GitHub organizations.
    #[serde(default, flatten)]
    pub captured: MentionMaps,
}

impl Commit {
    pub fn mentions(&self) -> Option<&HashMap<String, String>> {
        self.captured.users.as_ref()
    }

    pub fn organization_mentions(&self) -> Option<&HashMap<String, String>> {
        self.captured.organizations.as_ref()
    }

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

    pub fn list_from_value(value: serde_json::Value) -> Vec<Self> {
        json_list(&value)
            .cloned()
            .unwrap_or_default()
            .into_iter()
            .filter_map(|item| serde_json::from_value(item).ok())
            .collect()
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
    #[serde(default, deserialize_with = "deserialize_opt_i64")]
    pub level: Option<i64>,
    #[serde(default)]
    pub rank: Option<String>,
    #[serde(default)]
    pub rank_description: Option<String>,
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
            "first_commit" => format!("{actor} made their first commit"),
            "leveled_up" => self.level_up_title(),
            "approved" => "Your commit was approved.".into(),
            "rejected" => "Your commit was rejected.".into(),
            other if !other.is_empty() => format!("{actor}: {other}"),
            _ => format!("{actor} notified you"),
        }
    }

    pub fn level_up_title(&self) -> String {
        match (
            self.level,
            self.rank.as_deref().filter(|rank| !rank.is_empty()),
        ) {
            (Some(level), Some(rank)) => format!("You reached level {level}: {rank}."),
            _ => "You leveled up.".into(),
        }
    }

    pub fn level_chip(&self) -> Option<String> {
        let level = self.level?;
        let rank = self.rank.as_deref().filter(|rank| !rank.is_empty())?;
        Some(format!("Lv. {level} · {rank}"))
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

    pub fn people(&self) -> Vec<&User> {
        let mut people = Vec::new();
        if let Some(actor) = self.actor.as_ref() {
            people.push(actor);
        }
        for actor in &self.actors {
            if people.iter().all(|seen| seen.id != actor.id) {
                people.push(actor);
            }
        }
        people
    }

    pub fn actors_total(&self) -> i64 {
        let n = self.people().len() as i64;
        self.actors_count.unwrap_or(n).max(n).max(1)
    }

    pub fn other_actors(&self) -> i64 {
        if matches!(self.kind(), "follow" | "followed" | "star" | "starred") {
            self.actors_total().saturating_sub(1)
        } else {
            0
        }
    }

    pub fn is_grouped(&self) -> bool {
        self.event_count.unwrap_or(1) > 1
    }

    pub fn group_noun(&self) -> &'static str {
        match self.kind() {
            "follow" | "followed" => "followers",
            "star" | "starred" => "stars",
            _ => "updates",
        }
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

fn json_datetime(value: &serde_json::Value, names: &[&str]) -> Option<DateTime<Utc>> {
    names.iter().find_map(|name| {
        value
            .get(*name)
            .and_then(|item| item.as_str())
            .and_then(parse_date)
    })
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
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

    pub const CATEGORIES: [Self; 6] = [
        Self::Replies,
        Self::Quotes,
        Self::Stars,
        Self::Mentions,
        Self::Follows,
        Self::Moderation,
    ];

    pub fn enabled_all() -> HashSet<Self> {
        Self::CATEGORIES.into_iter().collect()
    }

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

    pub fn query_param(selected: &HashSet<Self>) -> Option<String> {
        let types: Vec<&str> = Self::CATEGORIES
            .iter()
            .filter(|kind| selected.contains(kind))
            .map(|kind| kind.as_str())
            .collect();
        match types.len() {
            0 => Some("none".into()),
            n if n == Self::CATEGORIES.len() => None,
            _ => Some(types.join(",")),
        }
    }

    pub fn empty_fatal_for(selected: &HashSet<Self>) -> String {
        let types: Vec<&str> = Self::CATEGORIES
            .iter()
            .filter(|kind| selected.contains(kind))
            .map(|kind| kind.as_str())
            .collect();
        if types.len() == Self::CATEGORIES.len() {
            "$ cat /dev/notifications".into()
        } else {
            let pattern = if types.is_empty() {
                "^$".to_owned()
            } else {
                types.join("|")
            };
            format!("$ cat /dev/notifications | grep -E '{pattern}'")
        }
    }

    pub fn empty_detail_for(selected: &HashSet<Self>) -> &'static str {
        let types: Vec<Self> = Self::CATEGORIES
            .iter()
            .copied()
            .filter(|kind| selected.contains(kind))
            .collect();
        match types.as_slice() {
            [] => "All categories are turned off. Enable a category to see notifications.",
            [one] => one.empty_detail(),
            rest if rest.len() == Self::CATEGORIES.len() => Self::All.empty_detail(),
            _ => "No notifications in the selected categories yet.",
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
                "Nothing here yet. Follows, stars, replies, quotes, mentions, level-ups, and moderation updates for your commits will show up here."
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
    #[serde(default)]
    pub read_through_id: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct MessageImage {
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub width: Option<i32>,
    #[serde(default)]
    pub height: Option<i32>,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub url: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Message {
    #[serde(deserialize_with = "deserialize_i64ish")]
    pub id: i64,
    #[serde(default)]
    pub user_id: Option<String>,
    #[serde(default)]
    pub client_id: Option<String>,
    #[serde(default)]
    pub body: Option<String>,
    #[serde(default)]
    pub reply_to_id: Option<i64>,
    #[serde(default, deserialize_with = "deserialize_opt_ok")]
    pub reply_to: Option<MessageReply>,
    /// Captured `@username` → user id, lowercased keys. Only these mentions are linkified.
    #[serde(default, deserialize_with = "deserialize_mentions")]
    pub mentions: Option<HashMap<String, String>>,
    #[serde(
        default,
        alias = "linkPreview",
        deserialize_with = "deserialize_opt_ok"
    )]
    pub link_preview: Option<LinkPreview>,
    #[serde(default, deserialize_with = "deserialize_vec")]
    pub images: Vec<MessageImage>,
    #[serde(default)]
    pub event: Option<String>,
    #[serde(default)]
    pub subject_id: Option<String>,
    #[serde(default, deserialize_with = "deserialize_opt_ok")]
    pub subject: Option<User>,
    #[serde(default, deserialize_with = "deserialize_opt_ok")]
    pub user: Option<User>,
    #[serde(default, deserialize_with = "deserialize_reactions")]
    pub reactions: Vec<MessageReaction>,
    #[serde(default, deserialize_with = "deserialize_opt_datetime")]
    pub created_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
pub struct MessageReaction {
    #[serde(default, deserialize_with = "deserialize_stringish")]
    pub user_id: String,
    #[serde(default)]
    pub emoji: String,
}

impl MessageReaction {
    pub fn from_list(value: &serde_json::Value) -> Vec<Self> {
        let raw = value
            .get("reactions")
            .or_else(|| value.get("data").and_then(|data| data.get("reactions")))
            .unwrap_or(value);
        let items = raw
            .as_array()
            .or_else(|| raw.get("data").and_then(serde_json::Value::as_array));
        items
            .map(|items| {
                items
                    .iter()
                    .filter_map(|item| serde_json::from_value(item.clone()).ok())
                    .filter(|reaction: &Self| {
                        !reaction.emoji.trim().is_empty() && !reaction.user_id.is_empty()
                    })
                    .collect()
            })
            .unwrap_or_default()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct MessageReply {
    #[serde(default, deserialize_with = "deserialize_i64ish")]
    pub id: i64,
    #[serde(default)]
    pub user_id: Option<String>,
    #[serde(default)]
    pub body: Option<String>,
    #[serde(default)]
    pub images_count: i64,
    #[serde(default, deserialize_with = "deserialize_opt_ok")]
    pub user: Option<User>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ConversationSummary {
    #[serde(deserialize_with = "deserialize_stringish")]
    pub id: String,
    #[serde(default)]
    pub kind: ConversationKind,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default, alias = "user", deserialize_with = "deserialize_opt_ok")]
    pub partner: Option<User>,
    #[serde(default, deserialize_with = "deserialize_vec")]
    pub members: Vec<Member>,
    #[serde(default, deserialize_with = "deserialize_opt_ok")]
    pub last_message: Option<Message>,
    #[serde(default, deserialize_with = "deserialize_opt_datetime")]
    pub last_message_at: Option<DateTime<Utc>>,
    #[serde(default, alias = "unread", alias = "unread_count")]
    pub ahead: i64,
    #[serde(
        default,
        alias = "lastIncomingMessageId",
        deserialize_with = "deserialize_opt_i64"
    )]
    pub last_incoming_message_id: Option<i64>,
    #[serde(default)]
    pub is_archived: bool,
    #[serde(default, deserialize_with = "deserialize_opt_datetime")]
    pub pinned_at: Option<DateTime<Utc>>,
    #[serde(default, deserialize_with = "deserialize_opt_datetime")]
    pub muted_until_at: Option<DateTime<Utc>>,
}

impl ConversationSummary {
    pub fn is_org(&self) -> bool {
        self.kind == ConversationKind::Org || self.partner.is_none()
    }

    pub fn others<'a>(&'a self, me_id: Option<&str>) -> Vec<&'a User> {
        self.members
            .iter()
            .map(|member| &member.user)
            .filter(|user| me_id.is_none_or(|id| user.id != id))
            .collect()
    }

    pub fn member_count(&self, me_id: Option<&str>) -> usize {
        self.others(me_id).len().saturating_add(1)
    }

    pub fn title(&self) -> String {
        self.title_for(None)
    }

    pub fn title_for(&self, me: Option<&User>) -> String {
        if !self.is_org() {
            return self
                .partner
                .as_ref()
                .map(|user| user.display_name().to_owned())
                .unwrap_or_else(|| "remote".into());
        }
        if let Some(name) = self
            .name
            .as_deref()
            .map(str::trim)
            .filter(|name| !name.is_empty())
        {
            return name.to_owned();
        }
        let others = self.others(me.map(|user| user.id.as_str()));
        if others.is_empty() {
            "Just you".into()
        } else {
            others
                .into_iter()
                .map(first_name)
                .collect::<Vec<_>>()
                .join(", ")
        }
    }

    pub fn handle_or_meta(&self, me_id: Option<&str>) -> String {
        if self.is_org() {
            format!("org · {}", self.member_count(me_id))
        } else {
            self.partner
                .as_ref()
                .map(|user| format!("@{}", user.username))
                .unwrap_or_default()
        }
    }

    pub fn preview(&self) -> String {
        self.last_message
            .as_ref()
            .and_then(|message| message.body.clone())
            .unwrap_or_default()
    }

    pub fn preview_for(&self, me_id: Option<&str>) -> String {
        let Some(message) = &self.last_message else {
            return String::new();
        };
        if message
            .event
            .as_deref()
            .is_some_and(|event| !event.is_empty())
        {
            return event_preview(message);
        }
        let body = message
            .body
            .as_deref()
            .map(str::trim)
            .filter(|body| !body.is_empty())
            .unwrap_or("sent an image");
        let mine = me_id.is_some() && message.user_id.as_deref() == me_id;
        if mine {
            format!("you: {body}")
        } else if self.is_org() {
            let who = message
                .user
                .as_ref()
                .map(first_name)
                .unwrap_or_else(|| "someone".into());
            format!("{who}: {body}")
        } else {
            body.to_owned()
        }
    }

    pub fn is_pinned(&self) -> bool {
        self.pinned_at.is_some()
    }

    pub fn is_muted(&self) -> bool {
        self.muted_until_at.is_some_and(|until| until > Utc::now())
    }

    pub fn arrival_id(&self, me_id: Option<&str>) -> i64 {
        if let Some(id) = self.last_incoming_message_id.filter(|id| *id > 0) {
            return id;
        }
        let Some(message) = &self.last_message else {
            return 0;
        };
        let mine = me_id.is_some_and(|me| {
            message.user_id.as_deref() == Some(me)
                || message.user.as_ref().is_some_and(|user| user.id == me)
        });
        if mine {
            0
        } else {
            message.id
        }
    }

    pub fn should_alert_arrival(
        &self,
        previous_incoming: i64,
        reading: bool,
        me_id: Option<&str>,
    ) -> bool {
        let incoming = self.arrival_id(me_id);
        if previous_incoming <= 0 || incoming <= previous_incoming || self.ahead <= 0 {
            return false;
        }
        if self.is_muted() {
            return false;
        }
        let ping = self
            .last_message
            .as_ref()
            .and_then(|message| message.event.as_deref())
            == Some("ping");
        !reading || ping
    }

    pub fn matches_query(&self, query: &str, me: Option<&User>) -> bool {
        let needle = query.trim().trim_start_matches('@').to_lowercase();
        if needle.is_empty() {
            return true;
        }
        let me_id = me.map(|user| user.id.as_str());
        let mut hay = self.title_for(me);
        hay.push(' ');
        hay.push_str(&self.handle_or_meta(me_id));
        if let Some(partner) = &self.partner {
            hay.push(' ');
            hay.push_str(partner.display_name());
            hay.push(' ');
            hay.push_str(&partner.username);
        }
        for member in &self.members {
            hay.push(' ');
            hay.push_str(member.user.display_name());
            hay.push(' ');
            hay.push_str(&member.user.username);
        }
        hay.to_lowercase().contains(&needle)
    }

    pub fn is_visible(&self) -> bool {
        self.kind == ConversationKind::Org
            || self
                .partner
                .as_ref()
                .is_some_and(|user| !user.username.trim().is_empty())
    }
}

fn first_name(user: &User) -> String {
    user.name
        .as_deref()
        .map(str::trim)
        .filter(|name| !name.is_empty())
        .and_then(|name| name.split_whitespace().next())
        .unwrap_or(user.username.as_str())
        .to_owned()
}

pub fn event_preview(message: &Message) -> String {
    event_line(
        message.user.as_ref(),
        message.subject.as_ref(),
        message.event.as_deref(),
        message.body.as_deref(),
    )
}

pub fn event_line(
    actor: Option<&User>,
    subject: Option<&User>,
    event: Option<&str>,
    body: Option<&str>,
) -> String {
    let actor = event_handle(actor);
    let subject = event_handle(subject);
    let body = body.map(str::trim).filter(|text| !text.is_empty());
    match event {
        Some("created") => body
            .map(|name| format!("{actor} created {name}"))
            .unwrap_or_else(|| format!("{actor} created the org")),
        Some("added") => format!("{actor} added {subject}"),
        Some("removed") => format!("{actor} removed {subject}"),
        Some("left") => format!("{actor} left"),
        Some("renamed") => body
            .map(|name| format!("{actor} renamed the org to {name}"))
            .unwrap_or_else(|| format!("{actor} cleared the org's name")),
        Some("promoted") => format!("{actor} made {subject} a maintainer"),
        Some("demoted") => format!("{actor} made {subject} a contributor"),
        Some("inherited") => format!("{actor} inherited maintainer"),
        Some("ping") => format!("{actor} sent a ping"),
        Some("call_started") => format!("{actor} started a call"),
        Some("call_answered") => format!("{actor} answered the call"),
        Some("call_cancelled") => format!("{actor} cancelled the call"),
        Some("call_declined") => format!("{actor} declined the call"),
        Some("call_failed") => format!("{actor} couldn't answer the call"),
        Some("call_missed") => format!("missed call from {actor}"),
        _ => body.unwrap_or_default().to_owned(),
    }
}

fn event_handle(user: Option<&User>) -> String {
    user.map(|user| user.username.trim())
        .filter(|username| !username.is_empty())
        .map(|username| format!("@{username}"))
        .unwrap_or_else(|| "someone".into())
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct RemoteCandidate {
    pub user: User,
    #[serde(default, deserialize_with = "deserialize_opt_ok")]
    pub reason: Option<String>,
}

impl RemoteCandidate {
    pub fn list_from_value(value: serde_json::Value) -> Vec<Self> {
        let data = value.get("data").cloned().unwrap_or(value.clone());
        json_vec::<Self>(&data, "candidates")
            .or_else(|| json_vec(&value, "candidates"))
            .unwrap_or_default()
    }

    pub fn pickable(&self) -> bool {
        self.reason.as_deref().is_none_or(str::is_empty)
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

impl InboxPage {
    pub fn from_value(value: serde_json::Value) -> Self {
        let root = value
            .get("data")
            .filter(|data| data.is_object() || data.is_array())
            .unwrap_or(&value);
        let items = json_list(&value)
            .or_else(|| json_list(root))
            .cloned()
            .unwrap_or_default();
        let data = items
            .into_iter()
            .filter_map(|item| serde_json::from_value::<ConversationSummary>(item).ok())
            .filter(ConversationSummary::is_visible)
            .collect();
        Self {
            data,
            has_more: json_bool(root, "has_more")
                .or_else(|| json_bool(root, "hasMore"))
                .or_else(|| json_bool(&value, "has_more"))
                .or_else(|| json_bool(&value, "hasMore"))
                .unwrap_or(false),
            unread_count: json_count(root, &["unread_count", "unreadCount", "unread"])
                .or_else(|| json_count(&value, &["unread_count", "unreadCount", "unread"]))
                .unwrap_or(0),
            requests_count: json_count(root, &["requests_count", "requestsCount", "requests"])
                .or_else(|| json_count(&value, &["requests_count", "requestsCount", "requests"]))
                .unwrap_or(0),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct OrgInfo {
    pub id: String,
    #[serde(default)]
    pub name: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ChatThread {
    #[serde(default, deserialize_with = "deserialize_vec")]
    pub messages: Vec<Message>,
    #[serde(default)]
    pub has_older: bool,
    #[serde(default)]
    pub read_through_id: i64,
    #[serde(default)]
    pub is_archived: bool,
    #[serde(default)]
    pub access: Option<String>,
    #[serde(default, deserialize_with = "deserialize_opt_ok")]
    pub partner: Option<User>,
    #[serde(default)]
    pub org: Option<OrgInfo>,
    #[serde(default, deserialize_with = "deserialize_vec")]
    pub members: Vec<Member>,
    #[serde(default)]
    pub partner_read_through_id: i64,
    #[serde(default, deserialize_with = "deserialize_opt_datetime")]
    pub partner_read_at: Option<DateTime<Utc>>,
    #[serde(default)]
    pub partner_typing: bool,
    #[serde(default, deserialize_with = "deserialize_vec")]
    pub typing: Vec<String>,
    #[serde(default, deserialize_with = "deserialize_vec")]
    pub pinned: Vec<Message>,
    #[serde(default)]
    pub role: Option<String>,
    #[serde(default, deserialize_with = "deserialize_opt_datetime")]
    pub muted_until_at: Option<DateTime<Utc>>,
}

impl ChatThread {
    pub fn pinned_from_value(value: &serde_json::Value) -> Vec<Message> {
        if let Some(pinned) = value.get("pinned") {
            return json_message_list(pinned);
        }
        if let Some(data) = value.get("data") {
            if let Some(pinned) = data.get("pinned") {
                return json_message_list(pinned);
            }
            if data.is_array() {
                return json_message_list(data);
            }
        }
        Vec::new()
    }

    pub fn is_muted(&self) -> bool {
        self.muted_until_at.is_some_and(|until| until > Utc::now())
    }

    pub fn is_maintainer_for(&self, user_id: Option<&str>) -> bool {
        if self
            .role
            .as_deref()
            .is_some_and(|role| role.eq_ignore_ascii_case("maintainer"))
        {
            return true;
        }
        let Some(user_id) = user_id.filter(|id| !id.is_empty()) else {
            return false;
        };
        self.members.iter().any(|member| {
            member.user.id == user_id
                && member
                    .role
                    .as_deref()
                    .is_some_and(|role| role.eq_ignore_ascii_case("maintainer"))
        })
    }

    pub fn from_value(value: serde_json::Value) -> Self {
        let root = value
            .get("data")
            .filter(|data| data.is_object())
            .or_else(|| value.get("thread").filter(|data| data.is_object()))
            .unwrap_or(&value);
        let messages = json_list(root)
            .or_else(|| json_list(&value))
            .cloned()
            .unwrap_or_default()
            .into_iter()
            .filter_map(|item| serde_json::from_value(item).ok())
            .collect();
        let pinned = Self::pinned_from_value(&value);
        let role = root
            .get("role")
            .and_then(serde_json::Value::as_str)
            .filter(|role| !role.is_empty())
            .map(str::to_owned);
        Self {
            messages,
            has_older: json_bool(root, "has_older")
                .or_else(|| json_bool(root, "hasOlder"))
                .unwrap_or(false),
            read_through_id: json_count(root, &["read_through_id", "readThroughId"]).unwrap_or(0),
            is_archived: json_bool(root, "is_archived").unwrap_or(false),
            access: root
                .get("access")
                .and_then(serde_json::Value::as_str)
                .map(str::to_owned),
            partner: root
                .get("partner")
                .cloned()
                .and_then(|value| serde_json::from_value(value).ok()),
            org: root
                .get("org")
                .cloned()
                .and_then(|value| serde_json::from_value(value).ok()),
            members: json_vec(root, "members").unwrap_or_default(),
            partner_read_through_id: json_count(
                root,
                &["partner_read_through_id", "partnerReadThroughId"],
            )
            .unwrap_or(0),
            partner_read_at: json_datetime(root, &["partner_read_at", "partnerReadAt"]),
            partner_typing: json_bool(root, "partner_typing").unwrap_or(false),
            typing: root
                .get("typing")
                .and_then(serde_json::Value::as_array)
                .map(|items| {
                    items
                        .iter()
                        .filter_map(serde_json::Value::as_str)
                        .filter(|name| !name.is_empty())
                        .map(str::to_owned)
                        .collect()
                })
                .unwrap_or_default(),
            pinned,
            role,
            muted_until_at: json_datetime(root, &["muted_until_at", "mutedUntilAt"]),
        }
    }
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

    pub fn from_created_org(value: &serde_json::Value) -> Option<Self> {
        let org = value
            .get("org")
            .or_else(|| value.get("data").and_then(|data| data.get("org")))
            .or_else(|| value.get("data"))
            .unwrap_or(value);
        let id = match org.get("id") {
            Some(serde_json::Value::String(id)) if !id.is_empty() => id.clone(),
            Some(serde_json::Value::Number(id)) => id.to_string(),
            _ => return None,
        };
        let name = org
            .get("name")
            .and_then(serde_json::Value::as_str)
            .map(str::trim)
            .filter(|name| !name.is_empty())
            .map(str::to_owned);
        Some(Self::Org { id, name })
    }

    pub fn title(&self) -> String {
        match self {
            Self::Direct { username } => format!("@{username}"),
            Self::Org { name, id } => name.clone().unwrap_or_else(|| id.clone()),
        }
    }

    pub fn key(&self) -> String {
        match self {
            Self::Direct { username } => username.clone(),
            Self::Org { id, .. } => format!("org:{id}"),
        }
    }

    pub fn html_url(&self, base: &str) -> String {
        match self {
            Self::Direct { username } => format!("{base}/remotes/@{username}"),
            Self::Org { id, .. } => format!("{base}/remotes/orgs/{id}"),
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
mod notification_filter_tests {
    use super::*;

    #[test]
    fn query_param_omits_type_when_every_category_is_on() {
        assert_eq!(
            NotificationFilter::query_param(&NotificationFilter::enabled_all()),
            None
        );
    }

    #[test]
    fn query_param_joins_included_categories() {
        let selected = [NotificationFilter::Replies, NotificationFilter::Mentions]
            .into_iter()
            .collect();
        assert_eq!(
            NotificationFilter::query_param(&selected).as_deref(),
            Some("replies,mentions")
        );
    }

    #[test]
    fn query_param_sends_none_when_every_category_is_off() {
        assert_eq!(
            NotificationFilter::query_param(&HashSet::new()).as_deref(),
            Some("none")
        );
    }

    #[test]
    fn empty_copy_matches_the_web_header() {
        assert_eq!(
            NotificationFilter::empty_fatal_for(&NotificationFilter::enabled_all()),
            "$ cat /dev/notifications"
        );
        assert_eq!(
            NotificationFilter::empty_detail_for(&HashSet::new()),
            "All categories are turned off. Enable a category to see notifications."
        );
        let mixed = [NotificationFilter::Replies, NotificationFilter::Quotes]
            .into_iter()
            .collect();
        assert_eq!(
            NotificationFilter::empty_fatal_for(&mixed),
            "$ cat /dev/notifications | grep -E 'replies|quotes'"
        );
        assert_eq!(
            NotificationFilter::empty_detail_for(&mixed),
            "No notifications in the selected categories yet."
        );
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
    fn inbox_page_reads_web_conversations_and_null_members() {
        let page = InboxPage::from_value(json!({
            "conversations": [{
                "id": "c1",
                "kind": "direct",
                "partner": { "id": "u1", "handle": "ada", "name": "Ada" },
                "members": null,
                "last_message": {
                    "id": 9,
                    "body": "hello",
                    "mentions": [],
                    "images": [{ "id": "img1" }]
                },
                "ahead": 2
            }],
            "has_more": false,
            "unread": 3,
            "requests": 1
        }));
        assert_eq!(page.data.len(), 1);
        assert_eq!(page.data[0].title(), "Ada");
        assert_eq!(page.data[0].preview(), "hello");
        assert_eq!(page.data[0].ahead, 2);
        assert!(page.data[0].members.is_empty());
        assert_eq!(page.unread_count, 3);
        assert_eq!(page.requests_count, 1);
        assert!(!page.has_more);
    }

    #[test]
    fn inbox_page_reads_wrapped_conversations() {
        let page = InboxPage::from_value(json!({
            "data": {
                "conversations": [{
                    "id": 42,
                    "partner": { "id": "u1", "username": "grace" },
                    "members": null,
                    "unread": 4
                }],
                "unread_count": 5,
                "hasMore": true
            }
        }));
        assert_eq!(page.data.len(), 1);
        assert_eq!(page.data[0].id, "42");
        assert_eq!(page.data[0].title(), "grace");
        assert_eq!(page.data[0].ahead, 4);
        assert_eq!(page.unread_count, 5);
        assert!(page.has_more);
    }

    #[test]
    fn inbox_page_hides_direct_remotes_with_deleted_partners() {
        let page = InboxPage::from_value(json!({
            "conversations": [
                { "id": "gone", "kind": "direct", "members": null },
                {
                    "id": "ok",
                    "kind": "direct",
                    "partner": { "id": "u1", "handle": "ada" }
                },
                { "id": "org", "kind": "org", "name": "core" }
            ]
        }));
        assert_eq!(
            page.data
                .iter()
                .map(|item| item.id.as_str())
                .collect::<Vec<_>>(),
            ["ok", "org"]
        );
    }

    #[test]
    fn related_commits_read_a_resource_collection() {
        let commits = Commit::list_from_value(json!({
            "data": [
                { "id": "c1", "body": "first", "user": { "id": "u1", "handle": "ada" } },
                { "broken": true },
                { "id": "c2", "body": "second" }
            ]
        }));
        assert_eq!(commits.len(), 2);
        assert_eq!(commits[0].author.as_ref().unwrap().username, "ada");
        assert_eq!(commits[1].id, "c2");
    }

    #[test]
    fn inbox_page_keeps_a_row_when_last_message_is_unreadable() {
        let page = InboxPage::from_value(json!({
            "conversations": [{
                "id": "c1",
                "partner": { "id": "u1", "handle": "ada" },
                "last_message": { "broken": true }
            }]
        }));
        assert_eq!(page.data.len(), 1);
        assert!(page.data[0].last_message.is_none());
        assert_eq!(page.data[0].title(), "ada");
    }

    #[test]
    fn org_rows_use_member_names_and_counts() {
        let page = InboxPage::from_value(json!({
            "conversations": [{
                "id": "org1",
                "kind": "org",
                "members": [
                    { "user": { "id": "me", "handle": "nuno", "name": "Nuno Maduro" } },
                    { "user": { "id": "u2", "handle": "ada", "name": "Ada Lovelace" } },
                    { "user": { "id": "u3", "handle": "grace", "name": "Grace Hopper" } }
                ],
                "last_message": {
                    "id": 4,
                    "user_id": "u2",
                    "body": "ship it",
                    "user": { "id": "u2", "handle": "ada", "name": "Ada Lovelace" }
                },
                "last_message_at": "2026-10-01T12:00:00Z"
            }]
        }));
        let me = User {
            id: "me".into(),
            username: "nuno".into(),
            name: Some("Nuno Maduro".into()),
            ..User::default()
        };
        let org = &page.data[0];
        assert_eq!(org.title_for(Some(&me)), "Ada, Grace");
        assert_eq!(org.handle_or_meta(Some("me")), "org · 3");
        assert_eq!(org.preview_for(Some("me")), "Ada: ship it");
        assert!(org.matches_query("ada", Some(&me)));
        assert!(!org.matches_query("linus", Some(&me)));
    }

    #[test]
    fn dm_preview_prefixes_your_own_push() {
        let page = InboxPage::from_value(json!({
            "conversations": [{
                "id": "dm",
                "kind": "direct",
                "partner": { "id": "u2", "handle": "ada", "name": "Ada" },
                "last_message": { "id": 1, "user_id": "me", "body": "hey" }
            }]
        }));
        assert_eq!(page.data[0].preview_for(Some("me")), "you: hey");
        assert_eq!(page.data[0].handle_or_meta(Some("me")), "@ada");
    }

    #[test]
    fn remote_candidates_read_the_web_payload() {
        let people = RemoteCandidate::list_from_value(json!({
            "candidates": [
                { "user": { "id": "u1", "handle": "ada", "name": "Ada" }, "reason": null },
                { "user": { "id": "u2", "handle": "grace" }, "reason": "already in this org" }
            ]
        }));
        assert_eq!(people.len(), 2);
        assert!(people[0].pickable());
        assert!(!people[1].pickable());
        assert_eq!(people[0].user.username, "ada");
    }

    #[test]
    fn created_org_reads_id_from_the_web_payload() {
        let reference = ThreadRef::from_created_org(&json!({
            "org": { "id": 9, "name": "core" }
        }))
        .unwrap();
        assert_eq!(
            reference,
            ThreadRef::Org {
                id: "9".into(),
                name: Some("core".into())
            }
        );
    }

    #[test]
    fn chat_thread_skips_broken_messages() {
        let thread = ChatThread::from_value(json!({
            "partner": { "id": "u1", "handle": "ada" },
            "messages": [
                { "id": 1, "body": "hi" },
                { "broken": true },
                { "id": "2", "body": "there" }
            ],
            "has_older": true
        }));
        assert_eq!(thread.messages.len(), 2);
        assert_eq!(thread.messages[1].id, 2);
        assert!(thread.has_older);
        assert_eq!(thread.partner.unwrap().username, "ada");
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
    fn grouped_stars_name_the_others_from_the_preview_or_the_count() {
        let grace = User {
            id: "1".into(),
            username: "grace".into(),
            name: Some("Grace".into()),
            ..Default::default()
        };
        let ada = User {
            id: "2".into(),
            username: "ada".into(),
            name: Some("Ada".into()),
            ..Default::default()
        };
        let grouped = Notification {
            kind: Some("starred".into()),
            actor: Some(grace.clone()),
            actors: vec![grace.clone(), ada],
            actors_count: Some(5),
            event_count: Some(5),
            ..Default::default()
        };
        assert_eq!(grouped.other_actors(), 4);
        assert_eq!(grouped.people().len(), 2);
        assert_eq!(grouped.actors_total(), 5);
        assert!(grouped.is_grouped());
        assert_eq!(grouped.group_noun(), "stars");
        let missing_count = Notification {
            kind: Some("starred".into()),
            actor: Some(grace.clone()),
            actors: vec![grace],
            ..Default::default()
        };
        assert_eq!(missing_count.other_actors(), 0);
        assert!(!missing_count.is_grouped());
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
    fn live_stream_reads_platform_and_end_time() {
        let user: User = serde_json::from_value(json!({
            "id": "1",
            "handle": "nuno",
            "live": {
                "url": "https://www.twitch.tv/nuno",
                "thumbnail_url": "https://example.com/thumb.jpg",
                "platform": "twitch",
                "ends_at": "2099-01-01T00:00:00Z"
            }
        }))
        .unwrap();
        let live = user.live.as_ref().expect("live object");
        assert_eq!(live.platform.as_deref(), Some("twitch"));
        assert!(live.ends_at.is_some());
        assert!(user.is_live());
    }

    #[test]
    fn expired_live_stream_is_not_live() {
        let user: User = serde_json::from_value(json!({
            "id": "1",
            "handle": "nuno",
            "live": {
                "url": "https://www.twitch.tv/nuno",
                "platform": "twitch",
                "ends_at": "2020-01-01T00:00:00Z"
            }
        }))
        .unwrap();
        assert!(!user.is_live());
        assert_eq!(user.live_url(), None);
    }

    #[test]
    fn first_commit_notification_names_the_actor() {
        let notice = Notification {
            kind: Some("first_commit".into()),
            actor: Some(User {
                username: "ada".into(),
                name: Some("Ada".into()),
                ..Default::default()
            }),
            ..Default::default()
        };
        assert_eq!(notice.title(), "Ada made their first commit");
    }

    #[test]
    fn leveled_up_notification_names_the_rank() {
        let notice = Notification {
            kind: Some("leveled_up".into()),
            level: Some(4),
            rank: Some("maintainer".into()),
            rank_description: Some("You keep the lights on.".into()),
            ..Default::default()
        };
        assert_eq!(notice.title(), "You reached level 4: maintainer.");
        assert_eq!(notice.level_chip().as_deref(), Some("Lv. 4 · maintainer"));
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

    #[test]
    fn is_followed_reads_sql_exists_as_one() {
        let user: User = serde_json::from_value(json!({
            "id": "1",
            "handle": "ada",
            "is_followed": 1
        }))
        .unwrap();
        assert!(user.is_followed_by_me());
    }

    #[test]
    fn suggestion_reason_prefers_people_you_follow() {
        let user = User {
            followed_by_following_count: Some(3),
            followers_count: Some(12),
            bio: Some("  ships rust\nand cakes".into()),
            ..User::default()
        };
        assert_eq!(
            user.suggestion_reason().as_deref(),
            Some("Followed by 3 you follow")
        );
        assert_eq!(user.bio_line(), Some("ships rust"));
        let popular = User {
            followers_count: Some(1),
            ..User::default()
        };
        assert_eq!(popular.suggestion_reason().as_deref(), Some("1 follower"));
    }

    #[test]
    fn user_preview_carries_follow_flags_from_the_profile() {
        let preview = UserPreview::from_user(User {
            username: "ada".into(),
            is_followed: Some(false),
            follows_you: Some(true),
            ..User::default()
        });
        assert!(!preview.is_following);
        assert!(preview.follows_you);
        assert_eq!(UserPreview::key("Ada"), "ada");
    }

    #[test]
    fn organization_reads_handle_and_additional_admins() {
        let organization = Organization::from_value(json!({
            "data": {
                "handle": "laravel",
                "name": "Laravel",
                "avatar_url": "https://example.com/laravel.png",
                "bio": "The PHP framework",
                "location": "United States",
                "website": "https://laravel.com",
                "github_url": "https://github.com/laravel",
                "html_url": "https://dev-null.sh/@laravel"
            },
            "lane": 1,
            "admins": [{ "id": "u1", "handle": "taylorotwell", "name": "Taylor Otwell" }]
        }))
        .unwrap();
        assert_eq!(organization.handle, "laravel");
        assert_eq!(organization.display_name(), "Laravel");
        assert_eq!(organization.lane, Some(Lane::Palette(1)));
        assert_eq!(organization.admins[0].username, "taylorotwell");
        assert_eq!(
            organization.html_url("https://dev-null.sh"),
            "https://dev-null.sh/@laravel"
        );
    }

    #[test]
    fn commit_mentions_split_organizations_from_maps_and_positions() {
        let mapped: Commit = serde_json::from_value(json!({
            "id": "c1",
            "mentions": { "nuno": "u1" },
            "organization_mentions": { "Laravel": "o1" }
        }))
        .unwrap();
        assert_eq!(mapped.mentions().unwrap().get("nuno").unwrap(), "u1");
        assert_eq!(
            mapped.organization_mentions().unwrap().get("laravel").unwrap(),
            "o1"
        );

        let positioned: Commit = serde_json::from_value(json!({
            "id": "c2",
            "mentions": [
                { "handle": "nuno", "offset": 0, "length": 5, "kind": "user" },
                { "handle": "laravel", "offset": 8, "length": 8, "kind": "organization" }
            ]
        }))
        .unwrap();
        assert_eq!(positioned.mentions().unwrap().get("nuno").unwrap(), "");
        assert_eq!(
            positioned
                .organization_mentions()
                .unwrap()
                .get("laravel")
                .unwrap(),
            ""
        );
        assert!(positioned.mentions().unwrap().get("laravel").is_none());
    }

    #[test]
    fn mention_suggestions_read_users_then_organizations() {
        let suggestions = MentionSuggestions::from_value(json!({
            "data": [{ "id": "u1", "handle": "nuno", "name": "Nuno Maduro" }],
            "organizations": [{ "handle": "laravel", "name": "Laravel" }]
        }));
        assert_eq!(suggestions.users[0].username, "nuno");
        assert_eq!(suggestions.organizations[0].handle, "laravel");
    }

    #[test]
    fn user_settings_default_to_automatic_translation() {
        let empty: UserSettings = serde_json::from_value(json!({})).unwrap();
        assert!(empty.automatic_translation);
        assert!(UserSettings::default().automatic_translation);
    }

    #[test]
    fn commit_translation_reads_body_segments() {
        let translation = CommitTranslation::from_value(json!({
            "translation": {
                "language": "en",
                "source_language": "de",
                "source_name": "German",
                "available": true,
                "translated": {
                    "body_segments": [
                        { "text": "Good " },
                        { "text": "morning" }
                    ]
                }
            }
        }));
        assert_eq!(translation.source_name.as_deref(), Some("German"));
        assert_eq!(translation.body.as_deref(), Some("Good morning"));
        assert!(translation.shows_translation());
    }

    #[test]
    fn suggested_users_skip_people_already_followed() {
        let users = User::list_from_value(json!({
            "suggestedUsers": [
                { "id": "1", "handle": "ada", "is_followed": 1 },
                { "id": "2", "handle": "grace" },
                { "id": "3", "username": "linus" }
            ]
        }));
        assert_eq!(
            users
                .iter()
                .map(|user| user.username.as_str())
                .collect::<Vec<_>>(),
            ["ada", "grace", "linus"]
        );
        let following = User::following_names(&[User {
            username: "Linus".into(),
            ..Default::default()
        }]);
        let shown = User::not_followed(users, &following);
        assert_eq!(
            shown
                .iter()
                .map(|user| user.username.as_str())
                .collect::<Vec<_>>(),
            ["grace"]
        );
    }

    #[test]
    fn suggested_from_value_does_not_treat_a_following_list_as_suggestions() {
        let users = User::suggested_from_value(&json!({
            "users": [{ "id": "1", "handle": "ada" }],
            "suggestedUsers": [{ "id": "2", "handle": "grace" }]
        }));
        assert_eq!(
            users
                .iter()
                .map(|user| user.username.as_str())
                .collect::<Vec<_>>(),
            ["grace"]
        );
        let following = User::list_from_value(json!({
            "users": [{ "id": "1", "handle": "ada" }],
            "suggestedUsers": [{ "id": "2", "handle": "grace" }]
        }));
        assert_eq!(
            following
                .iter()
                .map(|user| user.username.as_str())
                .collect::<Vec<_>>(),
            ["ada"]
        );
    }

    #[test]
    fn unique_unfollowed_authors_skip_people_already_followed() {
        let commits = [
            Commit {
                author: Some(User {
                    username: "ada".into(),
                    is_followed: Some(true),
                    ..Default::default()
                }),
                ..Default::default()
            },
            Commit {
                author: Some(User {
                    username: "grace".into(),
                    ..Default::default()
                }),
                ..Default::default()
            },
            Commit {
                author: Some(User {
                    username: "Grace".into(),
                    ..Default::default()
                }),
                ..Default::default()
            },
            Commit {
                author: Some(User {
                    username: "linus".into(),
                    ..Default::default()
                }),
                ..Default::default()
            },
        ];
        let following = HashSet::from(["linus".into()]);
        let shown = User::unique_unfollowed_authors(&commits, &following);
        assert_eq!(
            shown
                .iter()
                .map(|user| user.username.as_str())
                .collect::<Vec<_>>(),
            ["grace"]
        );
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
        assert_eq!(format!("{} XP", achievements.xp), "916 XP");
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

#[cfg(test)]
mod remotes_tests {
    use super::*;

    #[test]
    fn thread_keys_match_the_web() {
        assert_eq!(
            ThreadRef::Direct {
                username: "ada".into()
            }
            .key(),
            "ada"
        );
        assert_eq!(
            ThreadRef::Org {
                id: "org-1".into(),
                name: Some("core".into())
            }
            .key(),
            "org:org-1"
        );
        assert_eq!(
            ThreadRef::Direct {
                username: "ada".into()
            }
            .html_url("https://dev-null.sh"),
            "https://dev-null.sh/remotes/@ada"
        );
        assert_eq!(
            ThreadRef::Org {
                id: "org-1".into(),
                name: Some("core".into())
            }
            .html_url("https://dev-null.sh"),
            "https://dev-null.sh/remotes/orgs/org-1"
        );
    }

    #[test]
    fn added_event_names_both_people() {
        let message = Message {
            event: Some("added".into()),
            user: Some(User {
                username: "ada".into(),
                ..User::default()
            }),
            subject: Some(User {
                username: "grace".into(),
                ..User::default()
            }),
            ..Message::default()
        };
        assert_eq!(event_preview(&message), "@ada added @grace");
        assert_eq!(
            event_preview(&Message {
                event: Some("added".into()),
                ..Message::default()
            }),
            "someone added someone"
        );
    }

    #[test]
    fn call_events_name_who_started_or_missed() {
        let ada = User {
            username: "ada".into(),
            ..User::default()
        };
        assert_eq!(
            event_preview(&Message {
                event: Some("call_started".into()),
                user: Some(ada.clone()),
                ..Message::default()
            }),
            "@ada started a call"
        );
        assert_eq!(
            event_preview(&Message {
                event: Some("call_answered".into()),
                user: Some(ada.clone()),
                ..Message::default()
            }),
            "@ada answered the call"
        );
        assert_eq!(
            event_preview(&Message {
                event: Some("call_cancelled".into()),
                user: Some(ada.clone()),
                ..Message::default()
            }),
            "@ada cancelled the call"
        );
        assert_eq!(
            event_preview(&Message {
                event: Some("call_declined".into()),
                user: Some(ada.clone()),
                ..Message::default()
            }),
            "@ada declined the call"
        );
        assert_eq!(
            event_preview(&Message {
                event: Some("call_failed".into()),
                user: Some(ada.clone()),
                ..Message::default()
            }),
            "@ada couldn't answer the call"
        );
        assert_eq!(
            event_preview(&Message {
                event: Some("call_missed".into()),
                user: Some(ada),
                ..Message::default()
            }),
            "missed call from @ada"
        );
    }

    #[test]
    fn thread_parses_pull_receipts() {
        let thread = ChatThread::from_value(serde_json::json!({
            "messages": [],
            "partner_read_through_id": 12,
            "partner_read_at": "2026-10-28T12:34:56Z",
            "members": [{
                "user": { "id": "1", "username": "ada" },
                "read_through_id": 9
            }]
        }));
        assert_eq!(thread.partner_read_through_id, 12);
        assert!(thread.partner_read_at.is_some());
        assert_eq!(thread.members[0].read_through_id, 9);
    }

    #[test]
    fn thread_parses_org_pins_and_the_viewer_role() {
        let thread = ChatThread::from_value(serde_json::json!({
            "role": "maintainer",
            "messages": [{ "id": 1, "body": "hi" }],
            "pinned": [
                { "id": 4, "body": "keep this", "user": { "id": "1", "username": "ada" } },
                { "broken": true },
                { "id": "7", "body": "and this" }
            ],
            "members": [{
                "user": { "id": "2", "username": "grace" },
                "role": "contributor"
            }]
        }));
        assert_eq!(thread.role.as_deref(), Some("maintainer"));
        assert!(thread.is_maintainer_for(None));
        assert_eq!(thread.pinned.len(), 2);
        assert_eq!(thread.pinned[0].id, 4);
        assert_eq!(thread.pinned[1].id, 7);
        let contributor = ChatThread {
            members: vec![Member {
                user: User {
                    id: "2".into(),
                    username: "grace".into(),
                    ..User::default()
                },
                role: Some("contributor".into()),
                ..Member::default()
            }],
            ..ChatThread::default()
        };
        assert!(!contributor.is_maintainer_for(Some("2")));
        let by_member = ChatThread {
            members: vec![Member {
                user: User {
                    id: "2".into(),
                    username: "grace".into(),
                    ..User::default()
                },
                role: Some("maintainer".into()),
                ..Member::default()
            }],
            ..ChatThread::default()
        };
        assert!(by_member.is_maintainer_for(Some("2")));
        assert!(!by_member.is_maintainer_for(Some("1")));
    }

    #[test]
    fn pin_payload_reads_a_wrapped_collection() {
        let pinned = ChatThread::pinned_from_value(&serde_json::json!({
            "pinned": { "data": [{ "id": 9, "body": "note" }] },
            "reactions_version": 3
        }));
        assert_eq!(pinned.len(), 1);
        assert_eq!(pinned[0].id, 9);
        let nested = ChatThread::pinned_from_value(&serde_json::json!({
            "data": { "pinned": [{ "id": 2, "body": "hi" }] }
        }));
        assert_eq!(nested[0].id, 2);
    }

    #[test]
    fn conversation_mute_and_arrival_alerts() {
        let muted = ConversationSummary {
            last_incoming_message_id: Some(8),
            ahead: 1,
            muted_until_at: Some(Utc::now() + chrono::Duration::hours(1)),
            ..ConversationSummary::default()
        };
        assert!(muted.is_muted());
        assert!(!muted.should_alert_arrival(4, false, None));

        let incoming = ConversationSummary {
            last_incoming_message_id: Some(8),
            ahead: 1,
            last_message: Some(Message {
                id: 8,
                body: Some("hey".into()),
                ..Message::default()
            }),
            ..ConversationSummary::default()
        };
        assert!(incoming.should_alert_arrival(4, false, None));
        assert!(!incoming.should_alert_arrival(0, false, None));
        assert!(!incoming.should_alert_arrival(4, true, None));
        assert!(!incoming.should_alert_arrival(8, false, None));

        let ping = ConversationSummary {
            last_incoming_message_id: Some(9),
            ahead: 1,
            last_message: Some(Message {
                id: 9,
                event: Some("ping".into()),
                ..Message::default()
            }),
            ..ConversationSummary::default()
        };
        assert!(ping.should_alert_arrival(4, true, None));
    }

    #[test]
    fn chat_thread_reads_muted_until() {
        let thread = ChatThread::from_value(serde_json::json!({
            "messages": [],
            "muted_until_at": "2099-01-01T00:00:00Z"
        }));
        assert!(thread.is_muted());
    }

    #[test]
    fn message_parses_reactions() {
        let thread = ChatThread::from_value(serde_json::json!({
            "messages": [{
                "id": 4,
                "body": "shipped",
                "reactions": [
                    { "user_id": "1", "emoji": "❤️" },
                    { "user_id": "2", "emoji": "❤️" },
                    { "user_id": "1", "emoji": "😂" },
                    { "broken": true }
                ]
            }]
        }));
        assert_eq!(thread.messages[0].reactions.len(), 3);
        assert_eq!(thread.messages[0].reactions[0].emoji, "❤️");
        let listed = MessageReaction::from_list(&serde_json::json!({
            "reactions": { "data": [{ "user_id": "9", "emoji": "✅" }] }
        }));
        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0].user_id, "9");
    }
}
