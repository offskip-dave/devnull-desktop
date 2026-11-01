//! Device approval for the GTK client: Ed25519 key, PKCE, platform secret store, `devnull://` callbacks.

mod handler;
mod pending;
mod store;

use std::collections::HashMap;

use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine;
use devnull_api::{default_base_url, DeviceKey};
use sha2::{Digest, Sha256};
use thiserror::Error;
use url::Url;

pub use handler::register_url_handler;
pub use pending::{
    clear_pending, data_dir, load_pending, save_pending, take_callback_uri, take_pending,
    write_callback_uri,
};
pub use store::{delete_key, load_key, load_or_create_key, save_key};

/// GApplication / desktop / keyring id. Not the authorize `app_id` (that needs `platform:bundle`).
pub const APP_ID: &str = "sh.devnull.Client";
/// Server `app_id`: platform and bundle, e.g. `macos:sh.dev-null.mac`.
#[cfg(target_os = "macos")]
pub const AUTH_APP_ID: &str = "macos:sh.devnull.Client";
#[cfg(not(target_os = "macos"))]
pub const AUTH_APP_ID: &str = "linux:sh.devnull.Client";
/// This GTK client's callback. Kept as `linux/auth` on every OS so the server
/// still recognizes this client (the first-party Mac app uses a different URI).
pub const REDIRECT_URI: &str = "devnull://linux/auth";
const ACCOUNT: &str = "device-key";

#[cfg(target_os = "macos")]
pub const APP_NAME: &str = "/dev/null for macOS";
#[cfg(not(target_os = "macos"))]
pub const APP_NAME: &str = "/dev/null for Linux";

#[cfg(target_os = "macos")]
pub const PLATFORM: &str = "macOS";
#[cfg(not(target_os = "macos"))]
pub const PLATFORM: &str = "Linux";

#[derive(Debug, Error)]
pub enum AuthError {
    #[cfg_attr(
        target_os = "macos",
        error("Couldn't open the Keychain to store this device key.")
    )]
    #[cfg_attr(
        not(target_os = "macos"),
        error("Couldn't open your secret store. Start GNOME Keyring, KWallet, or KeePassXC.")
    )]
    Keyring,
    #[error("The callback URL isn't a /dev/null sign-in.")]
    BadCallback,
    #[error("Couldn't register this app to handle devnull:// links.")]
    Handler,
    #[error("{0}")]
    Message(String),
}

#[derive(Clone, Debug)]
pub struct Pkce {
    pub verifier: String,
    pub challenge: String,
    pub state: String,
}

impl Pkce {
    pub fn new() -> Self {
        let bytes: [u8; 32] = rand::random();
        let verifier = URL_SAFE_NO_PAD.encode(bytes);
        let challenge = URL_SAFE_NO_PAD.encode(Sha256::digest(verifier.as_bytes()));
        Self {
            verifier,
            challenge,
            state: uuid::Uuid::new_v4().to_string(),
        }
    }
}

impl Default for Pkce {
    fn default() -> Self {
        Self::new()
    }
}

pub fn device_name() -> String {
    let host = hostname::get()
        .ok()
        .and_then(|h| h.into_string().ok())
        .unwrap_or_else(|| PLATFORM.into());
    let needle = PLATFORM.to_ascii_lowercase();
    let name = if host.to_ascii_lowercase().contains(&needle) {
        host
    } else {
        format!("{host} {PLATFORM}")
    };
    name.chars().take(100).collect()
}

pub fn authorize_url(key: &DeviceKey, pkce: &Pkce) -> String {
    let mut url = Url::parse(&format!(
        "{}/mobile/authorize",
        default_base_url().trim_end_matches('/')
    ))
    .expect("base url");
    url.query_pairs_mut()
        .append_pair("fingerprint", &key.fingerprint())
        .append_pair("device_name", &device_name())
        .append_pair("app_name", APP_NAME)
        .append_pair("app_id", AUTH_APP_ID)
        .append_pair("code_challenge", &pkce.challenge)
        .append_pair("code_challenge_method", "S256")
        .append_pair("state", &pkce.state)
        .append_pair("redirect_uri", REDIRECT_URI);
    url.as_str().replace('+', "%2B")
}

#[derive(Debug, Clone)]
pub struct Callback {
    pub code: Option<String>,
    pub state: Option<String>,
    pub error: Option<String>,
}

fn percent_decode(raw: &str) -> String {
    let bytes = raw.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            if let Some(value) = std::str::from_utf8(&bytes[i + 1..i + 3])
                .ok()
                .and_then(|hex| u8::from_str_radix(hex, 16).ok())
            {
                out.push(value);
                i += 3;
                continue;
            }
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8(out).unwrap_or_else(|_| raw.to_owned())
}

pub fn parse_callback(raw: &str) -> Result<Callback, AuthError> {
    let uri = extract_callback(raw).ok_or(AuthError::BadCallback)?;
    let uri = if uri.contains('?') {
        uri
    } else {
        percent_decode(&uri)
    };
    let url = Url::parse(&uri).map_err(|_| AuthError::BadCallback)?;
    if url.scheme() != "devnull" {
        return Err(AuthError::BadCallback);
    }
    let host = url.host_str().unwrap_or("");
    let raw_path = url.path().trim_matches('/');
    let (path, path_query) = raw_path
        .split_once('?')
        .map(|(path, query)| (path, Some(query)))
        .unwrap_or((raw_path, None));
    if host != "linux" || (path != "auth" && !path.is_empty()) {
        return Err(AuthError::BadCallback);
    }
    let mut pairs: HashMap<_, _> = url.query_pairs().into_owned().collect();
    if pairs.is_empty() {
        if let Some(query) = path_query.or(url.fragment()) {
            pairs = Url::parse(&format!("devnull://linux/auth?{query}"))
                .map_err(|_| AuthError::BadCallback)?
                .query_pairs()
                .into_owned()
                .collect();
        }
    }
    Ok(Callback {
        code: pairs.get("code").cloned(),
        state: pairs.get("state").cloned(),
        error: pairs.get("error").cloned(),
    })
}

/// Pull a `devnull://linux/auth…` URL out of a larger string (clipboard, pasted text).
pub fn extract_callback(text: &str) -> Option<String> {
    let start = text.find("devnull://")?;
    let rest = &text[start..];
    let end = rest
        .find(|c: char| c.is_whitespace() || matches!(c, '"' | '\'' | '<' | '>' | ')'))
        .unwrap_or(rest.len());
    let uri = rest[..end].trim_end_matches(['.', ',', ';']);
    if uri.starts_with("devnull://linux/auth") {
        Some(uri.to_owned())
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_linux_callback() {
        let cb = parse_callback("devnull://linux/auth?code=abc&state=xyz").unwrap();
        assert_eq!(cb.code.as_deref(), Some("abc"));
        assert_eq!(cb.state.as_deref(), Some("xyz"));
    }

    #[test]
    fn parses_callback_when_query_was_percent_encoded_into_the_path() {
        let cb = parse_callback("devnull://linux/auth%3Fcode%3Dabc%26state%3Dxyz").unwrap();
        assert_eq!(cb.code.as_deref(), Some("abc"));
        assert_eq!(cb.state.as_deref(), Some("xyz"));
    }

    #[test]
    fn extracts_callback_from_surrounding_text() {
        let text = "Firefox can't find\ndevnull://linux/auth?code=abc&state=xyz.\n";
        assert_eq!(
            extract_callback(text).as_deref(),
            Some("devnull://linux/auth?code=abc&state=xyz")
        );
        assert!(parse_callback(text).is_ok());
    }

    #[test]
    fn rejects_other_hosts() {
        assert!(parse_callback("devnull://mac/auth?code=abc").is_err());
        assert!(parse_callback("https://dev-null.sh/").is_err());
    }

    #[test]
    fn auth_app_id_is_platform_and_bundle() {
        assert!(AUTH_APP_ID
            .chars()
            .next()
            .is_some_and(|c| c.is_ascii_lowercase()));
        assert!(AUTH_APP_ID.contains(':'));
        let (platform, bundle) = AUTH_APP_ID.split_once(':').unwrap();
        assert!(platform.chars().all(|c| c.is_ascii_lowercase()));
        assert_eq!(bundle, APP_ID);
        assert!(bundle
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-')));
    }

    #[test]
    fn authorize_url_uses_this_os_app_id() {
        let url = authorize_url(&DeviceKey::generate(), &Pkce::new());
        #[cfg(target_os = "linux")]
        assert!(
            url.contains("app_id=linux%3Ash.devnull.Client")
                || url.contains("app_id=linux:sh.devnull.Client"),
            "Linux authorize app_id must stay linux:sh.devnull.Client: {url}"
        );
        #[cfg(target_os = "macos")]
        assert!(
            url.contains("app_id=macos%3Ash.devnull.Client")
                || url.contains("app_id=macos:sh.devnull.Client"),
            "macOS authorize app_id must stay macos:sh.devnull.Client: {url}"
        );
        assert!(url.contains("redirect_uri=devnull%3A%2F%2Flinux%2Fauth"));
    }

    #[test]
    fn authorize_url_encodes_plus() {
        let key = DeviceKey::generate();
        let pkce = Pkce::new();
        let url = authorize_url(&key, &pkce);
        assert!(url.contains("code_challenge_method=S256"));
        assert!(url.contains("app_name="));
        assert!(
            url.contains("app_id=linux%3Ash.devnull.Client")
                || url.contains("app_id=macos%3Ash.devnull.Client")
                || url.contains(&format!("app_id={AUTH_APP_ID}"))
        );
        assert!(
            url.contains(PLATFORM),
            "authorize URL should name this platform ({PLATFORM}): {url}"
        );
        assert!(
            url.contains("redirect_uri=devnull%3A%2F%2Flinux%2Fauth")
                || url.contains("devnull://linux/auth")
        );
        assert!(
            !url.contains(&format!("fingerprint={}", key.fingerprint()))
                || !key.fingerprint().contains('+')
                || url.contains("%2B")
        );
    }
}
