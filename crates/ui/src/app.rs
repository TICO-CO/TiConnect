use gtk4 as gtk;
use libadwaita as adw;
use libadwaita::prelude::*;
use relm4::prelude::*;
use tokio::sync::mpsc;

use crate::autostart;
use crate::client::{self, DaemonEvent, Identity, LinkHandle, PairingStart, Status};
use crate::model::{
    Candidate, ConversationModel, Message, Peer, PeerListModel, PeerPolicy, SettingsForm,
};
use crate::views::chat::ChatView;
use crate::views::pairing::PairingView;
use crate::views::peers::PeersView;
use crate::views::settings::SettingsView;

trait SendInput {
    fn send_input(&self, message: Msg);
}

impl SendInput for ComponentSender<App> {
    fn send_input(&self, message: Msg) {
        let _ = self.input_sender().send(message);
    }
}

#[derive(Debug)]
pub enum Msg {
    Daemon(DaemonEvent),
    SelectPeer(String),
    Send(String),
    LoadOlder,
    MarkRead,
    OpenPairing,
    StartPairing,
    PairWithPin(String, String),
    PairWithQr(String),
    CancelPairing,
    Tick,
    SetSetting(String, String),
    SetDnd(bool),
    SetAutostart(bool),
    SetPolicy(String, PeerPolicy),
    ShowView(String),
    DuplicateInstance,
    QuitUi,
    Escape,
    IdentityLoaded(Result<Identity, String>),
    PeersLoaded(Result<Vec<Peer>, String>),
    CandidatesLoaded(Result<Vec<Candidate>, String>),
    SettingsLoaded(Result<SettingsForm, String>),
    StatusLoaded(Result<Status, String>),
    ConversationLoaded {
        peer_id: String,
        conversation_id: String,
        total: u32,
        messages: Result<Vec<Message>, String>,
    },
    OlderLoaded {
        messages: Result<Vec<Message>, String>,
        scroll: f64,
    },
    SendResult {
        body: String,
        result: Result<String, String>,
    },
    PairingStarted(Result<PairingStart, String>),
    PairingActionResult(Result<(), String>),
    SettingSaved(Result<(), String>),
    AutostartSaved(Result<bool, String>),
    PolicySaved(Result<(), String>),
    MarkReadResult(Result<(), String>),
    Error(String),
}

pub struct App {
    window: adw::ApplicationWindow,
    link: LinkHandle,
    identity: Option<Identity>,
    pending_peer: Option<String>,
    peers: PeerListModel,
    candidates: Vec<Candidate>,
    conversation: ConversationModel,
    settings: SettingsForm,
    autostart_enabled: bool,
    status: Option<Status>,
    online: bool,
    pairing: Option<PairingStart>,
    peers_view: PeersView,
    chat_view: ChatView,
    settings_view: SettingsView,
    pairing_view: PairingView,
    banner: adw::Banner,
    header_status: gtk::Label,
    stack: gtk::Stack,
    toast_overlay: adw::ToastOverlay,
}

impl SimpleComponent for App {
    type Input = Msg;
    type Output = ();
    type Init = ();
    type Root = adw::ApplicationWindow;
    type Widgets = ();

    fn init_root() -> Self::Root {
        adw::ApplicationWindow::builder()
            .application(&relm4::main_application())
            .title("TiConnect")
            .default_width(1040)
            .default_height(720)
            .build()
    }

    fn init(
        _: Self::Init,
        root: Self::Root,
        sender: ComponentSender<Self>,
    ) -> ComponentParts<Self> {
        relm4::set_global_css(include_str!("../resources/style.css"));

        let peers_view = PeersView::new(
            {
                let sender = sender.clone();
                move |peer_id| sender.send_input(Msg::SelectPeer(peer_id))
            },
            {
                let sender = sender.clone();
                move || sender.send_input(Msg::OpenPairing)
            },
        );
        let chat_view = ChatView::new(
            {
                let sender = sender.clone();
                move |body| sender.send_input(Msg::Send(body))
            },
            {
                let sender = sender.clone();
                move || sender.send_input(Msg::LoadOlder)
            },
            {
                let sender = sender.clone();
                move || sender.send_input(Msg::MarkRead)
            },
        );
        let settings_view = SettingsView::new(
            {
                let sender = sender.clone();
                move |key, value| sender.send_input(Msg::SetSetting(key, value))
            },
            {
                let sender = sender.clone();
                move |enabled| sender.send_input(Msg::SetDnd(enabled))
            },
            {
                let sender = sender.clone();
                move |enabled| sender.send_input(Msg::SetAutostart(enabled))
            },
            {
                let sender = sender.clone();
                move |peer_id, policy| sender.send_input(Msg::SetPolicy(peer_id, policy))
            },
        );
        let pairing_view = PairingView::new(
            {
                let sender = sender.clone();
                move || sender.send_input(Msg::StartPairing)
            },
            {
                let sender = sender.clone();
                move || sender.send_input(Msg::CancelPairing)
            },
            {
                let sender = sender.clone();
                move |node_id, pin| sender.send_input(Msg::PairWithPin(node_id, pin))
            },
            {
                let sender = sender.clone();
                move |payload| sender.send_input(Msg::PairWithQr(payload))
            },
        );

        let banner = adw::Banner::new("Daemon unavailable. Reconnecting...");
        banner.set_revealed(false);

        let header = adw::HeaderBar::new();
        let title = adw::WindowTitle::new("TiConnect", "LAN messenger");
        header.set_title_widget(Some(&title));

        let header_status = gtk::Label::new(Some("Connecting..."));
        header_status.add_css_class("caption");
        header_status.add_css_class("dim-label");
        header_status.set_margin_end(8);
        header.pack_end(&header_status);

        let chat_button = gtk::Button::from_icon_name("chat-bubbles-symbolic");
        chat_button.set_tooltip_text(Some("Conversation"));
        chat_button.update_property(&[gtk::accessible::Property::Label("Conversation")]);
        chat_button.add_css_class("flat");
        {
            let sender = sender.clone();
            chat_button
                .connect_clicked(move |_| sender.send_input(Msg::ShowView("chat".to_owned())));
        }
        header.pack_end(&chat_button);

        let settings_button = gtk::Button::from_icon_name("emblem-system-symbolic");
        settings_button.set_tooltip_text(Some("Settings"));
        settings_button.update_property(&[gtk::accessible::Property::Label("Settings")]);
        settings_button.add_css_class("flat");
        {
            let sender = sender.clone();
            settings_button
                .connect_clicked(move |_| sender.send_input(Msg::ShowView("settings".to_owned())));
        }
        header.pack_end(&settings_button);

        let pair_button = gtk::Button::from_icon_name("list-add-symbolic");
        pair_button.set_tooltip_text(Some("Pair a device"));
        pair_button.update_property(&[gtk::accessible::Property::Label("Pair a device")]);
        pair_button.add_css_class("flat");
        {
            let sender = sender.clone();
            pair_button.connect_clicked(move |_| sender.send_input(Msg::OpenPairing));
        }
        header.pack_end(&pair_button);

        let body = gtk::Box::new(gtk::Orientation::Horizontal, 0);
        body.set_vexpand(true);
        peers_view.root.set_size_request(320, -1);
        body.append(&peers_view.root);
        body.append(&gtk::Separator::new(gtk::Orientation::Vertical));

        let empty = gtk::Box::new(gtk::Orientation::Vertical, 8);
        empty.set_vexpand(true);
        empty.set_valign(gtk::Align::Center);
        empty.set_halign(gtk::Align::Center);
        let empty_title = gtk::Label::new(Some("No conversation selected"));
        empty_title.add_css_class("title-2");
        let empty_body = gtk::Label::new(Some(
            "Select a trusted peer from the sidebar or pair a new device.",
        ));
        empty_body.add_css_class("dim-label");
        empty_body.set_wrap(true);
        empty.append(&empty_title);
        empty.append(&empty_body);

        let stack = gtk::Stack::new();
        stack.set_hexpand(true);
        stack.set_vexpand(true);
        stack.add_named(&empty, Some("empty"));
        stack.add_named(&chat_view.root, Some("chat"));
        stack.add_named(&settings_view.root, Some("settings"));
        stack.set_visible_child_name("empty");
        body.append(&stack);

        let content = gtk::Box::new(gtk::Orientation::Vertical, 0);
        content.set_vexpand(true);
        content.append(&banner);
        content.append(&header);
        content.append(&body);

        let toast_overlay = adw::ToastOverlay::new();
        toast_overlay.set_child(Some(&content));
        root.set_content(Some(&toast_overlay));

        let (events_sender, mut events_receiver) = mpsc::unbounded_channel();
        let link = client::spawn_link(events_sender);
        let event_sender = sender.clone();
        relm4::spawn(async move {
            while let Some(event) = events_receiver.recv().await {
                event_sender.send_input(Msg::Daemon(event));
            }
        });

        let (activation_sender, mut activation_receiver) = mpsc::unbounded_channel();
        let activation_msgs = sender.clone();
        relm4::spawn(async move {
            while let Some(event) = activation_receiver.recv().await {
                match event {
                    ActivationEvent::Show(view) => {
                        activation_msgs.send_input(Msg::ShowView(view));
                    }
                    ActivationEvent::Duplicate => {
                        activation_msgs.send_input(Msg::DuplicateInstance);
                    }
                }
            }
        });
        relm4::spawn(async move {
            if let Err(error) = serve_activation(activation_sender).await {
                tracing::warn!(%error, "UI activation service failed");
            }
        });

        let tick_sender = sender.clone();
        gtk::glib::timeout_add_seconds_local(1, move || {
            tick_sender.send_input(Msg::Tick);
            gtk::glib::ControlFlow::Continue
        });

        let shortcuts = gtk::EventControllerKey::new();
        shortcuts.set_propagation_phase(gtk::PropagationPhase::Capture);
        {
            let sender = sender.clone();
            shortcuts.connect_key_pressed(move |_, key, _, state| {
                let control = state.contains(gtk::gdk::ModifierType::CONTROL_MASK);
                let message = if control && key == gtk::gdk::Key::q {
                    Some(Msg::QuitUi)
                } else if control && key == gtk::gdk::Key::comma {
                    Some(Msg::ShowView("settings".to_owned()))
                } else if control && key == gtk::gdk::Key::n {
                    Some(Msg::OpenPairing)
                } else if control && key == gtk::gdk::Key::_1 {
                    Some(Msg::ShowView("peers".to_owned()))
                } else if control && key == gtk::gdk::Key::_2 {
                    Some(Msg::ShowView("chat".to_owned()))
                } else if key == gtk::gdk::Key::Escape {
                    Some(Msg::Escape)
                } else {
                    None
                };
                match message {
                    Some(message) => {
                        sender.send_input(message);
                        gtk::glib::Propagation::Stop
                    }
                    None => gtk::glib::Propagation::Proceed,
                }
            });
        }
        root.add_controller(shortcuts);

        peers_view.render(&PeerListModel::default());
        chat_view.set_composer_enabled(false);

        let model = App {
            window: root.clone(),
            link,
            identity: None,
            pending_peer: None,
            peers: PeerListModel::default(),
            candidates: Vec::new(),
            conversation: ConversationModel::default(),
            settings: SettingsForm::default(),
            autostart_enabled: autostart::enabled(),
            status: None,
            online: false,
            pairing: None,
            peers_view,
            chat_view,
            settings_view,
            pairing_view,
            banner,
            header_status,
            stack,
            toast_overlay,
        };

        ComponentParts { model, widgets: () }
    }

    fn update(&mut self, msg: Msg, sender: ComponentSender<Self>) {
        match msg {
            Msg::Daemon(event) => self.handle_daemon(event, &sender),
            Msg::SelectPeer(peer_id) => self.select_peer(&peer_id, &sender),
            Msg::Send(body) => {
                let body = body.trim().to_owned();
                if body.is_empty() {
                    return;
                }
                if let Some(peer) = self.peers.selected() {
                    let peer_id = peer.id.clone();
                    self.chat_view.clear_composer();
                    self.chat_view.set_error(None);
                    let link = self.link.clone();
                    let sender = sender.clone();
                    relm4::spawn(async move {
                        let result = link
                            .send_message(peer_id, body.clone())
                            .await
                            .map_err(|error| error.to_string());
                        sender.send_input(Msg::SendResult { body, result });
                    });
                }
            }
            Msg::LoadOlder => {
                let Some((conversation_id, limit, offset)) = self.conversation.older_page() else {
                    return;
                };
                self.chat_view.set_loading(true);
                let scroll = self.chat_view.scroll_value();
                let link = self.link.clone();
                let sender = sender.clone();
                relm4::spawn(async move {
                    let messages = link
                        .list_messages(conversation_id, limit, offset)
                        .await
                        .map_err(|error| error.to_string());
                    sender.send_input(Msg::OlderLoaded { messages, scroll });
                });
            }
            Msg::MarkRead => {
                self.mark_read(&sender);
            }
            Msg::OpenPairing => {
                self.pairing_view.reset();
                self.pairing_view.present(&self.window());
                self.load_candidates(&sender);
            }
            Msg::StartPairing => {
                self.pairing_view.set_busy(true);
                self.pairing_view
                    .set_status(Some("Starting pairing..."), false);
                let link = self.link.clone();
                let sender = sender.clone();
                relm4::spawn(async move {
                    let result = link
                        .start_pairing()
                        .await
                        .map_err(|error| error.to_string());
                    sender.send_input(Msg::PairingStarted(result));
                });
            }
            Msg::PairWithPin(node_id, pin) => {
                self.pairing_view.set_busy(true);
                self.pairing_view
                    .set_status(Some("Pairing with PIN..."), false);
                let link = self.link.clone();
                let sender = sender.clone();
                relm4::spawn(async move {
                    let result = link
                        .pair_with_pin(node_id, pin)
                        .await
                        .map_err(|error| error.to_string());
                    sender.send_input(Msg::PairingActionResult(result));
                });
            }
            Msg::PairWithQr(payload) => {
                self.pairing_view.set_busy(true);
                self.pairing_view
                    .set_status(Some("Pairing with QR payload..."), false);
                let link = self.link.clone();
                let sender = sender.clone();
                relm4::spawn(async move {
                    let result = link
                        .pair_with_qr(payload)
                        .await
                        .map_err(|error| error.to_string());
                    sender.send_input(Msg::PairingActionResult(result));
                });
            }
            Msg::CancelPairing => {
                let link = self.link.clone();
                let sender = sender.clone();
                relm4::spawn(async move {
                    let result = link
                        .cancel_pairing()
                        .await
                        .map_err(|error| error.to_string());
                    sender.send_input(Msg::PairingActionResult(result));
                });
                self.pairing_view.close();
                self.pairing_view.reset();
                self.pairing = None;
            }
            Msg::Tick => {
                self.pairing_view.tick();
            }
            Msg::SetSetting(key, value) => {
                let sender = sender.clone();
                relm4::spawn_blocking(move || {
                    let result = save_setting(&key, &value).map_err(|error| error.to_string());
                    sender.send_input(Msg::SettingSaved(result));
                });
            }
            Msg::SetDnd(enabled) => {
                let link = self.link.clone();
                let sender = sender.clone();
                relm4::spawn(async move {
                    if let Err(error) = link.set_dnd(enabled).await {
                        sender.send_input(Msg::Error(error.to_string()));
                    }
                });
            }
            Msg::SetAutostart(enabled) => {
                let sender = sender.clone();
                relm4::spawn_blocking(move || {
                    let result = autostart::set_enabled(enabled)
                        .map(|()| enabled)
                        .map_err(|error| error.to_string());
                    sender.send_input(Msg::AutostartSaved(result));
                });
            }
            Msg::SetPolicy(peer_id, policy) => {
                self.peers.set_policy(&peer_id, policy);
                self.render_peers();
                let link = self.link.clone();
                let sender = sender.clone();
                relm4::spawn(async move {
                    let result = link
                        .set_peer_policy(peer_id, policy)
                        .await
                        .map_err(|error| error.to_string());
                    sender.send_input(Msg::PolicySaved(result));
                });
            }
            Msg::ShowView(view) => self.show_view(&view, &sender),
            Msg::DuplicateInstance | Msg::QuitUi => {
                relm4::main_application().quit();
            }
            Msg::Escape => {
                self.pairing_view.close();
                if self.pairing.take().is_some() {
                    let link = self.link.clone();
                    let sender = sender.clone();
                    relm4::spawn(async move {
                        let result = link
                            .cancel_pairing()
                            .await
                            .map_err(|error| error.to_string());
                        sender.send_input(Msg::PairingActionResult(result));
                    });
                }
                if self.stack.visible_child_name().as_deref() == Some("settings") {
                    let name = if self.peers.selected().is_some() {
                        "chat"
                    } else {
                        "empty"
                    };
                    self.stack.set_visible_child_name(name);
                }
            }
            Msg::IdentityLoaded(result) => match result {
                Ok(identity) => {
                    self.identity = Some(identity);
                    if let Some(peer_id) = self.pending_peer.take() {
                        self.select_peer(&peer_id, &sender);
                    }
                }
                Err(error) => self.toast(error),
            },
            Msg::PeersLoaded(result) => match result {
                Ok(peers) => {
                    self.peers.replace(peers);
                    self.render_peers();
                }
                Err(error) => self.toast(error),
            },
            Msg::CandidatesLoaded(result) => match result {
                Ok(candidates) => {
                    self.candidates = candidates;
                    self.pairing_view.set_candidates(&self.candidates);
                }
                Err(error) => self.toast(error),
            },
            Msg::SettingsLoaded(result) => match result {
                Ok(settings) => {
                    self.settings = settings;
                    self.render_settings();
                }
                Err(error) => self.toast(error),
            },
            Msg::StatusLoaded(result) => match result {
                Ok(status) => {
                    self.status = Some(status);
                    self.render_status();
                }
                Err(error) => self.toast(error),
            },
            Msg::ConversationLoaded {
                peer_id,
                conversation_id,
                total,
                messages,
            } => {
                if self.peers.selected().map(|peer| peer.id.as_str()) != Some(peer_id.as_str()) {
                    return;
                }
                match messages {
                    Ok(messages) => {
                        let peer = self.peers.selected().cloned();
                        self.conversation
                            .reset(conversation_id, peer_id, total, messages);
                        if let Some(peer) = peer {
                            self.chat_view
                                .set_peer(&peer.display_label(), &peer.fingerprint);
                        }
                        self.render_conversation();
                        self.stack.set_visible_child_name("chat");
                        self.chat_view.scroll_to_bottom();
                        self.mark_read(&sender);
                    }
                    Err(error) => {
                        self.chat_view.set_loading(false);
                        self.chat_view.set_error(Some(&error));
                    }
                }
            }
            Msg::OlderLoaded { messages, scroll } => match messages {
                Ok(messages) => {
                    self.conversation.prepend(messages);
                    self.render_conversation();
                    self.chat_view.set_scroll_value(scroll);
                }
                Err(error) => {
                    self.conversation.set_loading(false);
                    self.chat_view.set_loading(false);
                    self.toast(error);
                }
            },
            Msg::SendResult { body, result } => match result {
                Ok(_) => {
                    if let Some(peer) = self.peers.selected() {
                        let peer_id = peer.id.clone();
                        self.open_conversation(&peer_id, &sender);
                    }
                }
                Err(error) => {
                    self.chat_view.set_composer_text(&body);
                    self.chat_view.set_error(Some(&error));
                }
            },
            Msg::PairingStarted(result) => match result {
                Ok(start) => {
                    self.pairing = Some(start.clone());
                    self.pairing_view.set_busy(false);
                    if let Err(error) = self.pairing_view.show_pairing(&start) {
                        self.pairing_view.set_status(Some(&error.to_string()), true);
                    }
                }
                Err(error) => {
                    self.pairing_view.set_busy(false);
                    self.pairing_view.set_status(Some(&error), true);
                }
            },
            Msg::PairingActionResult(result) => match result {
                Ok(()) => {
                    self.pairing_view.set_busy(false);
                    self.pairing_view
                        .set_status(Some("Waiting for pairing to complete..."), false);
                }
                Err(error) => {
                    self.pairing_view.set_busy(false);
                    self.pairing_view.set_status(Some(&error), true);
                }
            },
            Msg::SettingSaved(result) => match result {
                Ok(()) => {}
                Err(error) => self.toast(error),
            },
            Msg::AutostartSaved(result) => match result {
                Ok(enabled) => {
                    self.autostart_enabled = enabled;
                    self.render_settings();
                }
                Err(error) => self.toast(error),
            },
            Msg::PolicySaved(result) => {
                if let Err(error) = result {
                    self.toast(error);
                    self.load_peers(&sender);
                }
            }
            Msg::MarkReadResult(result) => {
                if let Err(error) = result {
                    tracing::debug!(%error, "mark read failed");
                }
            }
            Msg::Error(error) => self.toast(error),
        }
    }
}

impl App {
    fn window(&self) -> gtk::Window {
        self.window.clone().upcast()
    }

    fn handle_daemon(&mut self, event: DaemonEvent, sender: &ComponentSender<App>) {
        match event {
            DaemonEvent::Connected => {
                self.online = true;
                self.banner.set_revealed(false);
                self.chat_view.set_composer_enabled(true);
                self.load_identity(sender);
                self.load_peers(sender);
                self.load_settings(sender);
                self.load_candidates(sender);
                self.load_status(sender);
                if let Some(peer_id) = self.pending_peer.clone() {
                    self.select_peer(&peer_id, sender);
                } else if let Some(peer) = self.peers.selected().cloned() {
                    self.open_conversation(&peer.id, sender);
                }
            }
            DaemonEvent::Disconnected(reason) => {
                self.online = false;
                self.banner.set_revealed(true);
                self.banner
                    .set_title(&format!("Daemon unavailable: {reason}"));
                self.header_status.set_text("Daemon offline");
                self.chat_view.set_composer_enabled(false);
                let peers = self
                    .peers
                    .peers()
                    .iter()
                    .cloned()
                    .map(|mut peer| {
                        peer.connected = false;
                        peer
                    })
                    .collect();
                self.peers.replace(peers);
                self.render_peers();
            }
            DaemonEvent::Peers(peers) => {
                for peer in peers {
                    self.peers.upsert(peer);
                }
                self.render_peers();
            }
            DaemonEvent::CandidateLost(_) => {
                self.load_candidates(sender);
            }
            DaemonEvent::Candidates(candidates) => {
                for candidate in candidates {
                    if let Some(existing) = self
                        .candidates
                        .iter_mut()
                        .find(|item| item.node_id == candidate.node_id)
                    {
                        *existing = candidate;
                    } else {
                        self.candidates.push(candidate);
                    }
                }
                self.candidates.sort_by_key(Candidate::label);
                self.pairing_view.set_candidates(&self.candidates);
            }
            DaemonEvent::MessageReceived(message) => {
                if self.conversation.conversation_id() == Some(message.conversation_id.as_str()) {
                    self.conversation.push(message);
                    self.render_conversation();
                    self.mark_read(sender);
                }
            }
            DaemonEvent::MessageStateChanged { message_id, state } => {
                self.conversation.update_state(&message_id, state);
                self.render_conversation();
            }
            DaemonEvent::PeerConnected(peer) => {
                self.peers.set_connected(&peer.id, true);
                self.render_peers();
            }
            DaemonEvent::PeerDisconnected(peer) => {
                self.peers.set_connected(&peer.id, false);
                self.render_peers();
            }
            DaemonEvent::PairingStarted(start) => {
                self.pairing = Some(start.clone());
                self.pairing_view.set_busy(false);
                if let Err(error) = self.pairing_view.show_pairing(&start) {
                    self.pairing_view.set_status(Some(&error.to_string()), true);
                }
            }
            DaemonEvent::PairingCompleted { fingerprint, .. } => {
                self.pairing_view.set_busy(false);
                self.pairing_view.set_status(
                    Some(&format!("Paired successfully with {fingerprint}")),
                    false,
                );
                self.load_peers(sender);
                self.load_candidates(sender);
            }
            DaemonEvent::PairingFailed(reason) => {
                self.pairing = None;
                self.pairing_view.stop_countdown();
                self.pairing_view.set_busy(false);
                self.pairing_view.set_status(Some(&reason), true);
            }
            DaemonEvent::PolicyChanged { peer_id, policy } => {
                self.peers.set_policy(&peer_id, policy);
                self.render_peers();
                self.render_settings();
            }
            DaemonEvent::SettingsChanged(_) => {
                self.load_settings(sender);
                self.load_identity(sender);
            }
            DaemonEvent::SyncCompleted { peer_id, .. } => {
                if self
                    .conversation
                    .peer_id()
                    .is_some_and(|current| current == peer_id)
                {
                    let peer_id = peer_id.clone();
                    self.open_conversation(&peer_id, sender);
                }
            }
        }
    }

    fn select_peer(&mut self, peer_id: &str, sender: &ComponentSender<App>) {
        self.peers.select(peer_id);
        self.render_peers();
        if let Some(peer) = self.peers.selected().cloned() {
            self.chat_view
                .set_peer(&peer.display_label(), &peer.fingerprint);
        }
        self.stack.set_visible_child_name("chat");
        self.open_conversation(peer_id, sender);
    }

    fn open_conversation(&mut self, peer_id: &str, sender: &ComponentSender<App>) {
        let Some(identity) = self.identity.as_ref() else {
            self.pending_peer = Some(peer_id.to_owned());
            self.load_identity(sender);
            return;
        };
        let Ok(local) = identity.node_id.parse::<ticonnect_core::id::NodeId>() else {
            self.toast("daemon returned an invalid local node id".to_owned());
            return;
        };
        let Ok(peer) = peer_id.parse::<ticonnect_core::id::NodeId>() else {
            self.toast("invalid peer id".to_owned());
            return;
        };
        let conversation_id = ticonnect_core::message::conversation_id(local, peer);
        self.conversation.clear();
        self.chat_view.set_loading(true);
        self.chat_view.set_error(None);
        let link = self.link.clone();
        let sender = sender.clone();
        let peer_id = peer_id.to_owned();
        relm4::spawn(async move {
            let result = async {
                let total = link.count_messages(conversation_id.clone()).await?;
                let offset = total.saturating_sub(50);
                let messages = link
                    .list_messages(conversation_id.clone(), 50, offset)
                    .await?;
                Ok::<_, crate::error::Error>((total, messages))
            }
            .await
            .map_err(|error| error.to_string());
            let (total, messages) = match result {
                Ok((total, messages)) => (total, Ok(messages)),
                Err(error) => (0, Err(error)),
            };
            sender.send_input(Msg::ConversationLoaded {
                peer_id,
                conversation_id,
                total,
                messages,
            });
        });
    }

    fn mark_read(&mut self, sender: &ComponentSender<App>) {
        let Some(conversation_id) = self.conversation.conversation_id().map(ToOwned::to_owned)
        else {
            return;
        };
        let local = self
            .identity
            .as_ref()
            .map(|identity| identity.node_id.as_str());
        let ids = self.conversation.mark_read_candidates(local);
        if ids.is_empty() {
            return;
        }
        let link = self.link.clone();
        let sender = sender.clone();
        relm4::spawn(async move {
            let result = link
                .mark_read(conversation_id, ids)
                .await
                .map_err(|error| error.to_string());
            sender.send_input(Msg::MarkReadResult(result));
        });
    }

    fn show_view(&mut self, view: &str, sender: &ComponentSender<App>) {
        match client::parse_activation_view(view) {
            Ok("settings") => self.stack.set_visible_child_name("settings"),
            Ok("pairing") => {
                self.pairing_view.reset();
                self.pairing_view.present(&self.window());
                self.load_candidates(sender);
            }
            Ok("peers") => self
                .stack
                .set_visible_child_name(if self.peers.selected().is_some() {
                    "chat"
                } else {
                    "empty"
                }),
            Ok(_) => self
                .stack
                .set_visible_child_name(if self.peers.selected().is_some() {
                    "chat"
                } else {
                    "empty"
                }),
            Err(error) => self.toast(error.to_string()),
        }
        self.window.present();
    }

    fn load_identity(&self, sender: &ComponentSender<App>) {
        let link = self.link.clone();
        let sender = sender.clone();
        relm4::spawn(async move {
            let result = link.identity().await.map_err(|error| error.to_string());
            sender.send_input(Msg::IdentityLoaded(result));
        });
    }

    fn load_peers(&self, sender: &ComponentSender<App>) {
        let link = self.link.clone();
        let sender = sender.clone();
        relm4::spawn(async move {
            let result = link.list_peers().await.map_err(|error| error.to_string());
            sender.send_input(Msg::PeersLoaded(result));
        });
    }

    fn load_candidates(&self, sender: &ComponentSender<App>) {
        let link = self.link.clone();
        let sender = sender.clone();
        relm4::spawn(async move {
            let result = link
                .list_candidates()
                .await
                .map_err(|error| error.to_string());
            sender.send_input(Msg::CandidatesLoaded(result));
        });
    }

    fn load_settings(&self, sender: &ComponentSender<App>) {
        let link = self.link.clone();
        let sender = sender.clone();
        relm4::spawn(async move {
            let result = link
                .list_settings()
                .await
                .map_err(|error| error.to_string());
            sender.send_input(Msg::SettingsLoaded(result));
        });
    }

    fn load_status(&self, sender: &ComponentSender<App>) {
        let link = self.link.clone();
        let sender = sender.clone();
        relm4::spawn(async move {
            let result = link.status().await.map_err(|error| error.to_string());
            sender.send_input(Msg::StatusLoaded(result));
        });
    }

    fn render_conversation(&mut self) {
        let local = self
            .identity
            .as_ref()
            .map(|identity| identity.node_id.as_str());
        self.chat_view.set_loading(self.conversation.loading());
        self.chat_view.render(self.conversation.messages(), local);
    }

    fn render_peers(&mut self) {
        self.peers_view.render(&self.peers);
        self.render_settings();
    }

    fn render_settings(&self) {
        self.settings_view
            .render(&self.settings, self.peers.peers(), self.autostart_enabled);
    }

    fn render_status(&mut self) {
        let Some(status) = self.status.as_ref() else {
            return;
        };
        let connections = match status.transport_state.as_str() {
            "online" => format!("{} connected", status.active_sessions),
            "idle" => "idle".to_owned(),
            _ => "daemon online".to_owned(),
        };
        self.header_status
            .set_text(&format!("{} · {} queued", connections, status.outbox_depth));
    }

    fn toast(&self, message: String) {
        let toast = adw::Toast::new(&message);
        toast.set_timeout(4);
        self.toast_overlay.add_toast(toast);
    }
}

fn save_setting(key: &str, value: &str) -> Result<(), crate::error::Error> {
    let mut settings = ticonnect_settings::Settings::load()?;
    settings.set_setting(key, value)?;
    settings.save()?;
    Ok(())
}

#[derive(Debug, Clone)]
enum ActivationEvent {
    Show(String),
    Duplicate,
}

#[derive(Clone)]
struct UiService {
    sender: mpsc::UnboundedSender<ActivationEvent>,
}

#[zbus::interface(name = "org.ticonnect.UI")]
impl UiService {
    async fn activate(&self, view: String) -> zbus::fdo::Result<()> {
        let view = client::parse_activation_view(&view)
            .map_err(|error| zbus::fdo::Error::InvalidArgs(error.to_string()))?;
        self.sender
            .send(ActivationEvent::Show(view.to_owned()))
            .map_err(|_| zbus::fdo::Error::Failed("UI is not available".to_owned()))?;
        Ok(())
    }
}

async fn serve_activation(sender: mpsc::UnboundedSender<ActivationEvent>) -> zbus::Result<()> {
    let registered = zbus::connection::Builder::session()
        .allow_name_replacements(false)
        .replace_existing_names(false)
        .name(ticonnect_core::ipc::UI_INTERFACE)
        .serve_at(
            "/org/ticonnect/UI",
            UiService {
                sender: sender.clone(),
            },
        )
        .build()
        .await;

    match registered {
        Ok(_connection) => {
            std::future::pending::<()>().await;
            Ok(())
        }
        Err(zbus::Error::NameTaken) => {
            let connection = zbus::Connection::session().await?;
            let proxy = zbus::Proxy::new(
                &connection,
                ticonnect_core::ipc::UI_INTERFACE,
                "/org/ticonnect/UI",
                ticonnect_core::ipc::UI_INTERFACE,
            )
            .await?;
            let _: () = proxy.call("Activate", &("chat",)).await?;
            let _ = sender.send(ActivationEvent::Duplicate);
            Ok(())
        }
        Err(error) => Err(error),
    }
}

pub fn run() -> anyhow::Result<()> {
    let app = relm4::RelmApp::new("org.ticonnect.UI");
    app.allow_multiple_instances(true);
    app.run::<App>(());
    Ok(())
}
