use std::collections::VecDeque;
use std::rc::Rc;

use gtk::prelude::*;
use gtk4 as gtk;
use ticonnect_core::fingerprint::Fingerprint;
use ticonnect_core::ipc::events::{Call, Message, RemoteSession, WatchParty};

pub const MAX_VISIBLE: usize = 3;
pub const DEDUP_CAPACITY: usize = 256;
const PREVIEW_CHARS: usize = 140;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CardSurface {
    Request,
    Toast,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CardKind {
    Message,
    IncomingCall,
    RemoteRequest,
    WatchPartyInvite,
    ActiveCall,
    Sharing,
    PartyPill,
    CallEnded,
}

impl CardKind {
    fn surface(self) -> CardSurface {
        match self {
            Self::Message | Self::CallEnded => CardSurface::Toast,
            Self::IncomingCall
            | Self::RemoteRequest
            | Self::WatchPartyInvite
            | Self::ActiveCall
            | Self::Sharing
            | Self::PartyPill => CardSurface::Request,
        }
    }

    fn label(self) -> &'static str {
        match self {
            Self::Message => "Message",
            Self::IncomingCall => "Incoming call",
            Self::RemoteRequest => "Remote desktop",
            Self::WatchPartyInvite => "Watch party invite",
            Self::ActiveCall => "In call",
            Self::Sharing => "Sharing screen",
            Self::PartyPill => "Watch party",
            Self::CallEnded => "Voice call ended",
        }
    }

    fn is_active(self) -> bool {
        matches!(self, Self::ActiveCall | Self::Sharing | Self::PartyPill)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CardAction {
    AcceptCall,
    DeclineCall,
    AcceptRemote,
    DenyRemote,
    HangupCall,
    EndRemote,
    JoinWatchParty,
    DeclineWatchParty,
    OpenParty,
}

impl CardAction {
    pub fn label(self) -> &'static str {
        match self {
            Self::AcceptCall | Self::AcceptRemote => "Accept",
            Self::DeclineCall | Self::DenyRemote | Self::DeclineWatchParty => "Decline",
            Self::HangupCall | Self::EndRemote => "End",
            Self::JoinWatchParty => "Join",
            Self::OpenParty => "Open",
        }
    }

    fn is_danger(self) -> bool {
        matches!(
            self,
            Self::DeclineCall | Self::DenyRemote | Self::DeclineWatchParty
        )
    }

    fn is_accent(self) -> bool {
        matches!(
            self,
            Self::AcceptCall | Self::AcceptRemote | Self::JoinWatchParty | Self::OpenParty
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Card {
    pub id: String,
    pub user_id: String,
    pub sender: String,
    pub preview: String,
    pub detail: Option<String>,
    pub sent_at: u64,
    pub kind: CardKind,
    pub persistent: bool,
    pub actions: Vec<CardAction>,
}

impl Card {
    pub fn from_message(message: &Message, sender: String) -> Self {
        Self {
            id: message.id.clone(),
            user_id: if message.user_id.is_empty() {
                message.sender_id.clone()
            } else {
                message.user_id.clone()
            },
            sender,
            preview: preview(&message.body),
            detail: None,
            sent_at: message.sent_at,
            kind: CardKind::Message,
            persistent: false,
            actions: Vec::new(),
        }
    }

    pub fn from_call(call: &Call, sender: String) -> Self {
        Self {
            id: format!("call:{}", call.id),
            user_id: call.user_id.clone(),
            sender,
            preview: "Voice call · LAN".to_owned(),
            detail: None,
            sent_at: call.started_at,
            kind: CardKind::IncomingCall,
            persistent: true,
            actions: vec![CardAction::DeclineCall, CardAction::AcceptCall],
        }
    }

    pub fn from_watch_party(party: &WatchParty, sender: String) -> Self {
        Self {
            id: format!("party:{}", party.party_id),
            user_id: party.host_id.clone(),
            sender,
            preview: "Watch party · How we built TiConnect".to_owned(),
            detail: None,
            sent_at: party.updated_at,
            kind: CardKind::WatchPartyInvite,
            persistent: true,
            actions: vec![CardAction::DeclineWatchParty, CardAction::JoinWatchParty],
        }
    }

    pub fn from_remote_request(session: &RemoteSession, sender: String) -> Self {
        Self {
            id: format!("remote:{}", session.session_id),
            user_id: session.user_id.clone(),
            sender,
            preview: "Wants to view and control this screen".to_owned(),
            detail: fingerprint_detail(&session.fingerprint),
            sent_at: 0,
            kind: CardKind::RemoteRequest,
            persistent: true,
            actions: vec![CardAction::DenyRemote, CardAction::AcceptRemote],
        }
    }

    pub fn from_active_call(call: &Call, sender: String) -> Self {
        Self {
            id: format!("active-call:{}", call.id),
            user_id: call.user_id.clone(),
            sender,
            preview: "Encrypted · Opus · LAN".to_owned(),
            detail: None,
            sent_at: call.connected_at,
            kind: CardKind::ActiveCall,
            persistent: true,
            actions: vec![CardAction::HangupCall],
        }
    }

    pub fn from_sharing(session: &RemoteSession, sender: String) -> Self {
        Self {
            id: format!("sharing:{}", session.session_id),
            user_id: session.user_id.clone(),
            sender,
            preview: "Indicator stays visible while sharing.".to_owned(),
            detail: fingerprint_detail(&session.fingerprint),
            sent_at: 0,
            kind: CardKind::Sharing,
            persistent: true,
            actions: vec![CardAction::EndRemote],
        }
    }

    pub fn from_party_pill(party: &WatchParty) -> Self {
        Self {
            id: format!("party-pill:{}", party.party_id),
            user_id: party.host_id.clone(),
            sender: "Watch party".to_owned(),
            preview: "Watch party · 3 watching".to_owned(),
            detail: None,
            sent_at: party.updated_at,
            kind: CardKind::PartyPill,
            persistent: true,
            actions: vec![CardAction::OpenParty],
        }
    }

    pub fn from_call_ended(sender: String, user_id: String, duration_secs: u64) -> Self {
        Self {
            id: format!("call-ended:{user_id}:{duration_secs}"),
            user_id,
            sender,
            preview: format!("Voice call ended · {}", clock_duration(duration_secs)),
            detail: None,
            sent_at: now_millis(),
            kind: CardKind::CallEnded,
            persistent: false,
            actions: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AddOutcome {
    Overlay,
    Notification,
    Duplicate,
    Suppressed,
}

#[derive(Debug)]
struct VisibleCard {
    card: Card,
    remaining_ms: u64,
    hovered: bool,
}

#[derive(Debug)]
pub struct CardQueue {
    visible: Vec<VisibleCard>,
    pending: Vec<Card>,
    timeout_ms: u64,
    max_visible: usize,
}

impl CardQueue {
    pub fn new(timeout_ms: u64) -> Self {
        Self {
            visible: Vec::new(),
            pending: Vec::new(),
            timeout_ms,
            max_visible: MAX_VISIBLE,
        }
    }

    pub fn set_timeout_ms(&mut self, timeout_ms: u64) {
        self.timeout_ms = timeout_ms;
    }

    pub fn enqueue(&mut self, card: Card) {
        if self.visible.len() < self.max_visible {
            self.visible.push(VisibleCard {
                card,
                remaining_ms: self.timeout_ms,
                hovered: false,
            });
        } else {
            self.pending.push(card);
        }
    }

    pub fn advance(&mut self, elapsed_ms: u64) -> bool {
        for card in &mut self.visible {
            if !card.hovered && !card.card.persistent {
                card.remaining_ms = card.remaining_ms.saturating_sub(elapsed_ms);
            }
        }
        let before = self.visible.len();
        self.visible
            .retain(|card| card.card.persistent || card.remaining_ms > 0);
        let expired = self.visible.len() != before;
        let pending_before = self.pending.len();
        self.promote_pending();
        expired || self.pending.len() != pending_before
    }

    pub fn set_hovered(&mut self, id: &str, hovered: bool) -> bool {
        let Some(card) = self.visible.iter_mut().find(|card| card.card.id == id) else {
            return false;
        };
        card.hovered = hovered;
        true
    }

    pub fn dismiss(&mut self, id: &str) -> bool {
        let before = self.visible.len();
        self.visible.retain(|card| card.card.id != id);
        self.pending.retain(|card| card.id != id);
        self.promote_pending();
        before != self.visible.len()
    }

    pub fn clear(&mut self) -> bool {
        let changed = !self.visible.is_empty();
        self.visible.clear();
        self.pending.clear();
        changed
    }

    pub fn visible(&self) -> impl Iterator<Item = &Card> {
        self.visible.iter().map(|card| &card.card)
    }

    pub fn is_empty(&self) -> bool {
        self.visible.is_empty()
    }

    fn promote_pending(&mut self) {
        while self.visible.len() < self.max_visible {
            let Some(card) = self.pending.first().cloned() else {
                break;
            };
            self.pending.remove(0);
            self.visible.push(VisibleCard {
                card,
                remaining_ms: self.timeout_ms,
                hovered: false,
            });
        }
    }
}

#[derive(Debug)]
pub struct CardRouter {
    requests: CardQueue,
    toasts: CardQueue,
    seen: VecDeque<String>,
}

impl CardRouter {
    pub fn new(timeout_ms: u64) -> Self {
        Self {
            requests: CardQueue::new(timeout_ms),
            toasts: CardQueue::new(timeout_ms),
            seen: VecDeque::new(),
        }
    }

    pub fn set_timeout_ms(&mut self, timeout_ms: u64) {
        self.requests.set_timeout_ms(timeout_ms);
        self.toasts.set_timeout_ms(timeout_ms);
    }

    pub fn route(
        &mut self,
        card: Card,
        notify: bool,
        overlay_enabled: bool,
        overlay_available: bool,
    ) -> AddOutcome {
        if !notify {
            return AddOutcome::Suppressed;
        }
        if self.seen.iter().any(|id| id == &card.id) {
            return AddOutcome::Duplicate;
        }
        self.seen.push_back(card.id.clone());
        while self.seen.len() > DEDUP_CAPACITY {
            self.seen.pop_front();
        }
        if !overlay_enabled || !overlay_available {
            return AddOutcome::Notification;
        }
        match card.kind.surface() {
            CardSurface::Request => self.requests.enqueue(card),
            CardSurface::Toast => self.toasts.enqueue(card),
        }
        AddOutcome::Overlay
    }

    pub fn advance(&mut self, elapsed_ms: u64) -> bool {
        let requests = self.requests.advance(elapsed_ms);
        let toasts = self.toasts.advance(elapsed_ms);
        requests || toasts
    }

    pub fn set_hovered(&mut self, id: &str, hovered: bool) -> bool {
        self.requests.set_hovered(id, hovered) || self.toasts.set_hovered(id, hovered)
    }

    pub fn dismiss(&mut self, id: &str) -> bool {
        self.requests.dismiss(id) || self.toasts.dismiss(id)
    }

    pub fn clear(&mut self) -> bool {
        let requests = self.requests.clear();
        let toasts = self.toasts.clear();
        requests || toasts
    }

    pub fn visible_requests(&self) -> impl Iterator<Item = &Card> {
        self.requests.visible()
    }

    pub fn visible_toasts(&self) -> impl Iterator<Item = &Card> {
        self.toasts.visible()
    }

    pub fn is_empty(&self) -> bool {
        self.requests.is_empty() && self.toasts.is_empty()
    }
}

fn fingerprint_detail(fingerprint: &str) -> Option<String> {
    if fingerprint.trim().is_empty() {
        return None;
    }
    let grouped = fingerprint
        .parse::<Fingerprint>()
        .map(|value| value.to_string())
        .unwrap_or_else(|_| fingerprint.to_owned());
    Some(format!("Identity: {grouped}"))
}

pub fn preview(body: &str) -> String {
    let normalized = body.split_whitespace().collect::<Vec<_>>().join(" ");
    if normalized.is_empty() {
        return "New message".to_owned();
    }
    let mut chars = normalized.chars();
    let shortened: String = chars.by_ref().take(PREVIEW_CHARS).collect();
    if chars.next().is_some() {
        format!("{shortened}...")
    } else {
        shortened
    }
}

pub type ClickHandler = Rc<dyn Fn(&str)>;
pub type HoverHandler = Rc<dyn Fn(&str, bool)>;
pub type ActionHandler = Rc<dyn Fn(&str, CardAction)>;

pub fn render_cards(
    container: &gtk::Box,
    cards: &[Card],
    on_click: &ClickHandler,
    on_hover: &HoverHandler,
    on_action: &ActionHandler,
) {
    while let Some(child) = container.first_child() {
        container.remove(&child);
    }
    let animate =
        gtk::Settings::default().is_some_and(|settings| settings.is_gtk_enable_animations());
    for card in cards {
        let widget = card_widget(card, on_click, on_hover, on_action);
        if animate {
            let revealer = gtk::Revealer::new();
            revealer.set_transition_type(gtk::RevealerTransitionType::SlideDown);
            revealer.set_transition_duration(180);
            revealer.set_child(Some(&widget));
            container.append(&revealer);
            gtk::glib::idle_add_local_once(move || revealer.set_reveal_child(true));
        } else {
            container.append(&widget);
        }
    }
}

fn card_widget(
    card: &Card,
    on_click: &ClickHandler,
    on_hover: &HoverHandler,
    on_action: &ActionHandler,
) -> gtk::Box {
    let button = gtk::Box::new(gtk::Orientation::Vertical, 0);
    button.add_css_class("ticonnect-card");
    button.add_css_class(card_class(card));
    button.set_focusable(false);

    let content = gtk::Box::new(gtk::Orientation::Vertical, 8);
    content.set_margin_top(14);
    content.set_margin_bottom(14);
    content.set_margin_start(16);
    content.set_margin_end(16);
    content.set_hexpand(true);

    let header = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    header.set_valign(gtk::Align::Center);
    let monogram = gtk::Label::new(Some(&initials(&card.sender)));
    monogram.add_css_class("ticonnect-monogram");
    let label_text = if matches!(card.kind, CardKind::Message | CardKind::CallEnded) {
        card.sender.clone()
    } else {
        card.kind.label().to_owned()
    };
    let kind = gtk::Label::new(Some(&label_text));
    kind.set_xalign(0.0);
    kind.set_hexpand(true);
    kind.add_css_class("ticonnect-card-kind");
    let time = gtk::Label::new(Some(&time_label(card.sent_at)));
    time.set_xalign(1.0);
    time.add_css_class("ticonnect-card-time");
    header.append(&monogram);
    header.append(&kind);
    header.append(&time);

    let title = gtk::Label::new(Some(&card.sender));
    title.set_xalign(0.0);
    title.set_wrap(true);
    title.set_wrap_mode(gtk::pango::WrapMode::WordChar);
    title.set_lines(2);
    title.set_ellipsize(gtk::pango::EllipsizeMode::End);
    title.add_css_class("ticonnect-card-title");

    let body = gtk::Label::new(Some(&card.preview));
    body.set_xalign(0.0);
    body.set_wrap(true);
    body.set_wrap_mode(gtk::pango::WrapMode::WordChar);
    body.set_lines(3);
    body.set_ellipsize(gtk::pango::EllipsizeMode::End);
    body.add_css_class(if card.kind == CardKind::Message {
        "ticonnect-card-preview"
    } else {
        "ticonnect-card-subtitle"
    });

    content.append(&header);
    if card.kind != CardKind::Message {
        content.append(&title);
    }
    content.append(&body);
    if let Some(detail) = &card.detail {
        let identity = gtk::Label::new(Some(detail));
        identity.set_xalign(0.0);
        identity.set_wrap(true);
        identity.set_wrap_mode(gtk::pango::WrapMode::Char);
        identity.set_lines(3);
        identity.set_ellipsize(gtk::pango::EllipsizeMode::End);
        identity.add_css_class("ticonnect-card-fingerprint");
        content.append(&identity);
    }

    if !card.actions.is_empty() {
        let actions = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        actions.set_halign(gtk::Align::Start);
        for action in &card.actions {
            let action_button = gtk::Button::with_label(action.label());
            action_button.add_css_class("ticonnect-card-action");
            if action.is_danger() {
                action_button.add_css_class("danger");
            }
            if action.is_accent() {
                action_button.add_css_class("accent");
            }
            let handler = on_action.clone();
            let id = card.id.clone();
            let action = *action;
            action_button.connect_clicked(move |_| handler(&id, action));
            actions.append(&action_button);
        }
        content.append(&actions);
    }

    button.append(&content);

    if card.actions.is_empty() {
        let click = on_click.clone();
        let id = card.id.clone();
        let gesture = gtk::GestureClick::new();
        gesture.connect_released(move |_, _, _, _| click(&id));
        button.add_controller(gesture);
    }

    let motion = gtk::EventControllerMotion::new();
    let enter = on_hover.clone();
    let enter_id = card.id.clone();
    motion.connect_enter(move |_, _, _| enter(&enter_id, true));
    let leave = on_hover.clone();
    let leave_id = card.id.clone();
    motion.connect_leave(move |_| leave(&leave_id, false));
    button.add_controller(motion);

    button
}

fn card_class(card: &Card) -> &'static str {
    if card.kind.is_active() {
        return "ticonnect-card-active";
    }
    if card.kind == CardKind::PartyPill {
        return "ticonnect-card-pill";
    }
    if card.kind == CardKind::Message || card.kind == CardKind::CallEnded {
        return "ticonnect-toast";
    }
    "ticonnect-request"
}

fn initials(name: &str) -> String {
    let mut words = name
        .split(|character: char| character.is_whitespace() || character == '-' || character == '_')
        .filter(|word| !word.is_empty());
    let first = words.next().unwrap_or("");
    let second = words.next().unwrap_or("");
    let initials: String = if second.is_empty() {
        first.chars().take(2).collect()
    } else {
        first
            .chars()
            .take(1)
            .chain(second.chars().take(1))
            .collect()
    };
    initials.to_uppercase()
}

fn time_label(sent_at: u64) -> String {
    if sent_at == 0 {
        return String::new();
    }
    let Ok(seconds) = i64::try_from(sent_at / 1_000) else {
        return String::new();
    };
    gtk::glib::DateTime::from_unix_local(seconds)
        .ok()
        .and_then(|date| date.format("%H:%M").ok())
        .map(|value| value.to_string())
        .unwrap_or_default()
}

fn clock_duration(seconds: u64) -> String {
    let hours = seconds / 3600;
    let minutes = (seconds % 3600) / 60;
    let remaining = seconds % 60;
    format!("{hours:02}:{minutes:02}:{remaining:02}")
}

fn now_millis() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| u64::try_from(duration.as_millis()).unwrap_or(u64::MAX))
        .unwrap_or_default()
}
