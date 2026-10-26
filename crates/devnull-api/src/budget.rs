use std::collections::VecDeque;
use std::time::{Duration, Instant};
use tokio::sync::Mutex;
use tokio::time::sleep;

/// Keeps the app under the API's 120 signed requests a minute.
pub struct RequestBudget {
    limit: usize,
    sent: Mutex<VecDeque<Instant>>,
}

impl RequestBudget {
    pub fn signed() -> Self {
        Self {
            limit: 110,
            sent: Mutex::new(VecDeque::new()),
        }
    }

    pub fn unsigned() -> Self {
        Self {
            limit: 55,
            sent: Mutex::new(VecDeque::new()),
        }
    }

    pub async fn take(&self) {
        loop {
            let mut sent = self.sent.lock().await;
            let now = Instant::now();
            while sent
                .front()
                .is_some_and(|t| now.duration_since(*t) >= Duration::from_secs(60))
            {
                sent.pop_front();
            }
            if sent.len() < self.limit {
                sent.push_back(now);
                return;
            }
            let oldest = sent[0];
            drop(sent);
            let wait = Duration::from_secs(60).saturating_sub(now.duration_since(oldest))
                + Duration::from_millis(50);
            sleep(wait).await;
        }
    }
}
