use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;
use std::time::Instant;

use gtk4 as gtk;
use libadwaita as adw;
use libadwaita::prelude::*;

use crate::model::{ConversationSummary, Peer, PeerListModel, PeerPolicyLabel, PeerPresentation};

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

    pub fn render(
        &self,
        model: &PeerListModel,
        summaries: &[ConversationSummary],
        avatars: &HashMap<String, gtk::gdk::Texture>,
        typing: &HashMap<String, Instant>,
    ) {
        while let Some(child) = self.list.first_child() {
            self.list.remove(&child);
        }
        self.ids.borrow_mut().clear();

        let mut peers = model.peers().to_vec();
        peers.sort_by(|left, right| {
            let left_at = summary_for(summaries, &left.id)
                .map(|summary| summary.last_sent_at)
                .unwrap_or_default();
            let right_at = summary_for(summaries, &right.id)
                .map(|summary| summary.last_sent_at)
                .unwrap_or_default();
            right_at.cmp(&left_at).then_with(|| {
                left.display_label()
                    .to_lowercase()
                    .cmp(&right.display_label().to_lowercase())
            })
        });
        self.empty.set_visible(peers.is_empty());
        self.list.set_visible(!peers.is_empty());

        let selected_id = model.selected().map(|peer| peer.id.clone());
        for peer in &peers {
            self.ids.borrow_mut().push(peer.id.clone());
            let summary = summary_for(summaries, &peer.id);
            let row = peer_row(
                peer,
                summary,
                avatars.get(&peer.fingerprint),
                typing.contains_key(&peer.id),
            );
            self.list.append(&row);
            if selected_id.as_deref() == Some(peer.id.as_str()) {
                self.list.select_row(Some(&row));
            }
        }
    }
}

fn summary_for<'a>(
    summaries: &'a [ConversationSummary],
    peer_id: &str,
) -> Option<&'a ConversationSummary> {
    summaries.iter().find(|summary| summary.peer_id == peer_id)
}

fn peer_row(
    peer: &Peer,
    summary: Option<&ConversationSummary>,
    avatar: Option<&gtk::gdk::Texture>,
    typing: bool,
) -> gtk::ListBoxRow {
    let row = gtk::ListBoxRow::new();
    row.add_css_class("peer-row");
    row.set_tooltip_text(Some(&peer.fingerprint));
    row.update_property(&[gtk::accessible::Property::Label(&format!(
        "{} {}, policy {}, {}",
        peer.display_label(),
        peer.fingerprint,
        peer.policy.label(),
        if peer.connected { "online" } else { "offline" }
    ))]);

    let outer = gtk::Box::new(gtk::Orientation::Horizontal, 10);
    outer.set_margin_top(7);
    outer.set_margin_bottom(7);
    outer.set_margin_start(9);
    outer.set_margin_end(9);

    let avatar = avatar_widget(avatar, &peer.initials(), 42);
    avatar.set_valign(gtk::Align::Center);

    let text = gtk::Box::new(gtk::Orientation::Vertical, 2);
    text.set_hexpand(true);

    let name = gtk::Label::new(Some(&peer.display_label()));
    name.set_halign(gtk::Align::Start);
    name.set_ellipsize(gtk::pango::EllipsizeMode::End);
    name.add_css_class("peer-name");

    let preview = gtk::Label::new(Some(&preview_text(summary, peer, typing)));
    preview.set_halign(gtk::Align::Start);
    preview.set_ellipsize(gtk::pango::EllipsizeMode::End);
    preview.set_xalign(0.0);
    preview.add_css_class("caption");
    if typing {
        preview.add_css_class("typing-preview");
    } else {
        preview.add_css_class("dim-label");
    }

    text.append(&name);
    text.append(&preview);

    let meta = gtk::Box::new(gtk::Orientation::Vertical, 3);
    meta.set_valign(gtk::Align::Center);
    meta.set_halign(gtk::Align::End);

    let time = gtk::Label::new(Some(
        summary
            .map(|summary| summary.last_sent_at)
            .map(format_timestamp)
            .as_deref()
            .unwrap_or_default(),
    ));
    time.add_css_class("caption");
    time.add_css_class("dim-label");

    let unread = summary.map_or(0, |summary| summary.unread_count);
    if unread > 0 {
        let badge = gtk::Label::new(Some(&unread.to_string()));
        badge.add_css_class("unread-badge");
        badge.set_halign(gtk::Align::End);
        meta.append(&badge);
    } else if peer.policy == crate::model::PeerPolicy::Mute {
        let muted = gtk::Image::from_icon_name("notifications-disabled-symbolic");
        muted.add_css_class("dim-label");
        muted.set_pixel_size(14);
        meta.append(&muted);
    }

    let presence = gtk::Box::new(gtk::Orientation::Horizontal, 4);
    presence.set_halign(gtk::Align::End);
    let dot = gtk::Label::new(Some("●"));
    dot.add_css_class(if peer.connected {
        "presence-online"
    } else {
        "presence-offline"
    });
    dot.add_css_class("presence-dot");
    presence.append(&dot);
    presence.append(&time);
    meta.prepend(&presence);

    outer.append(&avatar);
    outer.append(&text);
    outer.append(&meta);
    row.set_child(Some(&outer));
    row
}

pub fn avatar_widget(
    texture: Option<&gtk::gdk::Texture>,
    initials: &str,
    size: i32,
) -> adw::Avatar {
    let avatar = adw::Avatar::new(size, Some(initials), true);
    avatar.add_css_class("peer-avatar");
    if let Some(texture) = texture {
        avatar.set_custom_image(Some(texture));
    }
    avatar
}

fn preview_text(summary: Option<&ConversationSummary>, peer: &Peer, typing: bool) -> String {
    if typing {
        return "typing…".to_owned();
    }
    let Some(summary) = summary else {
        return if peer.connected {
            "Online".to_owned()
        } else {
            "No messages yet".to_owned()
        };
    };
    let body = summary.last_body.trim();
    if body.is_empty() {
        if summary.last_message_id.is_empty() {
            "No messages yet".to_owned()
        } else {
            "Attachment".to_owned()
        }
    } else {
        body.replace('\n', " ")
    }
}

pub fn format_timestamp(timestamp: u64) -> String {
    let Ok(timestamp) = i64::try_from(timestamp) else {
        return String::new();
    };
    let Ok(datetime) = gtk::glib::DateTime::from_unix_local(timestamp) else {
        return String::new();
    };
    let now = gtk::glib::DateTime::now_local().ok();
    let same_day = now.is_some_and(|now| {
        now.year() == datetime.year()
            && now.month() == datetime.month()
            && now.day_of_month() == datetime.day_of_month()
    });
    datetime
        .format(if same_day { "%H:%M" } else { "%d/%m" })
        .map(|value| value.to_string())
        .unwrap_or_default()
}
