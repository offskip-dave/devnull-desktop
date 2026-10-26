//! Signed and unsigned `/api/v1` client for /dev/null.
//!
//! Request signatures match the Mac app (`APIClient.swift`) and
//! `App\Services\SignedRequests` on the server.

mod budget;
mod client;
mod date;
mod error;
mod sign;
mod target;
pub mod types;

pub use client::{Client, Upload, USER_AGENT};
pub use error::ApiError;
pub use sign::{DeviceKey, JSON_NAMESPACE, MULTIPART_NAMESPACE};
pub use target::target;
pub use types::*;

pub fn default_base_url() -> String {
    std::env::var("DEVNULL_URL").unwrap_or_else(|_| "https://dev-null.sh".to_owned())
}
