//! Device approval for the Linux client: Ed25519 key, PKCE, Secret Service, `devnull://` callbacks.

use std::collections::HashMap;

use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine;
use devnull_api::{default_base_url, DeviceKey};
use sha2::{Digest, Sha256};
use thiserror::Error;
use url::Url;

pub const APP_ID: &str = "sh.devnull.Client";
pub const REDIRECT_URI: &str = "devnull://linux/auth";
const KEYRING_LABEL: &str = "/dev/null device key";
const ATTR_APP: &str = "app";
const ATTR_ACCOUNT: &str = "account";
const ACCOUNT: &str = "device-key";

#[derive(Debug, Error)]
pub enum AuthError {
    #[error("Couldn't open your secret store. Start GNOME Keyring, KWallet, or KeePassXC.")]
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
        .unwrap_or_else(|| "Linux".into());
    let name = if host.to_ascii_lowercase().contains("linux") {
        host
    } else {
        format!("{host} Linux")
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

pub fn parse_callback(raw: &str) -> Result<Callback, AuthError> {
    let uri = extract_callback(raw).ok_or(AuthError::BadCallback)?;
    let url = Url::parse(&uri).map_err(|_| AuthError::BadCallback)?;
    if url.scheme() != "devnull" {
        return Err(AuthError::BadCallback);
    }
    let host = url.host_str().unwrap_or("");
    let path = url.path().trim_matches('/');
    if host != "linux" || (path != "auth" && !path.is_empty()) {
        return Err(AuthError::BadCallback);
    }
    let pairs: HashMap<_, _> = url.query_pairs().into_owned().collect();
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

/// Install a user desktop file so the browser can hand `devnull://` back to this binary.
pub fn register_url_handler() -> Result<(), AuthError> {
    if std::env::var_os("FLATPAK_ID").is_some() {
        return Ok(());
    }
    let exe = std::env::current_exe().map_err(|_| AuthError::Handler)?;
    let apps_dir = data_home().join("applications");
    std::fs::create_dir_all(&apps_dir).map_err(|_| AuthError::Handler)?;
    let desktop_path = apps_dir.join("sh.devnull.Client.desktop");
    let exec = quote_exec(&exe);
    let desktop = format!(
        "\
[Desktop Entry]
Type=Application
Name=/dev/null
Comment=A social network for developers
Exec={exec} %u
Icon=sh.devnull.Client
Terminal=false
Categories=Network;InstantMessaging;
MimeType=x-scheme-handler/devnull;
StartupWMClass=sh.devnull.Client
DBusActivatable=false
"
    );
    std::fs::write(&desktop_path, desktop).map_err(|_| AuthError::Handler)?;

    let _ = std::process::Command::new("update-desktop-database")
        .arg(&apps_dir)
        .status();
    let _ = std::process::Command::new("xdg-mime")
        .args([
            "default",
            "sh.devnull.Client.desktop",
            "x-scheme-handler/devnull",
        ])
        .status();
    let _ = std::process::Command::new("xdg-settings")
        .args([
            "set",
            "default-url-scheme-handler",
            "devnull",
            "sh.devnull.Client.desktop",
        ])
        .status();
    let _ = std::process::Command::new("gio")
        .args([
            "mime",
            "x-scheme-handler/devnull",
            "sh.devnull.Client.desktop",
        ])
        .status();
    write_mimeapps_default();
    Ok(())
}

fn data_home() -> std::path::PathBuf {
    xdg_home("XDG_DATA_HOME", ".local/share")
}

fn config_home() -> std::path::PathBuf {
    xdg_home("XDG_CONFIG_HOME", ".config")
}

fn xdg_home(var: &str, fallback: &str) -> std::path::PathBuf {
    if let Ok(dir) = std::env::var(var) {
        if !dir.is_empty() {
            return std::path::PathBuf::from(dir);
        }
    }
    std::path::PathBuf::from(std::env::var("HOME").unwrap_or_else(|_| ".".into())).join(fallback)
}

fn write_mimeapps_default() {
    let path = config_home().join("mimeapps.list");
    let _ = std::fs::create_dir_all(config_home());
    let mut text = std::fs::read_to_string(&path).unwrap_or_default();
    const KEY: &str = "x-scheme-handler/devnull=";
    const LINE: &str = "x-scheme-handler/devnull=sh.devnull.Client.desktop";
    if let Some(idx) = text.find(KEY) {
        let end = text[idx..]
            .find('\n')
            .map(|i| idx + i)
            .unwrap_or(text.len());
        text.replace_range(idx..end, LINE);
    } else if let Some(section) = text.find("[Default Applications]") {
        let insert_at = text[section..]
            .find('\n')
            .map(|i| section + i + 1)
            .unwrap_or(text.len());
        text.insert_str(insert_at, &format!("{LINE}\n"));
    } else {
        if !text.is_empty() && !text.ends_with('\n') {
            text.push('\n');
        }
        text.push_str(&format!("\n[Default Applications]\n{LINE}\n"));
    }
    let _ = std::fs::write(path, text);
}

fn quote_exec(path: &std::path::Path) -> String {
    let raw = path.display().to_string().replace('%', "%%");
    if raw
        .chars()
        .any(|c| c.is_whitespace() || "\"'\\><~|&;$*?#()`".contains(c))
    {
        format!("\"{}\"", raw.replace('\\', "\\\\").replace('"', "\\\""))
    } else {
        raw
    }
}

fn attributes() -> HashMap<&'static str, &'static str> {
    HashMap::from([(ATTR_APP, APP_ID), (ATTR_ACCOUNT, ACCOUNT)])
}

pub async fn load_key() -> Result<Option<DeviceKey>, AuthError> {
    let keyring = oo7::Keyring::new().await.map_err(|_| AuthError::Keyring)?;
    let items = keyring
        .search_items(&attributes())
        .await
        .map_err(|_| AuthError::Keyring)?;
    let Some(item) = items.into_iter().next() else {
        return Ok(None);
    };
    let secret = item.secret().await.map_err(|_| AuthError::Keyring)?;
    let bytes = secret.as_bytes();
    if bytes.len() != 32 {
        return Ok(None);
    }
    let mut key = [0u8; 32];
    key.copy_from_slice(bytes);
    Ok(Some(DeviceKey::from_bytes(&key)))
}

pub async fn load_or_create_key() -> Result<DeviceKey, AuthError> {
    if let Some(existing) = load_key().await? {
        return Ok(existing);
    }
    let key = DeviceKey::generate();
    save_key(&key).await?;
    Ok(key)
}

pub async fn save_key(key: &DeviceKey) -> Result<(), AuthError> {
    let keyring = oo7::Keyring::new().await.map_err(|_| AuthError::Keyring)?;
    keyring
        .create_item(KEYRING_LABEL, &attributes(), &key.to_bytes(), true)
        .await
        .map_err(|_| AuthError::Keyring)?;
    Ok(())
}

pub async fn delete_key() -> Result<(), AuthError> {
    let keyring = oo7::Keyring::new().await.map_err(|_| AuthError::Keyring)?;
    let items = keyring
        .search_items(&attributes())
        .await
        .map_err(|_| AuthError::Keyring)?;
    for item in items {
        let _ = item.delete().await;
    }
    Ok(())
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
    fn authorize_url_encodes_plus() {
        let key = DeviceKey::generate();
        let pkce = Pkce::new();
        let url = authorize_url(&key, &pkce);
        assert!(url.contains("code_challenge_method=S256"));
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
