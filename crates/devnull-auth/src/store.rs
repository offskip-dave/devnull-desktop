//! Device-key storage: Secret Service on Linux, Keychain on macOS.

use super::{AuthError, ACCOUNT, APP_ID};
use base64::engine::general_purpose::STANDARD;
use base64::Engine;
use devnull_api::DeviceKey;

pub(crate) fn key_from_bytes(bytes: &[u8]) -> Option<DeviceKey> {
    if bytes.len() == 32 {
        let mut key = [0u8; 32];
        key.copy_from_slice(bytes);
        return Some(DeviceKey::from_bytes(&key));
    }
    let text = std::str::from_utf8(bytes).ok()?.trim();
    let decoded = STANDARD.decode(text).ok()?;
    if decoded.len() != 32 {
        return None;
    }
    let mut key = [0u8; 32];
    key.copy_from_slice(&decoded);
    Some(DeviceKey::from_bytes(&key))
}

#[cfg(not(any(target_os = "linux", target_os = "macos")))]
compile_error!("devnull-auth supports Linux and macOS");

pub async fn load_or_create_key() -> Result<DeviceKey, AuthError> {
    if let Some(existing) = load_key().await? {
        return Ok(existing);
    }
    let key = DeviceKey::generate();
    save_key(&key).await?;
    Ok(key)
}

#[cfg(target_os = "linux")]
mod linux {
    use super::*;
    use std::collections::HashMap;

    const KEYRING_LABEL: &str = "/dev/null device key";
    const ATTR_APP: &str = "app";
    const ATTR_ACCOUNT: &str = "account";

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
        Ok(key_from_bytes(secret.as_bytes()))
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
}

#[cfg(target_os = "macos")]
mod macos {
    use super::*;
    use security_framework::passwords::{
        delete_generic_password, get_generic_password, set_generic_password,
    };

    const SERVICE: &str = APP_ID;
    const ERR_SEC_ITEM_NOT_FOUND: i32 = -25300;

    fn not_found(err: &security_framework::base::Error) -> bool {
        err.code() == ERR_SEC_ITEM_NOT_FOUND
    }

    #[allow(clippy::unused_async)]
    pub async fn load_key() -> Result<Option<DeviceKey>, AuthError> {
        match get_generic_password(SERVICE, ACCOUNT) {
            Ok(bytes) => Ok(key_from_bytes(&bytes)),
            Err(err) if not_found(&err) => Ok(None),
            Err(_) => Err(AuthError::Keyring),
        }
    }

    #[allow(clippy::unused_async)]
    pub async fn save_key(key: &DeviceKey) -> Result<(), AuthError> {
        let encoded = STANDARD.encode(key.to_bytes());
        set_generic_password(SERVICE, ACCOUNT, encoded.as_bytes()).map_err(|_| AuthError::Keyring)
    }

    #[allow(clippy::unused_async)]
    pub async fn delete_key() -> Result<(), AuthError> {
        match delete_generic_password(SERVICE, ACCOUNT) {
            Ok(()) => Ok(()),
            Err(err) if not_found(&err) => Ok(()),
            Err(_) => Err(AuthError::Keyring),
        }
    }
}

#[cfg(target_os = "linux")]
pub use linux::{delete_key, load_key, save_key};

#[cfg(target_os = "macos")]
pub use macos::{delete_key, load_key, save_key};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_linux_raw_secret_and_macos_base64_secret() {
        let key = DeviceKey::generate();
        let raw = key.to_bytes();
        assert_eq!(
            key_from_bytes(&raw).unwrap().fingerprint(),
            key.fingerprint()
        );
        let encoded = STANDARD.encode(raw);
        assert_eq!(
            key_from_bytes(encoded.as_bytes()).unwrap().fingerprint(),
            key.fingerprint()
        );
    }
}
