//! In-flight sign-in: PKCE and the device key, plus the `devnull://` callback the
//! macOS helper writes so the waiting instance can finish redeem without D-Bus.

use std::path::PathBuf;

use base64::engine::general_purpose::{STANDARD, URL_SAFE_NO_PAD};
use base64::Engine;
use sha2::{Digest, Sha256};

use super::{Pkce, APP_ID};
use devnull_api::DeviceKey;

const PENDING_FILE: &str = "pending-auth";
const CALLBACK_FILE: &str = "callback.uri";

pub fn data_dir() -> PathBuf {
    if let Ok(dir) = std::env::var("DEVNULL_DATA_DIR") {
        if !dir.is_empty() {
            return PathBuf::from(dir);
        }
    }
    if cfg!(target_os = "macos") {
        PathBuf::from(std::env::var("HOME").unwrap_or_else(|_| ".".into()))
            .join("Library/Application Support")
            .join(APP_ID)
    } else {
        let base = std::env::var("XDG_DATA_HOME")
            .ok()
            .filter(|s| !s.is_empty());
        let root = match base {
            Some(dir) => PathBuf::from(dir),
            None => PathBuf::from(std::env::var("HOME").unwrap_or_else(|_| ".".into()))
                .join(".local/share"),
        };
        root.join(APP_ID)
    }
}

pub fn save_pending(pkce: &Pkce, key: &DeviceKey) {
    let _ = std::fs::create_dir_all(data_dir());
    let path = data_dir().join(PENDING_FILE);
    let text = format!(
        "v1\n{}\n{}\n{}\n",
        pkce.verifier,
        pkce.state,
        STANDARD.encode(key.to_bytes())
    );
    let _ = std::fs::write(&path, text);
    restrict_owner(&path);
}

pub fn load_pending() -> Option<(Pkce, DeviceKey)> {
    let text = std::fs::read_to_string(data_dir().join(PENDING_FILE)).ok()?;
    decode_pending(&text)
}

pub fn take_pending() -> Option<(Pkce, DeviceKey)> {
    let path = data_dir().join(PENDING_FILE);
    let text = std::fs::read_to_string(&path).ok()?;
    let _ = std::fs::remove_file(&path);
    decode_pending(&text)
}

pub fn clear_pending() {
    let _ = std::fs::remove_file(data_dir().join(PENDING_FILE));
    let _ = std::fs::remove_file(data_dir().join(CALLBACK_FILE));
}

pub fn write_callback_uri(uri: &str) {
    let _ = std::fs::create_dir_all(data_dir());
    let path = data_dir().join(CALLBACK_FILE);
    let _ = std::fs::write(&path, uri);
    restrict_owner(&path);
}

fn restrict_owner(path: &std::path::Path) {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600));
    }
    #[cfg(not(unix))]
    {
        let _ = path;
    }
}

pub fn take_callback_uri() -> Option<String> {
    let path = data_dir().join(CALLBACK_FILE);
    let text = std::fs::read_to_string(&path).ok()?;
    let _ = std::fs::remove_file(&path);
    let uri = text.trim();
    if uri.is_empty() {
        None
    } else {
        Some(uri.to_owned())
    }
}

fn decode_pending(text: &str) -> Option<(Pkce, DeviceKey)> {
    let mut lines = text.lines();
    if lines.next()? != "v1" {
        return None;
    }
    let verifier = lines.next()?.trim().to_owned();
    let state = lines.next()?.trim().to_owned();
    let key_line = lines.next()?.trim();
    if verifier.is_empty() || state.is_empty() {
        return None;
    }
    let key_bytes = STANDARD.decode(key_line).ok()?;
    if key_bytes.len() != 32 {
        return None;
    }
    let mut raw = [0u8; 32];
    raw.copy_from_slice(&key_bytes);
    let challenge = URL_SAFE_NO_PAD.encode(Sha256::digest(verifier.as_bytes()));
    Some((
        Pkce {
            verifier,
            challenge,
            state,
        },
        DeviceKey::from_bytes(&raw),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    static ENV: Mutex<()> = Mutex::new(());

    fn with_data_dir<T>(f: impl FnOnce() -> T) -> T {
        let _guard = ENV.lock().unwrap_or_else(|err| err.into_inner());
        let dir = std::env::temp_dir().join(format!("devnull-auth-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let previous = std::env::var("DEVNULL_DATA_DIR").ok();
        // Safety: tests serialize on ENV, so this process is the only writer.
        unsafe {
            std::env::set_var("DEVNULL_DATA_DIR", &dir);
        }
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(f));
        unsafe {
            match previous {
                Some(value) => std::env::set_var("DEVNULL_DATA_DIR", value),
                None => std::env::remove_var("DEVNULL_DATA_DIR"),
            }
        }
        let _ = std::fs::remove_dir_all(&dir);
        result.unwrap()
    }

    #[test]
    fn pending_roundtrip_keeps_verifier_and_key() {
        let key = DeviceKey::generate();
        let pkce = Pkce::new();
        let text = format!(
            "v1\n{}\n{}\n{}\n",
            pkce.verifier,
            pkce.state,
            STANDARD.encode(key.to_bytes())
        );
        let (loaded, loaded_key) = decode_pending(&text).unwrap();
        assert_eq!(loaded.verifier, pkce.verifier);
        assert_eq!(loaded.state, pkce.state);
        assert_eq!(loaded.challenge, pkce.challenge);
        assert_eq!(loaded_key.fingerprint(), key.fingerprint());
    }

    #[test]
    fn pending_file_roundtrip_works_on_this_os() {
        with_data_dir(|| {
            let key = DeviceKey::generate();
            let pkce = Pkce::new();
            save_pending(&pkce, &key);
            write_callback_uri("devnull://linux/auth?code=abc&state=xyz");
            assert_eq!(
                take_callback_uri().as_deref(),
                Some("devnull://linux/auth?code=abc&state=xyz")
            );
            let (loaded, loaded_key) = take_pending().unwrap();
            assert_eq!(loaded.verifier, pkce.verifier);
            assert_eq!(loaded.state, pkce.state);
            assert_eq!(loaded_key.fingerprint(), key.fingerprint());
            assert!(take_pending().is_none());
        });
    }

    #[test]
    fn data_dir_is_under_the_platform_app_folder() {
        let _guard = ENV.lock().unwrap_or_else(|err| err.into_inner());
        if std::env::var("DEVNULL_DATA_DIR").is_ok() {
            return;
        }
        let dir = data_dir();
        assert_eq!(dir.file_name().and_then(|name| name.to_str()), Some(APP_ID));
        let text = dir.to_string_lossy();
        if cfg!(target_os = "macos") {
            assert!(
                text.contains("Application Support"),
                "macOS data dir should be Application Support: {text}"
            );
        } else {
            assert!(
                !text.contains("Application Support"),
                "Linux data dir should follow XDG, not Application Support: {text}"
            );
        }
    }
}
