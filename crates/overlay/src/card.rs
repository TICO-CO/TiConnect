use std::collections::VecDeque;
use std::rc::Rc;

use gtk::prelude::*;
use gtk4 as gtk;
use ticonnect_core::ipc::events::Message;

pub const MAX_VISIBLE: usize = 3;
pub const DEDUP_CAPACITY: usize = 256;
const PREVIEW_CHARS: usize = 140;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Card {
    pub id: String,
    pub sender: String,
    pub preview: String,
    pub sent_at: u64,
}

impl Card {
    pub fn from_message(message: &Message, sender: String) -> Self {
        Self {
            id: message.id.clone(),
            sender,
            preview: preview(&message.body),
            sent_at: message.sent_at,
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
            if !card.hovered {
                card.remaining_ms = card.remaining_ms.saturating_sub(elapsed_ms);
            }
        }
        let before = self.visible.len();
        self.visible.retain(|card| card.remaining_ms > 0);
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

pub fn render_cards(
    container: &gtk::Box,
    cards: &[Card],
    on_click: &ClickHandler,
    on_hover: &HoverHandler,
) {
    while let Some(child) = container.first_child() {
        container.remove(&child);
    }
    let animate =
        gtk::Settings::default().is_some_and(|settings| settings.is_gtk_enable_animations());
    for card in cards {
        let widget = card_widget(card, on_click, on_hover);
        if animate {
            let revealer = gtk::Revealer::new();
            revealer.set_transition_type(gtk::RevealerTransitionType::SlideUp);
            revealer.set_transition_duration(180);
            revealer.set_child(Some(&widget));
            container.append(&revealer);
            gtk::glib::idle_add_local_once(move || revealer.set_reveal_child(true));
        } else {
            container.append(&widget);
        }
    }
}

fn card_widget(card: &Card, on_click: &ClickHandler, on_hover: &HoverHandler) -> gtk::Button {
    let button = gtk::Button::new();
    button.add_css_class("ticonnect-card");
    button.set_focusable(false);

    let content = gtk::Box::new(gtk::Orientation::Vertical, 4);
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
    button.set_child(Some(&content));

    let click = on_click.clone();
    let id = card.id.clone();
    button.connect_clicked(move |_| click(&id));

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
    let Ok(seconds) = i64::try_from(sent_at) else {
        return String::new();
    };
    gtk::glib::DateTime::from_unix_local(seconds)
        .ok()
        .and_then(|date| date.format("%H:%M").ok())
        .map(|value| value.to_string())
        .unwrap_or_default()
}
