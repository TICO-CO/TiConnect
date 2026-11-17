use std::collections::VecDeque;
use std::rc::Rc;

use gtk::prelude::*;
use gtk4 as gtk;
use ticonnect_core::ipc::events::{Call, Message, RemoteSession, WatchParty};

pub const MAX_VISIBLE: usize = 3;
pub const DEDUP_CAPACITY: usize = 256;
const PREVIEW_CHARS: usize = 140;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CardKind {
    Message,
    IncomingCall,
    RemoteRequest,
    WatchPartyInvite,
    Youtube,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CardAction {
    AcceptCall,
    DeclineCall,
    OpenRemote,
    DenyRemote,
    JoinWatchParty,
    DeclineWatchParty,
}

impl CardAction {
    pub fn label(self) -> &'static str {
        match self {
            Self::AcceptCall => "Accept",
            Self::DeclineCall => "Decline",
            Self::OpenRemote => "Open",
            Self::DenyRemote => "Deny",
            Self::JoinWatchParty => "Join",
            Self::DeclineWatchParty => "Decline",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Card {
    pub id: String,
    pub user_id: String,
    pub sender: String,
    pub preview: String,
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
            preview: "Incoming voice call".to_owned(),
            sent_at: call.started_at,
            kind: CardKind::IncomingCall,
            persistent: true,
            actions: vec![CardAction::AcceptCall, CardAction::DeclineCall],
        }
    }

    pub fn from_watch_party(party: &WatchParty, sender: String) -> Self {
        Self {
            id: format!("party:{}", party.party_id),
            user_id: party.user_id.clone(),
            sender,
            preview: "YouTube watch party invite".to_owned(),
            sent_at: party.updated_at,
            kind: CardKind::WatchPartyInvite,
            persistent: true,
            actions: vec![CardAction::JoinWatchParty, CardAction::DeclineWatchParty],
        }
    }

    pub fn from_remote_request(session: &RemoteSession, sender: String) -> Self {
        Self {
            id: format!("remote:{}", session.session_id),
            user_id: session.user_id.clone(),
            sender,
            preview: "Remote desktop request".to_owned(),
            sent_at: 0,
            kind: CardKind::RemoteRequest,
            persistent: true,
            actions: vec![CardAction::OpenRemote, CardAction::DenyRemote],
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
    queue: CardQueue,
    seen: VecDeque<String>,
}

impl CardRouter {
    pub fn new(timeout_ms: u64) -> Self {
        Self {
            queue: CardQueue::new(timeout_ms),
            seen: VecDeque::new(),
        }
    }

    pub fn set_timeout_ms(&mut self, timeout_ms: u64) {
        self.queue.set_timeout_ms(timeout_ms);
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
        if overlay_enabled && overlay_available {
            self.queue.enqueue(card);
            AddOutcome::Overlay
        } else {
            AddOutcome::Notification
        }
    }

    pub fn advance(&mut self, elapsed_ms: u64) -> bool {
        self.queue.advance(elapsed_ms)
    }

    pub fn set_hovered(&mut self, id: &str, hovered: bool) -> bool {
        self.queue.set_hovered(id, hovered)
    }

    pub fn dismiss(&mut self, id: &str) -> bool {
        self.queue.dismiss(id)
    }

    pub fn clear(&mut self) -> bool {
        self.queue.clear()
    }

    pub fn visible(&self) -> impl Iterator<Item = &Card> {
        self.queue.visible()
    }

    pub fn is_empty(&self) -> bool {
        self.queue.is_empty()
    }
}

pub fn preview(body: &str) -> String {
    let normalized = body.split_whitespace().collect::<Vec<_>>().join(" ");
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
    button.set_focusable(false);

    let content = gtk::Box::new(gtk::Orientation::Vertical, 8);
    content.set_margin_top(12);
    content.set_margin_bottom(12);
    content.set_margin_start(14);
    content.set_margin_end(14);
    content.set_hexpand(true);

    let header = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    let sender = gtk::Label::new(Some(&card.sender));
    sender.set_xalign(0.0);
    sender.set_hexpand(true);
    sender.set_ellipsize(gtk::pango::EllipsizeMode::End);
    sender.add_css_class("ticonnect-card-sender");
    let time = gtk::Label::new(Some(&time_label(card.sent_at)));
    time.set_xalign(1.0);
    time.add_css_class("ticonnect-card-time");
    header.append(&sender);
    header.append(&time);

    let body = gtk::Label::new(Some(&card.preview));
    body.set_xalign(0.0);
    body.set_wrap(true);
    body.set_wrap_mode(gtk::pango::WrapMode::WordChar);
    body.set_lines(3);
    body.set_ellipsize(gtk::pango::EllipsizeMode::End);
    body.add_css_class("ticonnect-card-preview");

    content.append(&header);
    content.append(&body);

    if !card.actions.is_empty() {
        let actions = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        actions.set_halign(gtk::Align::End);
        for action in &card.actions {
            let action_button = gtk::Button::with_label(action.label());
            action_button.add_css_class("ticonnect-card-action");
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

fn time_label(sent_at: u64) -> String {
    if sent_at == 0 {
        return String::new();
    }
    let Ok(seconds) = i64::try_from(sent_at) else {
        return String::new();
    };
    gtk::glib::DateTime::from_unix_local(seconds)
        .ok()
        .and_then(|date| date.format("%H:%M").ok())
        .map(|value| value.to_string())
        .unwrap_or_default()
}
