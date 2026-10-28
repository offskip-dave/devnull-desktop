use std::collections::HashSet;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use reqwest::header::HeaderValue;
use reqwest::multipart::{Form, Part};
use reqwest::{Client as Http, Method, Request, StatusCode};
use serde::de::DeserializeOwned;
use serde::Serialize;
use sha2::{Digest, Sha256};

use crate::budget::RequestBudget;
use crate::default_base_url;
use crate::error::{self, ApiError};
use crate::sign::{
    canonical_message, random_nonce, DeviceKey, JSON_NAMESPACE, MULTIPART_NAMESPACE,
};
use crate::target::target;
use crate::types::*;

pub const USER_AGENT: &str = "DevNull-Linux/0.1";

#[derive(Debug)]
pub struct Upload {
    pub data: Vec<u8>,
    pub filename: String,
    pub mime_type: String,
}

struct Inner {
    key: Option<DeviceKey>,
    http: Http,
    base: String,
    budget: RequestBudget,
}

#[derive(Clone)]
pub struct Client {
    inner: Arc<Inner>,
}

impl std::fmt::Debug for Client {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Client")
    }
}

impl Client {
    pub fn unsigned() -> Result<Self, ApiError> {
        Self::build(None, RequestBudget::unsigned())
    }

    pub fn signed(key: DeviceKey) -> Result<Self, ApiError> {
        Self::build(Some(key), RequestBudget::signed())
    }

    fn build(key: Option<DeviceKey>, budget: RequestBudget) -> Result<Self, ApiError> {
        let http = Http::builder()
            .user_agent(USER_AGENT)
            .timeout(std::time::Duration::from_secs(30))
            .build()
            .map_err(|_| ApiError::network())?;
        Ok(Self {
            inner: Arc::new(Inner {
                key,
                http,
                base: default_base_url(),
                budget,
            }),
        })
    }

    pub fn base_url(&self) -> &str {
        &self.inner.base
    }

    pub fn is_signed(&self) -> bool {
        self.inner.key.is_some()
    }

    pub fn fingerprint(&self) -> Option<String> {
        self.inner.key.as_ref().map(DeviceKey::fingerprint)
    }

    pub async fn me(&self) -> Result<CurrentUser, ApiError> {
        self.get("me", &[]).await
    }

    pub async fn redeem(&self, code: &str, verifier: &str) -> Result<CurrentUser, ApiError> {
        #[derive(Serialize)]
        struct Body<'a> {
            code: &'a str,
            code_verifier: &'a str,
        }
        self.send_json(
            Method::POST,
            "devices",
            &[],
            &Body {
                code,
                code_verifier: verifier,
            },
        )
        .await
    }

    pub async fn log(&self, kind: LogKind, cursor: Option<&str>) -> Result<LogPage, ApiError> {
        self.get(&format!("log/{}", kind.as_str()), &[("cursor", cursor)])
            .await
    }

    pub async fn commit(&self, id: &str) -> Result<CommitPage, ApiError> {
        let bytes = self
            .execute(self.signed_request(
                Method::GET,
                &format!("commits/{id}"),
                &[],
                None,
                JSON_NAMESPACE,
            )?)
            .await?;
        let value: serde_json::Value =
            serde_json::from_slice(&bytes).map_err(|_| ApiError::decode())?;
        CommitPage::from_value(value).map_err(|_| ApiError::decode())
    }

    pub async fn commit_link_preview(&self, id: &str) -> Result<Option<LinkPreview>, ApiError> {
        #[derive(serde::Deserialize)]
        struct Body {
            link_preview: Option<LinkPreview>,
        }
        if let Ok(value) = self
            .public_json(&format!("/commits/{id}/link-preview"))
            .await
        {
            if let Ok(body) = serde_json::from_value::<Body>(value) {
                return Ok(body.link_preview);
            }
        }
        let value: serde_json::Value = self.get(&format!("commits/{id}/link-preview"), &[]).await?;
        Ok(serde_json::from_value::<Body>(value)
            .ok()
            .and_then(|body| body.link_preview))
    }

    pub async fn replies(&self, id: &str, cursor: Option<&str>) -> Result<Page<Commit>, ApiError> {
        self.get(&format!("commits/{id}/replies"), &[("cursor", cursor)])
            .await
    }

    pub async fn award_pastel(&self, id: &str) -> Result<PastelAward, ApiError> {
        self.send_empty_json(&format!("commits/{id}/pasteis")).await
    }

    pub async fn report_commit(
        &self,
        id: &str,
        reason: &str,
        details: Option<&str>,
    ) -> Result<(), ApiError> {
        let mut body = serde_json::json!({ "reason": reason });
        if let Some(details) = details.map(str::trim).filter(|value| !value.is_empty()) {
            body["details"] = serde_json::json!(details);
        }
        self.send_json_empty(Method::POST, &format!("commits/{id}/report"), &body)
            .await
    }

    pub async fn create_commit(
        &self,
        body: &str,
        parent_id: Option<&str>,
        quoted_commit_id: Option<&str>,
        images: Vec<Upload>,
        poll: Option<(Vec<String>, i64)>,
        scheduled_at: Option<&str>,
    ) -> Result<Commit, ApiError> {
        let mut payload = serde_json::json!({ "body": body });
        if let Some(parent) = parent_id {
            payload["parent_id"] = serde_json::json!(parent);
        }
        if let Some(quoted) = quoted_commit_id {
            payload["quoted_commit_id"] = serde_json::json!(quoted);
        }
        if let Some((options, duration)) = poll {
            payload["poll"] = serde_json::json!({
                "options": options,
                "duration": duration,
            });
        }
        if let Some(at) = scheduled_at {
            payload["scheduled_at"] = serde_json::json!(at);
        }
        if images.is_empty() {
            self.send_json(Method::POST, "commits", &[], &payload).await
        } else {
            self.send_multipart(Method::POST, "commits", payload, "images", images)
                .await
        }
    }

    pub async fn star(&self, id: &str) -> Result<(), ApiError> {
        self.send_empty(Method::POST, &format!("commits/{id}/star"))
            .await
    }

    pub async fn unstar(&self, id: &str) -> Result<(), ApiError> {
        self.send_empty(Method::DELETE, &format!("commits/{id}/star"))
            .await
    }

    pub async fn user(&self, username: &str) -> Result<User, ApiError> {
        self.profile(username).await.map(|p| p.user)
    }

    pub async fn profile(&self, username: &str) -> Result<Profile, ApiError> {
        let bytes = self
            .execute(self.signed_request(
                Method::GET,
                &format!("users/{username}"),
                &[],
                None,
                JSON_NAMESPACE,
            )?)
            .await?;
        let value: serde_json::Value =
            serde_json::from_slice(&bytes).map_err(|_| ApiError::decode())?;
        Profile::from_value(value).map_err(|_| ApiError::decode())
    }

    /// The GitHub profile README (`login/login`). Tries the signed API first, then GitHub.
    /// `Ok(None)` when there is no README.
    pub async fn profile_readme(
        &self,
        handle: &str,
        github_login: &str,
    ) -> Result<Option<String>, ApiError> {
        if let Ok(request) = self.signed_request(
            Method::GET,
            &format!("users/{handle}/readme"),
            &[],
            None,
            JSON_NAMESPACE,
        ) {
            if let Ok(bytes) = self.execute(request).await {
                return readme_from_bytes(&bytes);
            }
        }
        self.fetch_github_readme(github_login).await
    }

    async fn fetch_github_readme(&self, login: &str) -> Result<Option<String>, ApiError> {
        let url = format!("https://api.github.com/repos/{login}/{login}/readme");
        let response = self
            .inner
            .http
            .get(&url)
            .header(reqwest::header::ACCEPT, "application/vnd.github.raw+json")
            .header(reqwest::header::USER_AGENT, USER_AGENT)
            .send()
            .await
            .map_err(|_| ApiError::network())?;
        match response.status() {
            StatusCode::OK => {
                let text = response.text().await.map_err(|_| ApiError::network())?;
                let text = text.trim().to_owned();
                Ok(if text.is_empty() { None } else { Some(text) })
            }
            StatusCode::NOT_FOUND => Ok(None),
            _ => Err(ApiError::network()),
        }
    }

    pub async fn sync_github(&self) -> Result<CurrentUser, ApiError> {
        let bytes = self
            .execute(self.signed_request(Method::PUT, "me/github", &[], None, JSON_NAMESPACE)?)
            .await?;
        decode_maybe_wrapped(bytes)
    }

    pub async fn set_lane(&self, lane: Option<&Lane>) -> Result<CurrentUser, ApiError> {
        let payload = match lane {
            None => serde_json::json!({ "lane": serde_json::Value::Null }),
            Some(Lane::Palette(index)) => serde_json::json!({ "lane": index }),
            Some(Lane::Custom(hex)) => serde_json::json!({ "lane": hex }),
        };
        self.send_json(Method::PUT, "me/lane", &[], &payload).await
    }

    pub async fn search(&self, q: &str) -> Result<SearchResults, ApiError> {
        if let Ok(bytes) = self
            .execute(self.signed_request(
                Method::GET,
                "search",
                &[("q", Some(q))],
                None,
                JSON_NAMESPACE,
            )?)
            .await
        {
            if let Ok(value) = serde_json::from_slice::<serde_json::Value>(&bytes) {
                let results = SearchResults::from_value(value);
                if !results.is_empty() {
                    return Ok(results);
                }
            }
        }
        self.public_search(q).await
    }

    pub async fn suggested(&self) -> Result<Vec<User>, ApiError> {
        let following = self.following_set(None, HashSet::new()).await;
        self.suggested_excluding(&following).await
    }

    pub async fn suggested_excluding(
        &self,
        following: &HashSet<String>,
    ) -> Result<Vec<User>, ApiError> {
        let mut users = User::not_followed(self.users_from_suggested_paths().await, following);
        if users.is_empty() {
            users = self.suggested_from_logs(following).await;
        }
        Ok(users)
    }

    async fn users_from_suggested_paths(&self) -> Vec<User> {
        for path in ["suggested", "suggested-users", "me/suggested"] {
            let Ok(request) = self.signed_request(Method::GET, path, &[], None, JSON_NAMESPACE)
            else {
                continue;
            };
            let Ok(bytes) = self.execute(request).await else {
                continue;
            };
            let Ok(value) = serde_json::from_slice::<serde_json::Value>(&bytes) else {
                continue;
            };
            let mut users = User::suggested_from_value(&value);
            if users.is_empty() {
                users = User::list_from_value(value);
            }
            if !users.is_empty() {
                return users;
            }
        }
        Vec::new()
    }

    async fn suggested_from_logs(&self, following: &HashSet<String>) -> Vec<User> {
        let mut commits = Vec::new();
        for kind in [LogKind::Trending, LogKind::Latest] {
            if let Ok(page) = self.log(kind, None).await {
                commits.extend(page.data);
            }
        }
        User::unique_unfollowed_authors(&commits, following)
            .into_iter()
            .take(12)
            .collect()
    }

    pub async fn following(&self) -> Result<Vec<User>, ApiError> {
        self.following_of(None).await
    }

    pub async fn following_of(&self, username: Option<&str>) -> Result<Vec<User>, ApiError> {
        let named = username
            .map(str::trim)
            .filter(|name| !name.is_empty())
            .map(|name| format!("users/{name}/following"));
        let mut last_ok = Vec::new();
        for path in following_paths(named.as_deref()) {
            match self.following_from(&path).await {
                Ok(users) if !users.is_empty() => return Ok(users),
                Ok(users) => last_ok = users,
                Err(_) => continue,
            }
        }
        Ok(last_ok)
    }

    pub async fn following_set(
        &self,
        username: Option<&str>,
        mut known: HashSet<String>,
    ) -> HashSet<String> {
        known.extend(User::following_names(
            &self.following_of(username).await.unwrap_or_default(),
        ));
        if let Ok(page) = self.log(LogKind::Following, None).await {
            for commit in page.data {
                if let Some(author) = commit.author {
                    let name = author.username.trim().to_ascii_lowercase();
                    if !name.is_empty() {
                        known.insert(name);
                    }
                }
            }
        }
        known
    }

    async fn following_from(&self, path: &str) -> Result<Vec<User>, ApiError> {
        let mut out = Vec::new();
        let mut cursor: Option<String> = None;
        for _ in 0..20 {
            let bytes = self
                .execute(self.signed_request(
                    Method::GET,
                    path,
                    &[("cursor", cursor.as_deref())],
                    None,
                    JSON_NAMESPACE,
                )?)
                .await?;
            let value: serde_json::Value =
                serde_json::from_slice(&bytes).map_err(|_| ApiError::decode())?;
            let next = serde_json::from_value::<Page<User>>(value.clone())
                .ok()
                .and_then(|page| page.next_cursor());
            let users = User::list_from_value(value);
            if users.is_empty() {
                break;
            }
            out.extend(users);
            match next {
                Some(next) if cursor.as_ref() != Some(&next) => cursor = Some(next),
                _ => break,
            }
        }
        Ok(out)
    }

    pub async fn live_following(&self) -> Result<Vec<User>, ApiError> {
        for path in ["live-following", "me/live-following"] {
            match self
                .execute(self.signed_request(Method::GET, path, &[], None, JSON_NAMESPACE)?)
                .await
            {
                Ok(bytes) => {
                    let value: serde_json::Value =
                        serde_json::from_slice(&bytes).map_err(|_| ApiError::decode())?;
                    return Ok(User::list_from_value(value)
                        .into_iter()
                        .filter(User::is_live)
                        .take(3)
                        .collect());
                }
                Err(err) if err.status == 404 => continue,
                Err(err) => return Err(err),
            }
        }
        Ok(Vec::new())
    }

    pub async fn related_commits(&self, id: &str) -> Result<Vec<Commit>, ApiError> {
        let value: serde_json::Value = self.get(&format!("commits/{id}/related"), &[]).await?;
        Ok(Commit::list_from_value(value))
    }

    pub async fn starred(&self, cursor: Option<&str>) -> Result<LogPage, ApiError> {
        let page: Page<Commit> = self.get("starred", &[("cursor", cursor)]).await?;
        Ok(LogPage {
            data: page.data,
            links: page.links,
            meta: page.meta,
            new_commits_count: None,
        })
    }

    pub async fn set_pasteis_mode(&self, enabled: bool) -> Result<CurrentUser, ApiError> {
        self.send_json(
            Method::PUT,
            "me/pasteis-mode",
            &[],
            &serde_json::json!({ "enabled": enabled }),
        )
        .await
    }

    pub async fn set_timezone(&self, timezone: Option<&str>) -> Result<CurrentUser, ApiError> {
        let payload = match timezone {
            Some(name) => serde_json::json!({ "timezone": name }),
            None => serde_json::json!({ "timezone": serde_json::Value::Null }),
        };
        self.send_json(Method::PUT, "me/timezone", &[], &payload)
            .await
    }

    pub async fn set_remotes(&self, enabled: bool) -> Result<CurrentUser, ApiError> {
        self.send_json(
            Method::PUT,
            "me/remotes",
            &[],
            &serde_json::json!({ "enabled": enabled }),
        )
        .await
    }

    pub async fn export(&self) -> Result<Vec<u8>, ApiError> {
        self.execute(self.signed_request(Method::GET, "me/export", &[], None, JSON_NAMESPACE)?)
            .await
    }

    pub async fn delete_account(&self) -> Result<(), ApiError> {
        self.send_empty(Method::DELETE, "me").await
    }

    pub async fn mutes(&self) -> Result<Vec<User>, ApiError> {
        self.user_collection("me/mutes").await
    }

    pub async fn blocks(&self) -> Result<Vec<User>, ApiError> {
        self.user_collection("me/blocks").await
    }

    pub async fn mute(&self, username: &str) -> Result<(), ApiError> {
        self.send_empty(Method::POST, &format!("users/{username}/mute"))
            .await
    }

    pub async fn unmute(&self, username: &str) -> Result<(), ApiError> {
        self.send_empty(Method::DELETE, &format!("users/{username}/mute"))
            .await
    }

    pub async fn block(&self, username: &str) -> Result<(), ApiError> {
        self.send_empty(Method::POST, &format!("users/{username}/block"))
            .await
    }

    pub async fn unblock(&self, username: &str) -> Result<(), ApiError> {
        self.send_empty(Method::DELETE, &format!("users/{username}/block"))
            .await
    }

    async fn user_collection(&self, path: &str) -> Result<Vec<User>, ApiError> {
        let bytes = self
            .execute(self.signed_request(Method::GET, path, &[], None, JSON_NAMESPACE)?)
            .await?;
        let value: serde_json::Value =
            serde_json::from_slice(&bytes).map_err(|_| ApiError::decode())?;
        Ok(User::list_from_value(value))
    }

    pub async fn user_commits(
        &self,
        username: &str,
        replies: bool,
        cursor: Option<&str>,
    ) -> Result<Page<Commit>, ApiError> {
        let replies = if replies { Some("1") } else { None };
        self.get(
            &format!("users/{username}/commits"),
            &[("replies", replies), ("cursor", cursor)],
        )
        .await
    }

    pub async fn follow(&self, username: &str) -> Result<(), ApiError> {
        self.send_empty(Method::POST, &format!("users/{username}/follow"))
            .await
    }

    pub async fn unfollow(&self, username: &str) -> Result<(), ApiError> {
        self.send_empty(Method::DELETE, &format!("users/{username}/follow"))
            .await
    }

    pub async fn notifications(
        &self,
        types: &std::collections::HashSet<NotificationFilter>,
        cursor: Option<&str>,
    ) -> Result<Page<Notification>, ApiError> {
        let type_q = NotificationFilter::query_param(types);
        let value: serde_json::Value = self
            .get(
                "notifications",
                &[("type", type_q.as_deref()), ("cursor", cursor)],
            )
            .await?;
        Ok(Notification::page_from_value(value))
    }

    pub async fn notification_group(
        &self,
        id: &str,
        types: &std::collections::HashSet<NotificationFilter>,
        cursor: Option<&str>,
    ) -> Result<Page<Notification>, ApiError> {
        let type_q = NotificationFilter::query_param(types);
        let value: serde_json::Value = self
            .get(
                &format!("notifications/{id}/group"),
                &[("type", type_q.as_deref()), ("cursor", cursor)],
            )
            .await?;
        Ok(Notification::page_from_value(value))
    }

    pub async fn remotes(
        &self,
        tab: InboxTab,
        before_id: Option<&str>,
        query: Option<&str>,
    ) -> Result<InboxPage, ApiError> {
        let query = query.map(str::trim).filter(|text| !text.is_empty());
        let value: serde_json::Value = self
            .get(
                "remotes",
                &[
                    ("tab", Some(tab.as_str())),
                    ("before", before_id),
                    ("q", query),
                ],
            )
            .await?;
        Ok(InboxPage::from_value(value))
    }

    pub async fn remote_candidates(
        &self,
        query: &str,
        for_kind: &str,
    ) -> Result<Vec<RemoteCandidate>, ApiError> {
        let value: serde_json::Value = self
            .get(
                "remotes/candidates",
                &[("q", Some(query)), ("for", Some(for_kind))],
            )
            .await?;
        Ok(RemoteCandidate::list_from_value(value))
    }

    pub async fn create_org(&self, name: &str, members: &[String]) -> Result<ThreadRef, ApiError> {
        let value: serde_json::Value = self
            .send_json(
                Method::POST,
                "remotes/orgs",
                &[],
                &serde_json::json!({ "name": name, "members": members }),
            )
            .await?;
        ThreadRef::from_created_org(&value).ok_or_else(ApiError::decode)
    }

    pub async fn set_remote_preferences(
        &self,
        from: &str,
        pull_receipts: bool,
    ) -> Result<CurrentUser, ApiError> {
        self.send_json(
            Method::PUT,
            "me/remote-preferences",
            &[],
            &serde_json::json!({ "from": from, "pull_receipts": pull_receipts }),
        )
        .await
    }

    pub async fn unread_counts(&self, remotes_enabled: bool) -> Result<(i64, i64), ApiError> {
        let bytes = self
            .execute(self.signed_request(Method::GET, "me", &[], None, JSON_NAMESPACE)?)
            .await?;
        let value: serde_json::Value =
            serde_json::from_slice(&bytes).map_err(|_| ApiError::decode())?;
        let (notifications, mut remotes) = unread_counts_from_value(&value);
        if remotes_enabled {
            if let Ok(page) = self.remotes(InboxTab::Open, None, None).await {
                remotes = Some(page.unread_count);
            }
        } else {
            remotes = Some(0);
        }
        Ok((notifications.unwrap_or(0), remotes.unwrap_or(0)))
    }

    pub async fn thread(&self, reference: &ThreadRef) -> Result<ChatThread, ApiError> {
        let path = match reference {
            ThreadRef::Direct { username } => format!("remotes/@{username}/messages"),
            ThreadRef::Org { id, .. } => format!("remotes/orgs/{id}/messages"),
        };
        let value: serde_json::Value = self.get(&path, &[]).await?;
        Ok(ChatThread::from_value(value))
    }

    pub async fn push(
        &self,
        reference: &ThreadRef,
        body: &str,
        client_id: &str,
        images: Vec<Upload>,
        reply_to_id: Option<i64>,
    ) -> Result<Message, ApiError> {
        let mut payload = serde_json::json!({ "body": body, "client_id": client_id });
        if let Some(id) = reply_to_id {
            payload["reply_to_id"] = serde_json::json!(id);
        }
        let path = match reference {
            ThreadRef::Direct { username } => format!("remotes/@{username}/messages"),
            ThreadRef::Org { id, .. } => format!("remotes/orgs/{id}/messages"),
        };
        if images.is_empty() {
            self.send_json(Method::POST, &path, &[], &payload).await
        } else {
            self.send_multipart(Method::POST, &path, payload, "images", images)
                .await
        }
    }

    pub async fn ping_typing(&self, reference: &ThreadRef) -> Result<(), ApiError> {
        let path = match reference {
            ThreadRef::Direct { username } => format!("remotes/@{username}/typing"),
            ThreadRef::Org { id, .. } => format!("remotes/orgs/{id}/typing"),
        };
        self.send_json_empty(Method::PUT, &path, &serde_json::json!({}))
            .await
    }

    pub async fn mark_read(&self, reference: &ThreadRef, through: i64) -> Result<(), ApiError> {
        let path = match reference {
            ThreadRef::Direct { username } => format!("remotes/@{username}/read"),
            ThreadRef::Org { id, .. } => format!("remotes/orgs/{id}/read"),
        };
        self.send_json_empty(
            Method::PUT,
            &path,
            &serde_json::json!({ "through": through }),
        )
        .await
    }

    pub async fn set_archived(
        &self,
        reference: &ThreadRef,
        archived: bool,
    ) -> Result<(), ApiError> {
        let path = match reference {
            ThreadRef::Direct { username } => format!("remotes/@{username}/archive"),
            ThreadRef::Org { id, .. } => format!("remotes/orgs/{id}/archive"),
        };
        self.send_empty(
            if archived {
                Method::POST
            } else {
                Method::DELETE
            },
            &path,
        )
        .await
    }

    pub async fn leave_org(&self, org_id: &str, username: &str) -> Result<(), ApiError> {
        self.send_empty(
            Method::DELETE,
            &format!("remotes/orgs/{org_id}/members/{username}"),
        )
        .await
    }

    async fn get<T: DeserializeOwned>(
        &self,
        path: &str,
        query: &[(&str, Option<&str>)],
    ) -> Result<T, ApiError> {
        self.execute(self.signed_request(Method::GET, path, query, None, JSON_NAMESPACE)?)
            .await
            .and_then(decode_maybe_wrapped)
    }

    async fn send_json<T: DeserializeOwned, B: Serialize>(
        &self,
        method: Method,
        path: &str,
        query: &[(&str, Option<&str>)],
        body: &B,
    ) -> Result<T, ApiError> {
        let bytes = serde_json::to_vec(body).map_err(|_| ApiError::decode())?;
        let mut request = self.signed_request(method, path, query, Some(&bytes), JSON_NAMESPACE)?;
        request.headers_mut().insert(
            reqwest::header::CONTENT_TYPE,
            HeaderValue::from_static("application/json"),
        );
        *request.body_mut() = Some(bytes.into());
        decode_maybe_wrapped(self.execute(request).await?)
    }

    async fn send_json_empty<B: Serialize>(
        &self,
        method: Method,
        path: &str,
        body: &B,
    ) -> Result<(), ApiError> {
        let bytes = serde_json::to_vec(body).map_err(|_| ApiError::decode())?;
        let mut request = self.signed_request(method, path, &[], Some(&bytes), JSON_NAMESPACE)?;
        request.headers_mut().insert(
            reqwest::header::CONTENT_TYPE,
            HeaderValue::from_static("application/json"),
        );
        *request.body_mut() = Some(bytes.into());
        self.execute(request).await.map(|_| ())
    }

    async fn send_empty(&self, method: Method, path: &str) -> Result<(), ApiError> {
        self.execute(self.signed_request(method, path, &[], None, JSON_NAMESPACE)?)
            .await
            .map(|_| ())
    }

    async fn send_empty_json<T: DeserializeOwned>(&self, path: &str) -> Result<T, ApiError> {
        let bytes = self
            .execute(self.signed_request(Method::POST, path, &[], None, JSON_NAMESPACE)?)
            .await?;
        decode_maybe_wrapped(bytes)
    }

    async fn send_multipart<T: DeserializeOwned>(
        &self,
        method: Method,
        path: &str,
        json: serde_json::Value,
        field: &str,
        files: Vec<Upload>,
    ) -> Result<T, ApiError> {
        let payload = serde_json::to_vec(&json).map_err(|_| ApiError::decode())?;
        let hashes: String = files
            .iter()
            .map(|f| hex::encode(Sha256::digest(&f.data)))
            .collect::<Vec<_>>()
            .join(",");

        let uri = target(path, &[]);
        let mut request =
            self.signed_request_uri(method, &uri, &payload, Some(&hashes), MULTIPART_NAMESPACE)?;

        let mut form = Form::new().part(
            "payload",
            Part::bytes(payload)
                .file_name("payload.json")
                .mime_str("application/json")
                .map_err(|_| ApiError::decode())?,
        );
        let use_array = files.len() > 1 || field == "images";
        for file in files {
            let name = if use_array {
                format!("{field}[]")
            } else {
                field.to_owned()
            };
            let mime = file.mime_type.clone();
            form = form.part(
                name,
                Part::bytes(file.data)
                    .file_name(file.filename)
                    .mime_str(&mime)
                    .map_err(|_| ApiError::decode())?,
            );
        }

        *request.body_mut() = None;
        let mut builder = self
            .inner
            .http
            .request(request.method().clone(), request.url().clone());
        for (k, v) in request.headers().iter() {
            builder = builder.header(k, v);
        }
        self.inner.budget.take().await;
        let response = builder
            .multipart(form)
            .send()
            .await
            .map_err(|_| ApiError::network())?;
        decode_maybe_wrapped(read_response(response).await?)
    }

    fn signed_request(
        &self,
        method: Method,
        path: &str,
        query: &[(&str, Option<&str>)],
        body: Option<&[u8]>,
        namespace: &str,
    ) -> Result<Request, ApiError> {
        let uri = target(path, query);
        self.signed_request_uri(method, &uri, body.unwrap_or(b""), None, namespace)
    }

    fn signed_request_uri(
        &self,
        method: Method,
        uri: &str,
        body: &[u8],
        file_hashes: Option<&str>,
        namespace: &str,
    ) -> Result<Request, ApiError> {
        let url = format!("{}{uri}", self.inner.base.trim_end_matches('/'));
        let mut request = self
            .inner
            .http
            .request(method.clone(), &url)
            .header(reqwest::header::ACCEPT, "application/json")
            .build()
            .map_err(|_| ApiError::network())?;

        if let Some(key) = &self.inner.key {
            let timestamp = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs()
                .to_string();
            let nonce = random_nonce();
            let message =
                canonical_message(&timestamp, &nonce, method.as_str(), uri, file_hashes, body);
            let signature = key.sign(&message, namespace);
            let headers = request.headers_mut();
            headers.insert("X-Devnull-Timestamp", header_value(&timestamp)?);
            headers.insert("X-Devnull-Nonce", header_value(&nonce)?);
            headers.insert("X-SSH-Signature", header_value(&signature)?);
        }

        Ok(request)
    }

    async fn execute(&self, request: Request) -> Result<Vec<u8>, ApiError> {
        self.inner.budget.take().await;
        let response = self
            .inner
            .http
            .execute(request)
            .await
            .map_err(|_| ApiError::network())?;
        read_response(response).await
    }

    async fn public_json(&self, path: &str) -> Result<serde_json::Value, ApiError> {
        let url = format!("{}{path}", self.inner.base.trim_end_matches('/'));
        self.inner.budget.take().await;
        let response = self
            .inner
            .http
            .get(url)
            .header(reqwest::header::ACCEPT, "application/json")
            .send()
            .await
            .map_err(|_| ApiError::network())?;
        let bytes = read_response(response).await?;
        serde_json::from_slice(&bytes).map_err(|_| ApiError::decode())
    }

    async fn public_search(&self, q: &str) -> Result<SearchResults, ApiError> {
        let encoded = percent_encoding::utf8_percent_encode(q, percent_encoding::NON_ALPHANUMERIC);
        let users = self
            .public_json(&format!("/search/users?q={encoded}"))
            .await
            .ok();
        let commits = self
            .public_json(&format!("/search/commits?q={encoded}"))
            .await
            .ok();
        let mut results = SearchResults::default();
        if let Some(value) = users {
            results.users = SearchResults::from_value(value).users;
        }
        if let Some(value) = commits {
            results.commits = SearchResults::from_value(value).commits;
        }
        Ok(results)
    }
}

async fn read_response(response: reqwest::Response) -> Result<Vec<u8>, ApiError> {
    let status = response.status();
    let bytes = response.bytes().await.map_err(|_| ApiError::network())?;
    if status.is_success() || status == StatusCode::NO_CONTENT {
        Ok(bytes.to_vec())
    } else {
        Err(error::from_body(status.as_u16(), &bytes))
    }
}

fn header_value(value: &str) -> Result<HeaderValue, ApiError> {
    HeaderValue::from_str(value).map_err(|_| ApiError::signing("Couldn't sign this request."))
}

fn readme_from_bytes(bytes: &[u8]) -> Result<Option<String>, ApiError> {
    if bytes.is_empty() {
        return Ok(None);
    }
    let value: serde_json::Value = serde_json::from_slice(bytes).map_err(|_| ApiError::decode())?;
    let data = value.get("data").unwrap_or(&value);
    if data.is_null() {
        return Ok(None);
    }
    if let Some(text) = data.as_str() {
        return Ok(nonempty_readme(text));
    }
    for key in ["readme", "body", "markdown", "html"] {
        if let Some(text) = data.get(key).and_then(|value| value.as_str()) {
            return Ok(nonempty_readme(text));
        }
    }
    Err(ApiError::decode())
}

fn nonempty_readme(text: &str) -> Option<String> {
    let text = text.trim();
    if text.is_empty() {
        None
    } else {
        Some(text.to_owned())
    }
}

fn decode_maybe_wrapped<T: DeserializeOwned>(bytes: Vec<u8>) -> Result<T, ApiError> {
    if bytes.is_empty() {
        return serde_json::from_str("null").map_err(|_| ApiError::decode());
    }
    let value: serde_json::Value =
        serde_json::from_slice(&bytes).map_err(|_| ApiError::decode())?;
    if let Some(data) = value.get("data") {
        if data.is_object() {
            if let Ok(parsed) = serde_json::from_value::<T>(merge_envelope(&value, data)) {
                return Ok(parsed);
            }
            if let Some(user) = data.get("user").filter(|user| user.is_object()) {
                if let Ok(parsed) = serde_json::from_value::<T>(merge_envelope(&value, user)) {
                    return Ok(parsed);
                }
            }
        }
    }
    serde_json::from_value(value).map_err(|_| ApiError::decode())
}

/// Copy envelope fields such as `unread_notifications_count` onto the inner object
/// when Laravel puts them beside `data` via `JsonResource::additional`.
fn following_paths(named: Option<&str>) -> Vec<String> {
    let mut paths = Vec::new();
    if let Some(path) = named {
        paths.push(path.to_owned());
    }
    paths.push("me/following".into());
    paths.push("following".into());
    paths
}

fn merge_envelope(envelope: &serde_json::Value, inner: &serde_json::Value) -> serde_json::Value {
    let mut merged = inner.clone();
    let Some(object) = merged.as_object_mut() else {
        return merged;
    };
    let Some(map) = envelope.as_object() else {
        return merged;
    };
    for (key, value) in map {
        if key != "data" {
            object.entry(key.clone()).or_insert_with(|| value.clone());
        }
    }
    merged
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn redeem_body_reads_handle_and_unread_count_beside_data() {
        let bytes = serde_json::to_vec(&serde_json::json!({
            "data": {
                "id": "01a13fa8-6cd4-70ef-b8e5-f46a3ca70ca8",
                "handle": "nunomaduro",
                "name": "Nuno Maduro",
                "created_at": "2026-10-15T13:02:44Z",
                "automatic_lane": 3,
                "remotes": { "from": "following", "unread_count": 2 }
            },
            "unread_notifications_count": 7
        }))
        .unwrap();

        let user: CurrentUser = decode_maybe_wrapped(bytes).unwrap();
        assert_eq!(user.username(), "nunomaduro");
        assert_eq!(user.unread_notifications(), 7);
        assert_eq!(user.unread_remotes(), 2);
    }

    #[test]
    fn readme_from_a_string_payload() {
        let bytes = serde_json::to_vec(&serde_json::json!({
            "data": "# Hello"
        }))
        .unwrap();
        assert_eq!(
            readme_from_bytes(&bytes).unwrap().as_deref(),
            Some("# Hello")
        );
    }

    #[test]
    fn empty_readme_is_missing() {
        let bytes = serde_json::to_vec(&serde_json::json!({ "data": null })).unwrap();
        assert_eq!(readme_from_bytes(&bytes).unwrap(), None);
    }

    #[test]
    fn following_list_prefers_the_native_user_path() {
        assert_eq!(
            following_paths(Some("users/ada/following")),
            ["users/ada/following", "me/following", "following"]
        );
        assert_eq!(following_paths(None), ["me/following", "following"]);
    }
}
