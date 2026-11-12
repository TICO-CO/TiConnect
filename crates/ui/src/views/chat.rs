use std::rc::Rc;

use gtk::prelude::*;
use gtk4 as gtk;

use crate::model::{ComposerAction, Message, MessageStateLabel, composer_action};

pub struct ChatView {
    pub root: gtk::Box,
    pub title: gtk::Label,
    pub subtitle: gtk::Label,
    list: gtk::ListBox,
    scrolled: gtk::ScrolledWindow,
    empty: gtk::Label,
    spinner: gtk::Spinner,
    composer: gtk::TextView,
    send_button: gtk::Button,
    error: gtk::Label,
}

impl ChatView {
    pub fn new(
        on_send: impl Fn(String) + 'static,
        on_load_older: impl Fn() + 'static,
        on_mark_read: impl Fn() + 'static,
    ) -> Self {
        let root = gtk::Box::new(gtk::Orientation::Vertical, 0);
        root.set_hexpand(true);
        root.set_vexpand(true);

        let header = gtk::Box::new(gtk::Orientation::Vertical, 2);
        header.set_margin_top(10);
        header.set_margin_bottom(8);
        header.set_margin_start(16);
        header.set_margin_end(16);

        let title = gtk::Label::new(Some("Select a peer"));
        title.set_halign(gtk::Align::Start);
        title.add_css_class("heading");

        let subtitle = gtk::Label::new(None);
        subtitle.set_halign(gtk::Align::Start);
        subtitle.set_ellipsize(gtk::pango::EllipsizeMode::Middle);
        subtitle.add_css_class("caption");
        subtitle.add_css_class("dim-label");
        subtitle.add_css_class("monospace");

        header.append(&title);
        header.append(&subtitle);

        let list = gtk::ListBox::new();
        list.set_selection_mode(gtk::SelectionMode::None);
        list.set_margin_top(8);
        list.set_margin_bottom(8);
        list.set_margin_start(12);
        list.set_margin_end(12);

        let empty = gtk::Label::new(Some("No messages yet.\nSay hello."));
        empty.set_wrap(true);
        empty.set_justify(gtk::Justification::Center);
        empty.set_vexpand(true);
        empty.set_valign(gtk::Align::Center);
        empty.add_css_class("dim-label");

        let spinner = gtk::Spinner::new();
        spinner.set_vexpand(true);
        spinner.set_valign(gtk::Align::Center);
        spinner.set_visible(false);

        let scrolled = gtk::ScrolledWindow::builder()
            .vexpand(true)
            .hscrollbar_policy(gtk::PolicyType::Never)
            .child(&list)
            .build();

        let on_load_older = Rc::new(on_load_older);
        let adjustment = scrolled.vadjustment();
        let load_older = on_load_older.clone();
        adjustment.connect_value_changed(move |adjustment| {
            if adjustment.value() <= 1.0 {
                load_older();
            }
        });
        let mark_read = Rc::new(on_mark_read);
        let mark_read_scroll = mark_read.clone();
        adjustment.connect_value_changed(move |_| {
            mark_read_scroll();
        });

        let composer = gtk::TextView::new();
        composer.set_wrap_mode(gtk::WrapMode::WordChar);
        composer.set_accepts_tab(false);
        composer.set_top_margin(8);
        composer.set_bottom_margin(8);
        composer.set_left_margin(8);
        composer.set_right_margin(8);
        composer.update_property(&[gtk::accessible::Property::Label("Message composer")]);

        let composer_scroll = gtk::ScrolledWindow::builder()
            .min_content_height(54)
            .max_content_height(140)
            .hscrollbar_policy(gtk::PolicyType::Never)
            .child(&composer)
            .build();
        composer_scroll.set_hexpand(true);

        let send_button = gtk::Button::from_icon_name("document-send-symbolic");
        send_button.set_tooltip_text(Some("Send message (Enter)"));
        send_button.update_property(&[gtk::accessible::Property::Label("Send message")]);
        send_button.add_css_class("suggested-action");
        send_button.set_valign(gtk::Align::End);
        send_button.set_sensitive(false);

        let on_send = Rc::new(on_send);
        let send_from_button = on_send.clone();
        let composer_for_button = composer.clone();
        send_button.connect_clicked(move |_| {
            send_from_button(buffer_text(&composer_for_button.buffer()));
        });

        let buffer = composer.buffer();
        let send_sensitive = send_button.clone();
        let buffer_for_changed = buffer.clone();
        buffer.connect_changed(move |_| {
            send_sensitive.set_sensitive(!buffer_text(&buffer_for_changed).trim().is_empty());
        });

        let key_controller = gtk::EventControllerKey::new();
        let on_send_key = on_send.clone();
        let composer_for_key = composer.clone();
        key_controller.connect_key_pressed(move |_, key, _, state| {
            let enter = key == gtk::gdk::Key::Return || key == gtk::gdk::Key::KP_Enter;
            match composer_action(enter, state.contains(gtk::gdk::ModifierType::SHIFT_MASK)) {
                ComposerAction::Send => {
                    let text = buffer_text(&composer_for_key.buffer());
                    if !text.trim().is_empty() {
                        on_send_key(text);
                    }
                    gtk::glib::Propagation::Stop
                }
                ComposerAction::Newline => gtk::glib::Propagation::Proceed,
                ComposerAction::Ignore => gtk::glib::Propagation::Proceed,
            }
        });
        composer.add_controller(key_controller);

        let error = gtk::Label::new(None);
        error.set_wrap(true);
        error.set_halign(gtk::Align::Start);
        error.add_css_class("error");
        error.set_visible(false);

        let composer_bar = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        composer_bar.set_margin_start(12);
        composer_bar.set_margin_end(12);
        composer_bar.set_margin_bottom(12);
        composer_bar.set_margin_top(6);
        composer_bar.append(&composer_scroll);
        composer_bar.append(&send_button);

        root.append(&header);
        root.append(&scrolled);
        root.append(&empty);
        root.append(&spinner);
        root.append(&error);
        root.append(&composer_bar);

        Self {
            root,
            title,
            subtitle,
            list,
            scrolled,
            empty,
            spinner,
            composer,
            send_button,
            error,
        }
    }

    pub fn set_peer(&self, title: &str, fingerprint: &str) {
        self.title.set_text(title);
        self.subtitle.set_text(fingerprint);
    }

    pub fn render(&self, messages: &[Message], local_node_id: Option<&str>) {
        while let Some(child) = self.list.first_child() {
            self.list.remove(&child);
        }
        self.empty.set_visible(messages.is_empty());
        for message in messages {
            let local = local_node_id.is_some_and(|local| message.sender_id == local);
            self.list.append(&message_row(message, local));
        }
    }

    pub fn set_loading(&self, loading: bool) {
        self.spinner.set_visible(loading);
        if loading {
            self.spinner.start();
        } else {
            self.spinner.stop();
        }
    }

    pub fn set_composer_enabled(&self, enabled: bool) {
        self.composer.set_sensitive(enabled);
        self.send_button
            .set_sensitive(enabled && !buffer_text(&self.composer.buffer()).trim().is_empty());
    }

    pub fn clear_composer(&self) {
        self.composer.buffer().set_text("");
    }

    pub fn composer_text(&self) -> String {
        buffer_text(&self.composer.buffer())
    }

    pub fn set_error(&self, message: Option<&str>) {
        match message {
            Some(message) => {
                self.error.set_text(message);
                self.error.set_visible(true);
            }
            None => {
                self.error.set_text("");
                self.error.set_visible(false);
            }
        }
    }

    pub fn scroll_to_bottom(&self) {
        let adjustment = self.scrolled.vadjustment();
        let upper = adjustment.upper();
        adjustment.set_value(upper);
    }

    pub fn scroll_value(&self) -> f64 {
        self.scrolled.vadjustment().value()
    }

    pub fn set_scroll_value(&self, value: f64) {
        self.scrolled.vadjustment().set_value(value);
    }

    pub fn set_composer_text(&self, text: &str) {
        self.composer.buffer().set_text(text);
    }
}

fn message_row(message: &Message, local: bool) -> gtk::ListBoxRow {
    let row = gtk::ListBoxRow::new();
    row.set_selectable(false);
    row.set_activatable(false);
    row.update_property(&[gtk::accessible::Property::Label(&format!(
        "{} message: {}",
        if local { "Sent" } else { "Received" },
        message.body
    ))]);

    let outer = gtk::Box::new(gtk::Orientation::Vertical, 2);
    outer.set_halign(if local {
        gtk::Align::End
    } else {
        gtk::Align::Start
    });

    let bubble = gtk::Label::new(Some(&message.body));
    bubble.set_wrap(true);
    bubble.set_xalign(0.0);
    bubble.set_selectable(true);
    bubble.set_max_width_chars(64);
    bubble.add_css_class("message-bubble");
    bubble.add_css_class(if local {
        "message-outgoing"
    } else {
        "message-incoming"
    });

    let meta = gtk::Label::new(Some(&format!(
        "{} {}",
        format_time(message.sent_at),
        message.state.label()
    )));
    meta.set_xalign(if local { 1.0 } else { 0.0 });
    meta.add_css_class("caption");
    meta.add_css_class("dim-label");
    if message.state == crate::model::MessageState::Failed {
        meta.add_css_class("error");
    }

    outer.append(&bubble);
    outer.append(&meta);
    row.set_child(Some(&outer));
    row
}

fn buffer_text(buffer: &gtk::TextBuffer) -> String {
    let start = buffer.start_iter();
    let end = buffer.end_iter();
    buffer.text(&start, &end, false).to_string()
}

fn format_time(timestamp: u64) -> String {
    let Ok(timestamp) = i64::try_from(timestamp) else {
        return String::new();
    };
    let Ok(datetime) = gtk::glib::DateTime::from_unix_local(timestamp) else {
        return String::new();
    };
    datetime
        .format("%H:%M")
        .map(|value| value.to_string())
        .unwrap_or_default()
}
