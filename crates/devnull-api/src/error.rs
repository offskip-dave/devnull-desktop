use std::fmt;

/// An error the API answered with, or a failure to reach it.
#[derive(Debug, Clone)]
pub struct ApiError {
    pub status: u16,
    pub code: Option<String>,
    pub message: String,
}

impl ApiError {
    pub fn network() -> Self {
        Self {
            status: 0,
            code: None,
            message: "Couldn't reach /dev/null. Check your connection.".into(),
        }
    }

    pub fn decode() -> Self {
        Self {
            status: 200,
            code: None,
            message: "/dev/null answered in a way this version of the app doesn't understand. Check for an update."
                .into(),
        }
    }

    pub fn signing(message: impl Into<String>) -> Self {
        Self {
            status: 0,
            code: Some("invalid_signature".into()),
            message: message.into(),
        }
    }

    /// The device was revoked, or the key was never approved.
    pub fn requires_sign_in(&self) -> bool {
        self.code.as_deref() == Some("device_not_approved")
            || (self.code.as_deref() == Some("invalid_signature") && self.status == 401)
    }

    pub fn is_commit_rules(&self) -> bool {
        self.status == 403 && self.message.to_ascii_lowercase().contains("commit")
    }

    /// The chosen organization author was refused (removed while composing, etc.).
    pub fn is_author_error(&self) -> bool {
        let message = self.message.to_ascii_lowercase();
        message.contains("organization")
            || message.contains("post as")
            || message.contains("author")
            || self.code.as_deref() == Some("author")
    }

    /// Laravel's throttle and similar 429 replies. Background work should stay quiet.
    pub fn is_rate_limited(&self) -> bool {
        if self.status == 429 {
            return true;
        }
        let message = self.message.to_ascii_lowercase();
        message.contains("too many attempts") || message.contains("too many requests")
    }
}

impl fmt::Display for ApiError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for ApiError {}

impl From<reqwest::Error> for ApiError {
    fn from(_: reqwest::Error) -> Self {
        Self::network()
    }
}

pub fn from_body(status: u16, data: &[u8]) -> ApiError {
    let json: serde_json::Value = serde_json::from_slice(data).unwrap_or(serde_json::Value::Null);
    let code = json.get("code").and_then(|v| v.as_str()).map(str::to_owned);

    let errors = json.get("errors").and_then(|errors| errors.as_object());
    let author_message = errors
        .and_then(|map| map.get("author"))
        .and_then(|v| v.as_array())
        .and_then(|arr| arr.first())
        .and_then(|v| v.as_str())
        .map(str::to_owned);
    let first_error = errors
        .and_then(|map| map.values().next())
        .and_then(|v| v.as_array())
        .and_then(|arr| arr.first())
        .and_then(|v| v.as_str())
        .map(str::to_owned);
    let message = author_message
        .clone()
        .or(first_error)
        .or_else(|| {
            json.get("message")
                .and_then(|v| v.as_str())
                .map(str::to_owned)
        })
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| {
            if status == 0 {
                "Couldn't reach /dev/null. Check your connection.".into()
            } else {
                "Something went wrong.".into()
            }
        });
    let code = if author_message.is_some() {
        Some("author".into())
    } else {
        code
    };

    ApiError {
        status,
        code,
        message,
    }
}

#[cfg(test)]
mod tests {
    use super::ApiError;

    #[test]
    fn rate_limit_matches_laravel_copy() {
        let err = ApiError {
            status: 429,
            code: None,
            message: "Too Many Attempts.".into(),
        };
        assert!(err.is_rate_limited());
        let copy = ApiError {
            status: 200,
            code: None,
            message: "Too many attempts.".into(),
        };
        assert!(copy.is_rate_limited());
        assert!(!ApiError::network().is_rate_limited());
    }
}
