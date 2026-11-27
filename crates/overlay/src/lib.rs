#![forbid(unsafe_code)]

pub mod card;
pub mod client;
pub mod layer_shell;
pub mod notifications;
pub mod player;
pub mod x11_fallback;

use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::rc::Rc;
use std::time::Duration;

use anyhow::Context;
use gtk::prelude::*;
use gtk4 as gtk;
use ticonnect_core::ipc::events::{Message, User};
use tokio::sync::mpsc::{self, UnboundedReceiver, UnboundedSender};

use crate::card::{
    ActionHandler, AddOutcome, Card, CardAction, CardRouter, ClickHandler, HoverHandler,
};
use crate::client::{Command, Event};
use crate::player::Player;

const APP_ID: &str = "org.ticonnect.Overlay";
const TICK_MS: u64 = 100;

pub fn run() -> anyhow::Result<()> {
    ticonnect_runtime::init().context("initialize runtime")?;

    if std::env::var_os("WAYLAND_DISPLAY").is_none() && std::env::var_os("DISPLAY").is_none() {
        tracing::info!("no graphical session; overlay not started");
        return Ok(());
    }

    if let Err(error) = ticonnect_runtime::ensure_daemon() {
        tracing::warn!(%error, "daemon is not available; overlay will keep reconnecting");
    }

    let (event_tx, event_rx) = mpsc::unbounded_channel();
    let (command_tx, command_rx) = mpsc::unbounded_channel();
    let (ui_tx, ui_rx) = mpsc::unbounded_channel();
    client::spawn(event_tx, command_rx);

    let receivers = Rc::new(RefCell::new(Some((event_rx, ui_rx))));
    let started = Rc::new(Cell::new(false));
    let commands = command_tx.clone();
    let ui_sender = ui_tx.clone();

    let builder = gtk::Application::builder().application_id(APP_ID);
    let builder = if ticonnect_core::ipc::ipc_bus_address().is_some() {
        builder.flags(gtk::gio::ApplicationFlags::NON_UNIQUE)
    } else {
        builder
    };
    let application = builder.build();
    application.connect_activate(move |application| {
        if started.replace(true) {
            return;
        }
        let Some((event_rx, ui_rx)) = receivers.borrow_mut().take() else {
            return;
        };
        install_css();
        let (requests_surface, toasts_surface) = build_surfaces(application).unwrap_or_else(|| {
            tracing::warn!("no supported overlay surface; using system notifications");
            (Surface::None, Surface::None)
        });
        let player = Player::new(application);
        let state = State {
            requests_surface,
            toasts_surface,
            player,
            player_visible: false,
            watch_party: None,
            active_call: None,
            active_remote: None,
            remote_request: None,
            pending_party: None,
            router: CardRouter::new(5_000),
            enabled: true,
            timeout_ms: 5_000,
            names: HashMap::new(),
            commands: commands.clone(),
            ui_tx: ui_sender.clone(),
        };
        let state = Rc::new(RefCell::new(state));
        spawn_events(state.clone(), event_rx, ui_rx);
        spawn_ticks(state);
    });

    application.run();
    Ok(())
}

struct State {
    requests_surface: Surface,
    toasts_surface: Surface,
    player: Player,
    player_visible: bool,
    watch_party: Option<ticonnect_core::ipc::events::WatchParty>,
    active_call: Option<ticonnect_core::ipc::events::Call>,
    active_remote: Option<ticonnect_core::ipc::events::RemoteSession>,
    remote_request: Option<ticonnect_core::ipc::events::RemoteSession>,
    pending_party: Option<ticonnect_core::ipc::events::WatchParty>,
    router: CardRouter,
    enabled: bool,
    timeout_ms: u64,
    names: HashMap<String, String>,
    commands: UnboundedSender<Command>,
    ui_tx: UnboundedSender<UiEvent>,
}

enum Surface {
    Wayland(layer_shell::Surface),
    X11(x11_fallback::Surface),
    None,
}

impl Surface {
    fn set_bottom_margin(&self, margin: i32) {
        match self {
            Self::Wayland(surface) => surface.set_bottom_margin(margin),
            Self::X11(surface) => surface.set_bottom_margin(margin),
            Self::None => {}
        }
    }
}

enum UiEvent {
    Hover { id: String, hovered: bool },
    Dismiss(String),
}

impl State {
    fn on_event(&mut self, event: Event) {
        match event {
            Event::Settings {
                enabled,
                timeout_secs,
            } => {
                self.enabled = enabled;
                self.timeout_ms = timeout_secs.saturating_mul(1_000);
                self.router.set_timeout_ms(self.timeout_ms);
                if !enabled && self.router.clear() {
                    self.render();
                }
            }
            Event::Users(users) => self.replace_users(users),
            Event::User(user) => self.update_user(user),
            Event::Message(message) => {
                self.show_message(message.clone());
                self.show_youtube(&message);
            }
            Event::IncomingCall(call) => self.show_call(call),
            Event::CallState(call) => self.apply_call_state(call),
            Event::RemoteSessionRequested(session) => self.show_remote_request(session),
            Event::RemoteState(session) => self.apply_remote_state(session),
            Event::CallEnded { call_id, reason } => self.finish_call(&call_id, reason),
            Event::RemoteSessionEnded { session_id, reason } => {
                self.finish_remote(&session_id, reason);
            }
            Event::WatchPartyRequested(party) => self.show_watch_party(party),
            Event::WatchPartyChanged(party) => self.apply_watch_party(party),
            Event::WatchPartyEnded { party_id, reason } => {
                self.end_watch_party(&party_id, reason);
            }
            Event::Disconnected(reason) => {
                tracing::warn!(%reason, "overlay lost the daemon connection");
                let card = Card::from_daemon_offline(reason);
                self.show_card(card);
            }
        }
    }

    fn on_ui_event(&mut self, event: UiEvent) {
        match event {
            UiEvent::Hover { id, hovered } => {
                self.router.set_hovered(&id, hovered);
            }
            UiEvent::Dismiss(id) => {
                if self.router.dismiss(&id) {
                    self.render();
                }
            }
        }
    }

    fn tick(&mut self, elapsed_ms: u64) {
        if self.router.advance(elapsed_ms) {
            self.render();
        }
    }

    fn replace_users(&mut self, users: Vec<User>) {
        self.names = users
            .into_iter()
            .map(|user| (user.id, user.display_name))
            .collect();
    }

    fn update_user(&mut self, user: User) {
        self.names.insert(user.id, user.display_name);
    }

    fn show_message(&mut self, message: Message) {
        let sender = self.sender_name(&message);
        let card = Card::from_message(&message, sender);
        let available = self.available();
        match self
            .router
            .route(card.clone(), message.notify, self.enabled, available)
        {
            AddOutcome::Overlay => self.render(),
            AddOutcome::Notification => {
                notifications::show(card, self.timeout_ms, self.commands.clone());
            }
            AddOutcome::Duplicate | AddOutcome::Suppressed => {}
        }
    }

    fn show_youtube(&mut self, message: &Message) {
        let Some(video_id) = ticonnect_core::youtube::first_video_id(&message.body) else {
            return;
        };
        if self.watch_party.is_some() {
            return;
        }
        self.player.show(&video_id, 0, true);
        self.player_visible = true;
        self.update_toast_offset();
    }

    fn show_watch_party(&mut self, party: ticonnect_core::ipc::events::WatchParty) {
        if party.role != "guest" {
            return;
        }
        let sender = self.user_name(&party.host_id);
        let card = Card::from_watch_party(&party, sender);
        self.pending_party = Some(party);
        self.show_card(card);
    }

    fn apply_watch_party(&mut self, party: ticonnect_core::ipc::events::WatchParty) {
        if party.state == "ringing" {
            return;
        }
        self.remove(&format!("party:{}", party.party_id));
        self.pending_party = None;
        self.player
            .show(&party.video_id, party.position_ms, party.playing);
        self.player_visible = true;
        self.update_toast_offset();
        let pill = Card::from_party_pill(&party);
        if self.watch_party.as_ref().map(|active| &active.party_id) != Some(&party.party_id) {
            self.show_card(pill);
        }
        self.watch_party = Some(party);
    }

    fn end_watch_party(
        &mut self,
        party_id: &str,
        reason: ticonnect_core::ipc::events::WatchPartyEndReason,
    ) {
        let ended = if self
            .watch_party
            .as_ref()
            .is_some_and(|party| party.party_id == party_id)
        {
            self.watch_party.take()
        } else {
            self.pending_party
                .take()
                .filter(|party| party.party_id == party_id)
        };
        if let Some(party) = ended {
            if self.watch_party.is_none() {
                self.player.hide();
                self.player_visible = false;
                self.update_toast_offset();
            }
            let sender = self.user_name(&party.host_id);
            let card = Card::from_party_ended(sender, party.host_id, party.party_id, reason);
            self.show_card(card);
        }
        self.remove(&format!("party:{party_id}"));
        self.remove(&format!("party-pill:{party_id}"));
    }

    fn show_call(&mut self, call: ticonnect_core::ipc::events::Call) {
        if call.direction != "incoming" {
            return;
        }
        let sender = self.user_name(&call.user_id);
        let card = Card::from_call(&call, sender);
        self.show_card(card);
    }

    fn show_remote_request(&mut self, session: ticonnect_core::ipc::events::RemoteSession) {
        if session.role != "host" && !session.role.is_empty() {
            return;
        }
        let sender = self.user_name(&session.user_id);
        let card = Card::from_remote_request(&session, sender);
        self.remote_request = Some(session);
        self.show_card(card);
    }

    fn apply_call_state(&mut self, call: Option<ticonnect_core::ipc::events::Call>) {
        if let Some(previous) = self.active_call.take() {
            self.remove(&format!("active-call:{}", previous.id));
        }
        let Some(call) = call else {
            return;
        };
        if call.state != "in-call" && call.state != "connecting" {
            return;
        }
        self.remove(&format!("call:{}", call.id));
        let sender = self.user_name(&call.user_id);
        let card = Card::from_active_call(&call, sender);
        self.show_card(card);
        self.active_call = Some(call);
    }

    fn finish_call(&mut self, call_id: &str, reason: ticonnect_core::ipc::events::CallEndReason) {
        self.remove(&format!("call:{call_id}"));
        self.remove(&format!("active-call:{call_id}"));
        let finished = self.active_call.take();
        let Some(call) = finished else {
            return;
        };
        let duration = crate::now_seconds().saturating_sub(call.started_at);
        let sender = self.user_name(&call.user_id);
        let card = Card::from_call_ended(sender, call.user_id, call.id, duration, reason);
        self.show_card(card);
    }

    fn apply_remote_state(&mut self, session: Option<ticonnect_core::ipc::events::RemoteSession>) {
        if let Some(previous) = self.active_remote.take() {
            self.remove(&format!("sharing:{}", previous.session_id));
        }
        let Some(session) = session else {
            return;
        };
        if session.state == "active" {
            self.remove(&format!("remote:{}", session.session_id));
            self.remote_request = None;
            let sender = self.user_name(&session.user_id);
            let card = Card::from_sharing(&session, sender);
            self.show_card(card);
            self.active_remote = Some(session);
        }
    }

    fn finish_remote(
        &mut self,
        session_id: &str,
        reason: ticonnect_core::ipc::events::RemoteEndReason,
    ) {
        self.remove(&format!("remote:{session_id}"));
        self.remove(&format!("sharing:{session_id}"));
        let ended = if self
            .active_remote
            .as_ref()
            .is_some_and(|session| session.session_id == session_id)
        {
            self.active_remote.take()
        } else {
            self.remote_request
                .take()
                .filter(|session| session.session_id == session_id)
        };
        if let Some(session) = ended {
            let sender = self.user_name(&session.user_id);
            let card =
                Card::from_remote_ended(sender, session.user_id, session_id.to_owned(), reason);
            self.show_card(card);
        }
    }

    fn show_card(&mut self, card: Card) {
        let available = self.available();
        match self
            .router
            .route(card.clone(), true, self.enabled, available)
        {
            AddOutcome::Overlay => self.render(),
            AddOutcome::Notification => {
                notifications::show(card, self.timeout_ms, self.commands.clone());
            }
            AddOutcome::Duplicate | AddOutcome::Suppressed => {}
        }
    }

    fn remove(&mut self, id: &str) {
        if self.router.dismiss(id) {
            self.render();
        }
    }

    fn update_toast_offset(&self) {
        let offset = if self.player_visible {
            24 + 420 + 8
        } else {
            24
        };
        self.toasts_surface.set_bottom_margin(offset);
    }

    fn available(&self) -> bool {
        !matches!(self.requests_surface, Surface::None)
            || !matches!(self.toasts_surface, Surface::None)
    }

    fn user_name(&self, user_id: &str) -> String {
        self.names
            .get(user_id)
            .filter(|name| !name.trim().is_empty())
            .cloned()
            .unwrap_or_else(|| short_id(user_id))
    }

    fn sender_name(&self, message: &Message) -> String {
        let id = if message.user_id.is_empty() {
            &message.sender_id
        } else {
            &message.user_id
        };
        self.names
            .get(id)
            .filter(|name| !name.trim().is_empty())
            .cloned()
            .unwrap_or_else(|| short_id(id))
    }

    fn render(&self) {
        let request_cards: Vec<Card> = self.router.visible_requests().cloned().collect();
        let toast_cards: Vec<Card> = self.router.visible_toasts().cloned().collect();
        let cards: Vec<Card> = request_cards
            .iter()
            .chain(toast_cards.iter())
            .cloned()
            .collect();
        let users: std::collections::HashMap<String, String> = cards
            .iter()
            .map(|card| (card.id.clone(), card.user_id.clone()))
            .collect();
        let commands = self.commands.clone();
        let ui = self.ui_tx.clone();
        let on_click: ClickHandler = Rc::new(move |id| {
            if let Some(user_id) = users.get(id) {
                let _ = commands.send(Command::Activate(format!("chat:{user_id}")));
            } else {
                let _ = commands.send(Command::Activate("chat".to_owned()));
            }
            let _ = ui.send(UiEvent::Dismiss(id.to_owned()));
        });
        let ui = self.ui_tx.clone();
        let on_hover: HoverHandler = Rc::new(move |id, hovered| {
            let _ = ui.send(UiEvent::Hover {
                id: id.to_owned(),
                hovered,
            });
        });
        let commands = self.commands.clone();
        let ui = self.ui_tx.clone();
        let on_action: ActionHandler = Rc::new(move |id: &str, action: CardAction| {
            match action {
                CardAction::AcceptCall => {
                    let _ = commands.send(Command::AcceptCall(strip(id, "call:")));
                }
                CardAction::DeclineCall => {
                    let _ = commands.send(Command::DeclineCall(strip(id, "call:")));
                }
                CardAction::HangupCall => {
                    let _ = commands.send(Command::HangupCall(strip(id, "active-call:")));
                }
                CardAction::AcceptRemote => {
                    let _ = commands.send(Command::AcceptRemoteSession(strip(id, "remote:")));
                }
                CardAction::DenyRemote => {
                    let _ = commands.send(Command::DenyRemoteSession(strip(id, "remote:")));
                }
                CardAction::EndRemote => {
                    let _ = commands.send(Command::EndRemoteSession(strip(id, "sharing:")));
                }
                CardAction::JoinWatchParty => {
                    let _ = commands.send(Command::AcceptWatchParty(strip(id, "party:")));
                }
                CardAction::DeclineWatchParty => {
                    let _ = commands.send(Command::DeclineWatchParty(strip(id, "party:")));
                }
                CardAction::OpenParty => {
                    let _ = commands.send(Command::Activate("watch-party".to_owned()));
                }
            }
            let _ = ui.send(UiEvent::Dismiss(id.to_owned()));
        });
        match &self.requests_surface {
            Surface::Wayland(surface) => {
                surface.render(&request_cards, &on_click, &on_hover, &on_action);
            }
            Surface::X11(surface) => {
                surface.render(&request_cards, &on_click, &on_hover, &on_action);
            }
            Surface::None => {}
        }
        match &self.toasts_surface {
            Surface::Wayland(surface) => {
                surface.render(&toast_cards, &on_click, &on_hover, &on_action);
            }
            Surface::X11(surface) => {
                surface.render(&toast_cards, &on_click, &on_hover, &on_action);
            }
            Surface::None => {}
        }
    }
}

fn spawn_events(
    state: Rc<RefCell<State>>,
    mut events: UnboundedReceiver<Event>,
    mut ui_events: UnboundedReceiver<UiEvent>,
) {
    gtk::glib::spawn_future_local(async move {
        loop {
            tokio::select! {
                event = events.recv() => {
                    let Some(event) = event else { break };
                    state.borrow_mut().on_event(event);
                }
                event = ui_events.recv() => {
                    let Some(event) = event else { break };
                    state.borrow_mut().on_ui_event(event);
                }
                else => break,
            }
        }
    });
}

fn spawn_ticks(state: Rc<RefCell<State>>) {
    gtk::glib::timeout_add_local(Duration::from_millis(TICK_MS), move || {
        state.borrow_mut().tick(TICK_MS);
        gtk::glib::ControlFlow::Continue
    });
}

fn build_surfaces(application: &gtk::Application) -> Option<(Surface, Surface)> {
    if gtk4_layer_shell::is_supported() {
        let requests = layer_shell::Surface::new(application, "ticonnect-overlay-requests", false)?;
        let toasts = layer_shell::Surface::new(application, "ticonnect-overlay-toasts", true)?;
        return Some((Surface::Wayland(requests), Surface::Wayland(toasts)));
    }
    let display = gtk::gdk::Display::default()?;
    if !display.backend().is_x11() {
        return None;
    }
    let requests = x11_fallback::Surface::new(application, "TiConnect Overlay Requests", false);
    let toasts = x11_fallback::Surface::new(application, "TiConnect Overlay Toasts", true);
    Some((Surface::X11(requests), Surface::X11(toasts)))
}

fn install_css() {
    let Some(display) = gtk::gdk::Display::default() else {
        return;
    };
    let provider = gtk::CssProvider::new();
    provider.load_from_string(include_str!("../resources/style.css"));
    gtk::style_context_add_provider_for_display(
        &display,
        &provider,
        gtk::STYLE_PROVIDER_PRIORITY_APPLICATION,
    );
}

pub(crate) fn now_seconds() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or_default()
}

fn strip(id: &str, prefix: &str) -> String {
    id.strip_prefix(prefix).unwrap_or(id).to_owned()
}

fn short_id(id: &str) -> String {
    let chars: Vec<char> = id.chars().collect();
    if chars.len() <= 12 {
        return id.to_owned();
    }
    let start: String = chars.iter().take(8).collect();
    let end: String = chars.iter().rev().take(4).rev().collect();
    format!("{start}...{end}")
}
