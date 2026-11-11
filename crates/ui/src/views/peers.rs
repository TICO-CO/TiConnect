use std::cell::RefCell;
use std::rc::Rc;

use gtk::prelude::*;
use gtk4 as gtk;

use crate::model::{Peer, PeerListModel};

pub struct PeersView {
    pub root: gtk::Box,
    list: gtk::ListBox,
    empty: gtk::Label,
    ids: Rc<RefCell<Vec<String>>>,
}

impl PeersView {
    pub fn new(on_select: impl Fn(String) + 'static, on_pair: impl Fn() + 'static) -> Self {
        let root = gtk::Box::new(gtk::Orientation::Vertical, 0);
        root.add_css_class("sidebar");

        let header = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        header.set_margin_top(12);
        header.set_margin_bottom(8);
        header.set_margin_start(12);
        header.set_margin_end(12);

        let title = gtk::Label::new(Some("Peers"));
        title.add_css_class("heading");
        title.set_hexpand(true);
        title.set_halign(gtk::Align::Start);

        let pair_button = gtk::Button::from_icon_name("list-add-symbolic");
        pair_button.set_tooltip_text(Some("Pair a new device"));
        pair_button.update_property(&[gtk::accessible::Property::Label("Pair a new device")]);
        pair_button.add_css_class("flat");
        pair_button.connect_clicked(move |_| on_pair());

        header.append(&title);
        header.append(&pair_button);

        let empty = gtk::Label::new(Some(
            "No trusted peers yet.\nUse + to pair a device on your network.",
        ));
        empty.set_wrap(true);
        empty.set_justify(gtk::Justification::Center);
        empty.set_margin_top(24);
        empty.set_margin_start(18);
        empty.set_margin_end(18);
        empty.add_css_class("dim-label");

        let list = gtk::ListBox::new();
        list.set_selection_mode(gtk::SelectionMode::Single);
        list.add_css_class("navigation-sidebar");
        list.set_activate_on_single_click(true);

        let ids: Rc<RefCell<Vec<String>>> = Rc::new(RefCell::new(Vec::new()));
        let ids_for_select = ids.clone();
        let on_select = Rc::new(on_select);
        list.connect_row_activated(move |_, row| {
            if let Some(id) = ids_for_select.borrow().get(row.index() as usize) {
                on_select(id.clone());
            }
        });

        let scrolled = gtk::ScrolledWindow::builder()
            .hscrollbar_policy(gtk::PolicyType::Never)
            .vexpand(true)
            .child(&list)
            .build();

        root.append(&header);
        root.append(&empty);
        root.append(&scrolled);

        Self {
            root,
            list,
            empty,
            ids,
        }
    }

    pub fn render(&self, model: &PeerListModel) {
        while let Some(child) = self.list.first_child() {
            self.list.remove(&child);
        }
        self.ids.borrow_mut().clear();

        let peers = model.peers();
        self.empty.set_visible(peers.is_empty());
        self.list.set_visible(!peers.is_empty());

        let selected_id = model.selected().map(|peer| peer.id.clone());
        for peer in peers {
            self.ids.borrow_mut().push(peer.id.clone());
            let row = peer_row(peer);
            self.list.append(&row);
            if selected_id.as_deref() == Some(peer.id.as_str()) {
                self.list.select_row(Some(&row));
            }
        }
    }
}

fn peer_row(peer: &Peer) -> gtk::ListBoxRow {
    let row = gtk::ListBoxRow::new();
    row.update_property(&[gtk::accessible::Property::Label(&format!(
        "{} {}, policy {}, {}",
        peer.display_label(),
        peer.fingerprint,
        peer.policy.label(),
        if peer.connected { "online" } else { "offline" }
    ))]);

    let outer = gtk::Box::new(gtk::Orientation::Horizontal, 10);
    outer.set_margin_top(8);
    outer.set_margin_bottom(8);
    outer.set_margin_start(10);
    outer.set_margin_end(10);

    let avatar = gtk::Label::new(Some(&peer.initials()));
    avatar.add_css_class("avatar");
    avatar.set_valign(gtk::Align::Center);
    avatar.set_size_request(34, 34);

    let text = gtk::Box::new(gtk::Orientation::Vertical, 2);
    text.set_hexpand(true);

    let name = gtk::Label::new(Some(&peer.display_label()));
    name.set_halign(gtk::Align::Start);
    name.set_ellipsize(gtk::pango::EllipsizeMode::End);
    name.add_css_class("heading");

    let fingerprint = gtk::Label::new(Some(&peer.fingerprint));
    fingerprint.set_halign(gtk::Align::Start);
    fingerprint.set_ellipsize(gtk::pango::EllipsizeMode::Middle);
    fingerprint.add_css_class("monospace");
    fingerprint.add_css_class("dim-label");
    fingerprint.add_css_class("caption");

    text.append(&name);
    text.append(&fingerprint);

    let meta = gtk::Box::new(gtk::Orientation::Vertical, 2);
    meta.set_valign(gtk::Align::Center);

    let presence = gtk::Label::new(Some(if peer.connected { "Online" } else { "Offline" }));
    presence.add_css_class("caption");
    presence.add_css_class(if peer.connected {
        "presence-online"
    } else {
        "presence-offline"
    });

    let policy = gtk::Label::new(Some(peer.policy.label()));
    policy.add_css_class("caption");
    policy.add_css_class("dim-label");

    meta.append(&presence);
    meta.append(&policy);

    outer.append(&avatar);
    outer.append(&text);
    outer.append(&meta);
    row.set_child(Some(&outer));
    row
}
