use std::collections::{BTreeMap, HashSet};

use ticonnect_settings::Appearance;

use crate::error::{Error, Result};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PeerPolicy {
    Accept,
    Mute,
    Ignore,
}

impl PeerPolicy {
    pub fn parse(value: &str) -> Self {
        match value {
            "mute" => Self::Mute,
            "ignore" => Self::Ignore,
            _ => Self::Accept,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Accept => "accept",
            Self::Mute => "mute",
            Self::Ignore => "ignore",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Accept => "Accept",
            Self::Mute => "Mute",
            Self::Ignore => "Ignore",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Peer {
    pub id: String,
    pub fingerprint: String,
    pub display_name: String,
    pub policy: PeerPolicy,
    pub trusted_at: u64,
    pub last_seen: u64,
    pub connected: bool,
}

impl Peer {
    pub fn display_label(&self) -> String {
        if self.display_name.trim().is_empty() {
            short_id(&self.id)
        } else {
            self.display_name.clone()
        }
    }

    pub fn initials(&self) -> String {
        let label = self.display_label();
        let mut parts = label.split_whitespace();
        let first = parts.next().and_then(|part| part.chars().next());
        let last = parts.last().and_then(|part| part.chars().next());
        match (first, last) {
            (Some(first), Some(last)) => format!("{first}{last}").to_uppercase(),
            (Some(first), None) => first.to_uppercase().to_string(),
            _ => "?".to_owned(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Candidate {
    pub node_id: String,
    pub fingerprint: String,
    pub display_name: String,
    pub addresses: String,
    pub source: String,
}

impl Candidate {
    pub fn label(&self) -> String {
        if self.display_name.trim().is_empty() {
            short_id(&self.node_id)
        } else {
            self.display_name.clone()
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MessageState {
    Queued,
    Sent,
    Delivered,
    Read,
    Failed,
    Expired,
    Unknown,
}

impl MessageState {
    pub fn parse(value: &str) -> Self {
        match value {
            "queued" => Self::Queued,
            "sent" => Self::Sent,
            "delivered" => Self::Delivered,
            "read" => Self::Read,
            "failed" => Self::Failed,
            "expired" => Self::Expired,
            _ => Self::Unknown,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Queued => "Queued",
            Self::Sent => "Sent",
            Self::Delivered => "Delivered",
            Self::Read => "Read",
            Self::Failed => "Failed",
            Self::Expired => "Expired",
            Self::Unknown => "Unknown",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Message {
    pub id: String,
    pub conversation_id: String,
    pub peer_id: String,
    pub sender_id: String,
    pub local_sender: bool,
    pub seq: u64,
    pub sent_at: u64,
    pub received_at: u64,
    pub body: String,
    pub state: MessageState,
}

impl Message {
    pub fn ordering_key(&self) -> (u64, u64, u64) {
        (self.sent_at, self.received_at, self.seq)
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PeerListModel {
    peers: Vec<Peer>,
    selected: Option<String>,
}

impl PeerListModel {
    pub fn replace(&mut self, peers: Vec<Peer>) {
        self.peers = peers;
        self.sort();
        if let Some(selected) = &self.selected
            && !self.peers.iter().any(|peer| &peer.id == selected)
        {
            self.selected = None;
        }
    }

    pub fn upsert(&mut self, peer: Peer) {
        if let Some(existing) = self.peers.iter_mut().find(|item| item.id == peer.id) {
            *existing = peer;
        } else {
            self.peers.push(peer);
        }
        self.sort();
    }

    pub fn set_connected(&mut self, peer_id: &str, connected: bool) {
        if let Some(peer) = self.peers.iter_mut().find(|peer| peer.id == peer_id) {
            peer.connected = connected;
        }
    }

    pub fn set_policy(&mut self, peer_id: &str, policy: PeerPolicy) {
        if let Some(peer) = self.peers.iter_mut().find(|peer| peer.id == peer_id) {
            peer.policy = policy;
        }
    }

    pub fn peers(&self) -> &[Peer] {
        &self.peers
    }

    pub fn is_empty(&self) -> bool {
        self.peers.is_empty()
    }

    pub fn selected(&self) -> Option<&Peer> {
        self.selected
            .as_ref()
            .and_then(|id| self.peers.iter().find(|peer| &peer.id == id))
    }

    pub fn select(&mut self, peer_id: &str) {
        if self.peers.iter().any(|peer| peer.id == peer_id) {
            self.selected = Some(peer_id.to_owned());
        }
    }

    pub fn clear_selection(&mut self) {
        self.selected = None;
    }

    fn sort(&mut self) {
        self.peers.sort_by(|left, right| {
            left.display_label()
                .to_lowercase()
                .cmp(&right.display_label().to_lowercase())
                .then_with(|| left.id.cmp(&right.id))
        });
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ConversationModel {
    conversation_id: Option<String>,
    peer_id: Option<String>,
    messages: Vec<Message>,
    oldest_offset: u32,
    total: u32,
    loading: bool,
    marked_read: HashSet<String>,
}

impl ConversationModel {
    pub fn reset(
        &mut self,
        conversation_id: String,
        peer_id: String,
        total: u32,
        messages: Vec<Message>,
    ) {
        self.conversation_id = Some(conversation_id);
        self.peer_id = Some(peer_id);
        self.messages = messages;
        self.sort();
        self.total = total;
        self.oldest_offset = self.total.saturating_sub(self.messages.len() as u32);
        self.loading = false;
        self.marked_read.clear();
    }

    pub fn clear(&mut self) {
        *self = Self::default();
    }

    pub fn conversation_id(&self) -> Option<&str> {
        self.conversation_id.as_deref()
    }

    pub fn peer_id(&self) -> Option<&str> {
        self.peer_id.as_deref()
    }

    pub fn messages(&self) -> &[Message] {
        &self.messages
    }

    pub fn is_empty(&self) -> bool {
        self.messages.is_empty()
    }

    pub fn set_loading(&mut self, loading: bool) {
        self.loading = loading;
    }

    pub fn loading(&self) -> bool {
        self.loading
    }

    pub fn prepend(&mut self, messages: Vec<Message>) {
        let count = messages.len() as u32;
        self.messages.splice(0..0, messages);
        self.sort();
        self.oldest_offset = self.oldest_offset.saturating_sub(count);
        self.loading = false;
    }

    pub fn push(&mut self, message: Message) {
        if let Some(existing) = self.messages.iter_mut().find(|item| item.id == message.id) {
            *existing = message;
        } else {
            self.messages.push(message);
            self.total = self.total.saturating_add(1);
            self.oldest_offset = self.total.saturating_sub(self.messages.len() as u32);
        }
        self.sort();
    }

    pub fn update_state(&mut self, message_id: &str, state: MessageState) {
        if let Some(message) = self.messages.iter_mut().find(|item| item.id == message_id) {
            message.state = state;
        }
    }

    pub fn has_older(&self) -> bool {
        self.oldest_offset > 0
    }

    pub fn older_page(&mut self) -> Option<(String, u32, u32)> {
        if !self.has_older() || self.loading {
            return None;
        }
        let conversation_id = self.conversation_id.clone()?;
        let limit = 50;
        let offset = self.oldest_offset.saturating_sub(limit);
        self.loading = true;
        Some((conversation_id, limit, offset))
    }

    pub fn mark_read_candidates(&mut self, local_sender_id: Option<&str>) -> Vec<String> {
        let mut ids = Vec::new();
        for message in &self.messages {
            if message.local_sender {
                continue;
            }
            if local_sender_id.is_some_and(|local| message.sender_id == local) {
                continue;
            }
            if self.marked_read.contains(&message.id) {
                continue;
            }
            self.marked_read.insert(message.id.clone());
            ids.push(message.id.clone());
        }
        ids
    }

    fn sort(&mut self) {
        self.messages.sort_by_key(Message::ordering_key);
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ComposerAction {
    Send,
    Newline,
    Ignore,
}

pub fn composer_action(enter: bool, shift: bool) -> ComposerAction {
    if !enter {
        ComposerAction::Ignore
    } else if shift {
        ComposerAction::Newline
    } else {
        ComposerAction::Send
    }
}

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

pub fn short_id(id: &str) -> String {
    let chars: Vec<char> = id.chars().collect();
    if chars.len() <= 12 {
        return id.to_owned();
    }
    let start: String = chars.iter().take(8).collect();
    let end: String = chars.iter().rev().take(4).rev().collect();
    format!("{start}...{end}")
}

pub fn pairing_view(value: &str) -> Result<&'static str> {
    match value.trim().to_ascii_lowercase().as_str() {
        "" | "chat" => Ok("chat"),
        "peers" => Ok("peers"),
        "settings" => Ok("settings"),
        "pairing" | "pair" => Ok("pairing"),
        other => Err(Error::InvalidResponse(format!(
            "unknown activation view: {other}"
        ))),
    }
}

pub fn format_countdown(expires_at: u64, now: u64) -> u64 {
    expires_at.saturating_sub(now)
}

pub fn format_pin(pin: &str) -> String {
    if pin.len() == 6 && pin.chars().all(|character| character.is_ascii_digit()) {
        format!("{} {}", &pin[..3], &pin[3..])
    } else {
        pin.to_owned()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn peer(id: &str, name: &str, connected: bool) -> Peer {
        Peer {
            id: id.to_owned(),
            fingerprint: "abc".to_owned(),
            display_name: name.to_owned(),
            policy: PeerPolicy::Accept,
            trusted_at: 0,
            last_seen: 0,
            connected,
        }
    }

    fn message(id: &str, sent_at: u64, local: bool) -> Message {
        Message {
            id: id.to_owned(),
            conversation_id: "conversation".to_owned(),
            peer_id: "peer".to_owned(),
            sender_id: "peer".to_owned(),
            local_sender: local,
            seq: sent_at,
            sent_at,
            received_at: sent_at,
            body: id.to_owned(),
            state: MessageState::Sent,
        }
    }

    #[test]
    fn peers_sort_and_track_presence() {
        let mut model = PeerListModel::default();
        model.replace(vec![peer("b", "Beta", false), peer("a", "Alpha", false)]);
        assert_eq!(model.peers()[0].display_name, "Alpha");
        model.set_connected("a", true);
        assert!(model.peers()[0].connected);
        model.set_policy("a", PeerPolicy::Mute);
        assert_eq!(model.peers()[0].policy, PeerPolicy::Mute);
    }

    #[test]
    fn conversation_dedupes_and_updates_state() {
        let mut model = ConversationModel::default();
        model.reset(
            "conversation".to_owned(),
            "peer".to_owned(),
            2,
            vec![message("one", 1, true)],
        );
        model.push(message("one", 1, true));
        model.push(message("two", 2, false));
        assert_eq!(model.messages().len(), 2);
        model.update_state("two", MessageState::Read);
        assert_eq!(model.messages()[1].state, MessageState::Read);
    }

    #[test]
    fn pagination_uses_oldest_offset() {
        let mut model = ConversationModel::default();
        model.reset(
            "conversation".to_owned(),
            "peer".to_owned(),
            120,
            (0..50)
                .map(|index| message(&index.to_string(), index, true))
                .collect(),
        );
        assert_eq!(
            model.older_page(),
            Some(("conversation".to_owned(), 50, 20))
        );
    }

    #[test]
    fn composer_enter_and_shift_enter_differ() {
        assert_eq!(composer_action(true, false), ComposerAction::Send);
        assert_eq!(composer_action(true, true), ComposerAction::Newline);
        assert_eq!(composer_action(false, false), ComposerAction::Ignore);
    }

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
        assert_eq!(pairing_view("").expect("default"), "chat");
        assert_eq!(pairing_view("settings").expect("settings"), "settings");
        assert!(pairing_view("bogus").is_err());
    }

    #[test]
    fn pin_and_countdown_format() {
        assert_eq!(format_pin("123456"), "123 456");
        assert_eq!(format_pin("12"), "12");
        assert_eq!(format_countdown(20, 5), 15);
        assert_eq!(format_countdown(5, 20), 0);
    }
}
