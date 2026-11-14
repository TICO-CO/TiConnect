#![forbid(unsafe_code)]

pub mod card;
pub mod client;
pub mod layer_shell;
pub mod notifications;
pub mod x11_fallback;

use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::rc::Rc;
use std::time::Duration;

use anyhow::Context;
use gtk::prelude::*;
use gtk4 as gtk;
use ticonnect_core::ipc::events::{Message, Peer};
use tokio::sync::mpsc::{self, UnboundedReceiver, UnboundedSender};

use crate::card::{AddOutcome, Card, CardRouter, ClickHandler, HoverHandler};
use crate::client::{Command, Event};

const APP_ID: &str = "org.ticonnect.Overlay";
const TICK_MS: u64 = 100;

pub fn run() -> anyhow::Result<()> {
    ticonnect_runtime::init().context("initialize runtime")?;

    if std::env::var_os("WAYLAND_DISPLAY").is_none() && std::env::var_os("DISPLAY").is_none() {
        tracing::info!("no graphical session; overlay not started");
        return Ok(());
    }

    let (event_tx, event_rx) = mpsc::unbounded_channel();
    let (command_tx, command_rx) = mpsc::unbounded_channel();
    let (ui_tx, ui_rx) = mpsc::unbounded_channel();
    client::spawn(event_tx, command_rx);

    let receivers = Rc::new(RefCell::new(Some((event_rx, ui_rx))));
    let started = Rc::new(Cell::new(false));
    let commands = command_tx.clone();
    let ui_sender = ui_tx.clone();

    let application = gtk::Application::builder().application_id(APP_ID).build();
    application.connect_activate(move |application| {
        if started.replace(true) {
            return;
        }
        let Some((event_rx, ui_rx)) = receivers.borrow_mut().take() else {
            return;
        };
        install_css();
        let surface = build_surface(application).unwrap_or_else(|| {
            tracing::warn!("no supported overlay surface; using system notifications");
            Surface::None
        });
        let state = State {
            surface,
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
    surface: Surface,
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
            Event::Peers(peers) => self.replace_peers(peers),
            Event::Peer(peer) => self.update_peer(peer),
            Event::Message(message) => self.show_message(message),
            Event::Disconnected(reason) => {
                tracing::warn!(%reason, "overlay lost the daemon connection");
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

    fn replace_peers(&mut self, peers: Vec<Peer>) {
        self.names = peers
            .into_iter()
            .map(|peer| (peer.id, peer.display_name))
            .collect();
    }

    fn update_peer(&mut self, peer: Peer) {
        self.names.insert(peer.id, peer.display_name);
    }

    fn show_message(&mut self, message: Message) {
        let sender = self.sender_name(&message);
        let card = Card::from_message(&message, sender);
        let available = !matches!(self.surface, Surface::None);
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

    fn sender_name(&self, message: &Message) -> String {
        let id = if message.peer_id.is_empty() {
            &message.sender_id
        } else {
            &message.peer_id
        };
        self.names
            .get(id)
            .filter(|name| !name.trim().is_empty())
            .cloned()
            .unwrap_or_else(|| short_id(id))
    }

    fn render(&self) {
        let cards: Vec<Card> = self.router.visible().cloned().collect();
        let commands = self.commands.clone();
        let ui = self.ui_tx.clone();
        let on_click: ClickHandler = Rc::new(move |id| {
            let _ = commands.send(Command::Activate("chat".to_owned()));
            let _ = ui.send(UiEvent::Dismiss(id.to_owned()));
        });
        let ui = self.ui_tx.clone();
        let on_hover: HoverHandler = Rc::new(move |id, hovered| {
            let _ = ui.send(UiEvent::Hover {
                id: id.to_owned(),
                hovered,
            });
        });
        match &self.surface {
            Surface::Wayland(surface) => surface.render(&cards, &on_click, &on_hover),
            Surface::X11(surface) => surface.render(&cards, &on_click, &on_hover),
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

fn build_surface(application: &gtk::Application) -> Option<Surface> {
    if gtk4_layer_shell::is_supported() {
        return layer_shell::Surface::new(application).map(Surface::Wayland);
    }
    let display = gtk::gdk::Display::default()?;
    display
        .backend()
        .is_x11()
        .then(|| x11_fallback::Surface::new(application))
        .map(Surface::X11)
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

fn short_id(id: &str) -> String {
    let chars: Vec<char> = id.chars().collect();
    if chars.len() <= 12 {
        return id.to_owned();
    }
    let start: String = chars.iter().take(8).collect();
    let end: String = chars.iter().rev().take(4).rev().collect();
    format!("{start}...{end}")
}
