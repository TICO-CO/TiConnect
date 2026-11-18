use std::collections::BTreeMap;

use ticonnect_settings::Appearance;

pub use ticonnect_core::ipc::events::{
    Call, Candidate, ConversationSummary, DaemonEvent, Identity, Message, MessageState,
    RemoteSession, Status, User, UserPolicy,
};

use crate::error::{Error, Result};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Backoff {
    current_ms: u64,
}

impl Default for Backoff {
    fn default() -> Self {
        Self { current_ms: 1_000 }
    }
}

impl Backoff {
    pub fn next_delay(&mut self) -> u64 {
        let delay = self.current_ms;
        self.current_ms = (self.current_ms * 2).min(10_000);
        delay
    }

    pub fn reset(&mut self) {
        self.current_ms = 1_000;
    }

    pub fn current_ms(self) -> u64 {
        self.current_ms
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct BlockedUser {
    pub fingerprint: String,
    pub node_id: String,
    pub display_name: String,
    pub blocked_at: u64,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SettingsForm {
    values: BTreeMap<String, String>,
}

impl SettingsForm {
    pub fn new(values: BTreeMap<String, String>) -> Self {
        Self { values }
    }

    pub fn replace(&mut self, values: BTreeMap<String, String>) {
        self.values = values;
    }

    pub fn values(&self) -> &BTreeMap<String, String> {
        &self.values
    }

    pub fn get(&self, key: &str) -> Option<&str> {
        self.values.get(key).map(String::as_str)
    }

    pub fn bool_value(&self, key: &str, default: bool) -> bool {
        self.get(key)
            .and_then(|value| value.parse::<bool>().ok())
            .unwrap_or(default)
    }

    pub fn u32_value(&self, key: &str, default: u32) -> u32 {
        self.get(key)
            .and_then(|value| value.parse::<u32>().ok())
            .unwrap_or(default)
    }

    pub fn appearance(&self) -> Appearance {
        self.get("appearance.color_scheme")
            .and_then(Appearance::parse)
            .unwrap_or_default()
    }

    pub fn validate_outbox_ttl(value: u32) -> Result<u32> {
        if (1..=365).contains(&value) {
            Ok(value)
        } else {
            Err(Error::InvalidResponse(
                "outbox TTL must be between 1 and 365 days".to_owned(),
            ))
        }
    }

    pub fn validate_beacon_interval(value: u32) -> Result<u32> {
        if (1..=3600).contains(&value) {
            Ok(value)
        } else {
            Err(Error::InvalidResponse(
                "beacon interval must be between 1 and 3600 seconds".to_owned(),
            ))
        }
    }
}

pub fn activation_view(value: &str) -> Result<&'static str> {
    match value.trim().to_ascii_lowercase().as_str() {
        "" | "chat" => Ok("chat"),
        "users" => Ok("users"),
        "settings" => Ok("settings"),
        "remote" => Ok("remote"),
        "watch-party" => Ok("watch-party"),
        other => Err(Error::InvalidResponse(format!(
            "unknown activation view: {other}"
        ))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn backoff_doubles_and_caps() {
        let mut backoff = Backoff::default();
        assert_eq!(backoff.next_delay(), 1_000);
        assert_eq!(backoff.next_delay(), 2_000);
        assert_eq!(backoff.next_delay(), 4_000);
        assert_eq!(backoff.next_delay(), 8_000);
        assert_eq!(backoff.next_delay(), 10_000);
        assert_eq!(backoff.next_delay(), 10_000);
        backoff.reset();
        assert_eq!(backoff.current_ms(), 1_000);
    }

    #[test]
    fn activation_views_are_stable() {
        assert_eq!(activation_view("").expect("default"), "chat");
        assert_eq!(activation_view("settings").expect("settings"), "settings");
        assert_eq!(activation_view("remote").expect("remote"), "remote");
        assert_eq!(
            activation_view("watch-party").expect("watch-party"),
            "watch-party"
        );
        assert!(activation_view("bogus").is_err());
    }
}
