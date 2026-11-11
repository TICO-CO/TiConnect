use std::cell::Cell;
use std::rc::Rc;

use gtk::prelude::*;
use gtk4 as gtk;

use crate::model::{Peer, PeerPolicy, SettingsForm};

pub struct SettingsView {
    pub root: gtk::Box,
    updating: Rc<Cell<bool>>,
    display_name: gtk::Entry,
    discovery_enabled: gtk::Switch,
    discovery_mdns: gtk::Switch,
    discovery_broadcast: gtk::Switch,
    read_receipts: gtk::Switch,
    dnd: gtk::Switch,
    overlay: gtk::Switch,
    outbox_ttl: gtk::SpinButton,
    ringtone: gtk::Entry,
    autostart: gtk::Switch,
    peers: gtk::Box,
    on_policy: Rc<dyn Fn(String, PeerPolicy)>,
}

impl SettingsView {
    pub fn new(
        on_setting: impl Fn(String, String) + 'static,
        on_dnd: impl Fn(bool) + 'static,
        on_autostart: impl Fn(bool) + 'static,
        on_policy: impl Fn(String, PeerPolicy) + 'static,
    ) -> Self {
        let updating = Rc::new(Cell::new(false));
        let on_setting: Rc<dyn Fn(String, String)> = Rc::new(on_setting);
        let on_dnd: Rc<dyn Fn(bool)> = Rc::new(on_dnd);
        let on_autostart: Rc<dyn Fn(bool)> = Rc::new(on_autostart);
        let on_policy: Rc<dyn Fn(String, PeerPolicy)> = Rc::new(on_policy);

        let root = gtk::Box::new(gtk::Orientation::Vertical, 0);
        root.set_hexpand(true);
        root.set_vexpand(true);

        let title = gtk::Label::new(Some("Settings"));
        title.set_halign(gtk::Align::Start);
        title.set_margin_top(14);
        title.set_margin_bottom(8);
        title.set_margin_start(18);
        title.set_margin_end(18);
        title.add_css_class("title-2");
        root.append(&title);

        let content = gtk::Box::new(gtk::Orientation::Vertical, 14);
        content.set_margin_start(18);
        content.set_margin_end(18);
        content.set_margin_bottom(18);

        let identity = section("Identity");
        let display_name = gtk::Entry::new();
        display_name.set_placeholder_text(Some("Display name"));
        display_name.update_property(&[gtk::accessible::Property::Label("Display name")]);
        {
            let on_setting = on_setting.clone();
            display_name.connect_activate(move |entry| {
                on_setting("identity.display_name".to_owned(), entry.text().to_string());
            });
        }
        let focus = gtk::EventControllerFocus::new();
        {
            let on_setting = on_setting.clone();
            let entry = display_name.clone();
            focus.connect_leave(move |_| {
                on_setting("identity.display_name".to_owned(), entry.text().to_string());
            });
        }
        display_name.add_controller(focus);
        identity.append(&display_name);
        content.append(&identity);

        let discovery = section("Discovery");
        let discovery_enabled = setting_switch(
            "Enabled",
            "Disable all LAN discovery",
            "discovery.enabled",
            &updating,
            on_setting.clone(),
        );
        let discovery_mdns = setting_switch(
            "mDNS",
            "Advertise and browse with multicast DNS",
            "discovery.mdns",
            &updating,
            on_setting.clone(),
        );
        let discovery_broadcast = setting_switch(
            "UDP broadcast",
            "Fallback discovery for networks without mDNS",
            "discovery.broadcast",
            &updating,
            on_setting.clone(),
        );
        discovery.append(&row(
            "Enabled",
            "Disable all LAN discovery",
            &discovery_enabled,
        ));
        discovery.append(&row(
            "mDNS",
            "Advertise and browse with multicast DNS",
            &discovery_mdns,
        ));
        discovery.append(&row(
            "UDP broadcast",
            "Fallback discovery for networks without mDNS",
            &discovery_broadcast,
        ));
        content.append(&discovery);

        let messaging = section("Messaging");
        let read_receipts = setting_switch(
            "Read receipts",
            "Send read receipts for inbound messages",
            "read_receipts.enabled",
            &updating,
            on_setting.clone(),
        );
        let dnd = gtk::Switch::new();
        dnd.update_property(&[gtk::accessible::Property::Label("Do not disturb")]);
        {
            let on_dnd = on_dnd.clone();
            let updating = updating.clone();
            dnd.connect_active_notify(move |switch| {
                if !updating.get() {
                    on_dnd(switch.is_active());
                }
            });
        }
        let outbox_ttl = gtk::SpinButton::with_range(1.0, 365.0, 1.0);
        outbox_ttl.set_value(7.0);
        outbox_ttl.update_property(&[gtk::accessible::Property::Label(
            "Outbox message expiry in days",
        )]);
        {
            let on_setting = on_setting.clone();
            let updating = updating.clone();
            outbox_ttl.connect_value_changed(move |spin| {
                if !updating.get() {
                    on_setting(
                        "outbox.ttl_days".to_owned(),
                        spin.value_as_int().to_string(),
                    );
                }
            });
        }
        let ttl_row = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        ttl_row.append(&label(
            "Outbox expiry (days)",
            "Messages expire after this many days",
        ));
        ttl_row.append(&outbox_ttl);
        messaging.append(&row(
            "Read receipts",
            "Send read receipts for inbound messages",
            &read_receipts,
        ));
        messaging.append(&row(
            "Do not disturb",
            "Suppress notifications while keeping messages",
            &dnd,
        ));
        messaging.append(&ttl_row);
        content.append(&messaging);

        let calls = section("Calls and overlay");
        let overlay = gtk::Switch::new();
        overlay.set_active(true);
        overlay.set_sensitive(false);
        overlay.update_property(&[gtk::accessible::Property::Label(
            "Overlay notifications, available in a later release",
        )]);
        let ringtone = gtk::Entry::new();
        ringtone.set_placeholder_text(Some("Ringtone file path"));
        ringtone.update_property(&[gtk::accessible::Property::Label("Ringtone path")]);
        {
            let on_setting = on_setting.clone();
            ringtone.connect_activate(move |entry| {
                on_setting("ringtone.path".to_owned(), entry.text().to_string());
            });
        }
        calls.append(&row(
            "Overlay notifications",
            "Available in a later release",
            &overlay,
        ));
        calls.append(&ringtone);
        content.append(&calls);

        let startup = section("Startup");
        let autostart = gtk::Switch::new();
        autostart.update_property(&[gtk::accessible::Property::Label(
            "Start TiConnect when logging in",
        )]);
        {
            let on_autostart = on_autostart.clone();
            let updating = updating.clone();
            autostart.connect_active_notify(move |switch| {
                if !updating.get() {
                    on_autostart(switch.is_active());
                }
            });
        }
        startup.append(&row(
            "Start on login",
            "Install an XDG autostart entry",
            &autostart,
        ));
        content.append(&startup);

        let peers_section = section("Peer policies");
        let peers = gtk::Box::new(gtk::Orientation::Vertical, 6);
        peers_section.append(&peers);
        content.append(&peers_section);

        let content_scroll = gtk::ScrolledWindow::builder()
            .hscrollbar_policy(gtk::PolicyType::Never)
            .vexpand(true)
            .child(&content)
            .build();
        root.append(&content_scroll);

        Self {
            root,
            updating,
            display_name,
            discovery_enabled,
            discovery_mdns,
            discovery_broadcast,
            read_receipts,
            dnd,
            overlay,
            outbox_ttl,
            ringtone,
            autostart,
            peers,
            on_policy,
        }
    }

    #[allow(deprecated)]
    pub fn render(&self, form: &SettingsForm, peers: &[Peer], autostart_enabled: bool) {
        self.updating.set(true);
        self.display_name
            .set_text(form.get("identity.display_name").unwrap_or_default());
        self.discovery_enabled
            .set_active(form.bool_value("discovery.enabled", true));
        self.discovery_mdns
            .set_active(form.bool_value("discovery.mdns", true));
        self.discovery_broadcast
            .set_active(form.bool_value("discovery.broadcast", true));
        self.read_receipts
            .set_active(form.bool_value("read_receipts.enabled", true));
        self.dnd.set_active(form.bool_value("dnd.enabled", false));
        self.overlay
            .set_active(form.bool_value("overlay.enabled", true));
        self.outbox_ttl
            .set_value(f64::from(form.u32_value("outbox.ttl_days", 7)));
        self.ringtone
            .set_text(form.get("ringtone.path").unwrap_or_default());
        self.autostart.set_active(autostart_enabled);

        while let Some(child) = self.peers.first_child() {
            self.peers.remove(&child);
        }
        for peer in peers {
            let row = gtk::Box::new(gtk::Orientation::Horizontal, 8);
            row.append(&label(&peer.display_label(), &peer.fingerprint));
            let combo = gtk::ComboBoxText::new();
            for policy in [PeerPolicy::Accept, PeerPolicy::Mute, PeerPolicy::Ignore] {
                combo.append(Some(policy.as_str()), policy.label());
            }
            combo.set_active_id(Some(peer.policy.as_str()));
            combo.update_property(&[gtk::accessible::Property::Label(&format!(
                "Policy for {}",
                peer.display_label()
            ))]);
            let on_policy = self.on_policy.clone();
            let peer_id = peer.id.clone();
            combo.connect_changed(move |combo| {
                if let Some(policy) = combo.active_id() {
                    on_policy(peer_id.clone(), PeerPolicy::parse(&policy));
                }
            });
            row.append(&combo);
            self.peers.append(&row);
        }
        self.updating.set(false);
    }
}

fn section(title: &str) -> gtk::Box {
    let section = gtk::Box::new(gtk::Orientation::Vertical, 6);
    let label = gtk::Label::new(Some(title));
    label.set_halign(gtk::Align::Start);
    label.add_css_class("heading");
    section.append(&label);
    section
}

fn label(title: &str, subtitle: &str) -> gtk::Box {
    let text = gtk::Box::new(gtk::Orientation::Vertical, 1);
    text.set_hexpand(true);
    let title = gtk::Label::new(Some(title));
    title.set_halign(gtk::Align::Start);
    let subtitle = gtk::Label::new(Some(subtitle));
    subtitle.set_halign(gtk::Align::Start);
    subtitle.add_css_class("caption");
    subtitle.add_css_class("dim-label");
    subtitle.set_ellipsize(gtk::pango::EllipsizeMode::End);
    text.append(&title);
    text.append(&subtitle);
    text
}

fn row(title: &str, subtitle: &str, switch: &gtk::Switch) -> gtk::Box {
    let row = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    row.append(&label(title, subtitle));
    row.append(switch);
    row
}

fn setting_switch(
    title: &str,
    subtitle: &str,
    key: &str,
    updating: &Rc<Cell<bool>>,
    on_setting: Rc<dyn Fn(String, String)>,
) -> gtk::Switch {
    let switch = gtk::Switch::new();
    switch.update_property(&[gtk::accessible::Property::Label(title)]);
    let _ = subtitle;
    let key = key.to_owned();
    let updating = updating.clone();
    switch.connect_active_notify(move |switch| {
        if !updating.get() {
            on_setting(key.clone(), switch.is_active().to_string());
        }
    });
    switch
}
