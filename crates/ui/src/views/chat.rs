use std::cell::Cell;
use std::rc::Rc;

use gtk4 as gtk;
use libadwaita::prelude::*;

use crate::model::{ComposerAction, Message, MessageState, MessageStateLabel, composer_action};
use crate::views::peers::{avatar_widget, format_timestamp};

pub struct ChatView {
    pub root: gtk::Box,
    pub title: gtk::Label,
    pub subtitle: gtk::Label,
    avatar_slot: gtk::Box,
    list: gtk::ListBox,
    scrolled: gtk::ScrolledWindow,
    empty: gtk::Label,
    spinner: gtk::Spinner,
    composer: gtk::TextView,
    send_button: gtk::Button,
    error: gtk::Label,
    on_typing: Rc<dyn Fn(bool)>,
    typing_active: Rc<Cell<bool>>,
    typing_generation: Rc<Cell<u64>>,
}

impl ChatView {
    pub fn new(
        on_send: impl Fn(String) + 'static,
        on_load_older: impl Fn() + 'static,
        on_mark_read: impl Fn() + 'static,
        on_attach: impl Fn() + 'static,
        on_typing: impl Fn(bool) + 'static,
    ) -> Self {
        let root = gtk::Box::new(gtk::Orientation::Vertical, 0);
        root.set_hexpand(true);
        root.set_vexpand(true);

        let header = gtk::Box::new(gtk::Orientation::Horizontal, 12);
        header.add_css_class("chat-header");
        header.set_margin_top(10);
        header.set_margin_bottom(8);
        header.set_margin_start(16);
        header.set_margin_end(16);

        let avatar_slot = gtk::Box::new(gtk::Orientation::Horizontal, 0);
        avatar_slot.set_valign(gtk::Align::Center);

        let title_box = gtk::Box::new(gtk::Orientation::Vertical, 1);
        title_box.set_hexpand(true);

        let title = gtk::Label::new(Some("Select a peer"));
        title.set_halign(gtk::Align::Start);
        title.add_css_class("chat-title");

        let subtitle = gtk::Label::new(None);
        subtitle.set_halign(gtk::Align::Start);
        subtitle.set_ellipsize(gtk::pango::EllipsizeMode::Middle);
        subtitle.add_css_class("caption");
        subtitle.add_css_class("dim-label");

        title_box.append(&title);
        title_box.append(&subtitle);

        header.append(&avatar_slot);
        header.append(&title_box);

        let list = gtk::ListBox::new();
        list.set_selection_mode(gtk::SelectionMode::None);
        list.add_css_class("message-list");
        list.set_margin_top(6);
        list.set_margin_bottom(6);
        list.set_margin_start(14);
        list.set_margin_end(14);

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
        composer.set_left_margin(10);
        composer.set_right_margin(10);
        composer.add_css_class("composer-input");
        composer.update_property(&[gtk::accessible::Property::Label("Message composer")]);

        let composer_scroll = gtk::ScrolledWindow::builder()
            .min_content_height(46)
            .max_content_height(140)
            .hscrollbar_policy(gtk::PolicyType::Never)
            .child(&composer)
            .build();
        composer_scroll.set_hexpand(true);
        composer_scroll.add_css_class("composer-input-frame");

        let attach_button = gtk::Button::from_icon_name("mail-attachment-symbolic");
        attach_button.set_tooltip_text(Some("Attach a file"));
        attach_button.update_property(&[gtk::accessible::Property::Label("Attach a file")]);
        attach_button.set_valign(gtk::Align::End);
        attach_button.add_css_class("flat");
        attach_button.connect_clicked(move |_| on_attach());

        let send_button = gtk::Button::from_icon_name("document-send-symbolic");
        send_button.set_tooltip_text(Some("Send message (Enter)"));
        send_button.update_property(&[gtk::accessible::Property::Label("Send message")]);
        send_button.add_css_class("suggested-action");
        send_button.add_css_class("send-button");
        send_button.set_valign(gtk::Align::End);
        send_button.set_sensitive(false);

        let on_send: Rc<dyn Fn(String)> = Rc::new(on_send);
        let on_typing: Rc<dyn Fn(bool)> = Rc::new(on_typing);
        let typing_active = Rc::new(Cell::new(false));
        let typing_generation: Rc<Cell<u64>> = Rc::new(Cell::new(0));
        let send_from_button = on_send.clone();
        let composer_for_button = composer.clone();
        let typing_stop = on_typing.clone();
        let typing_active_for_button = typing_active.clone();
        let generation_button = typing_generation.clone();
        send_button.connect_clicked(move |_| {
            send_from_button(buffer_text(&composer_for_button.buffer()));
            stop_typing(&typing_stop, &typing_active_for_button, &generation_button);
        });

        let buffer = composer.buffer();
        let send_sensitive = send_button.clone();
        let buffer_for_changed = buffer.clone();
        let typing_callback = on_typing.clone();
        let typing_active_changed = typing_active.clone();
        let generation_changed = typing_generation.clone();
        buffer.connect_changed(move |_| {
            let text = buffer_text(&buffer_for_changed);
            let has_text = !text.trim().is_empty();
            send_sensitive.set_sensitive(has_text);
            update_typing(
                &typing_callback,
                &typing_active_changed,
                &generation_changed,
                has_text,
            );
        });

        let focus = gtk::EventControllerFocus::new();
        let typing_focus = on_typing.clone();
        let active_focus = typing_active.clone();
        let generation_focus = typing_generation.clone();
        focus.connect_leave(move |_| {
            stop_typing(&typing_focus, &active_focus, &generation_focus);
        });
        composer.add_controller(focus);

        let key_controller = gtk::EventControllerKey::new();
        let on_send_key = on_send.clone();
        let composer_for_key = composer.clone();
        let typing_key = on_typing.clone();
        let active_key = typing_active.clone();
        let generation_key = typing_generation.clone();
        key_controller.connect_key_pressed(move |_, key, _, state| {
            let enter = key == gtk::gdk::Key::Return || key == gtk::gdk::Key::KP_Enter;
            match composer_action(enter, state.contains(gtk::gdk::ModifierType::SHIFT_MASK)) {
                ComposerAction::Send => {
                    let text = buffer_text(&composer_for_key.buffer());
                    if !text.trim().is_empty() {
                        on_send_key(text);
                        stop_typing(&typing_key, &active_key, &generation_key);
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
        composer_bar.add_css_class("composer-bar");
        composer_bar.set_margin_start(12);
        composer_bar.set_margin_end(12);
        composer_bar.set_margin_bottom(12);
        composer_bar.set_margin_top(6);
        composer_bar.append(&composer_scroll);
        composer_bar.append(&attach_button);
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
            avatar_slot,
            list,
            scrolled,
            empty,
            spinner,
            composer,
            send_button,
            error,
            on_typing,
            typing_active,
            typing_generation,
        }
    }

    pub fn set_peer(
        &self,
        title: &str,
        fingerprint: &str,
        avatar: Option<&gtk::gdk::Texture>,
        connected: bool,
        typing: bool,
    ) {
        self.title.set_text(title);
        self.subtitle.set_text(if typing {
            "typing…"
        } else if connected {
            "Online"
        } else {
            fingerprint
        });
        self.subtitle.set_tooltip_text(Some(fingerprint));
        self.subtitle.remove_css_class("typing-label");
        if typing {
            self.subtitle.add_css_class("typing-label");
        }
        while let Some(child) = self.avatar_slot.first_child() {
            self.avatar_slot.remove(&child);
        }
        let initials = initials(title);
        let avatar = avatar_widget(avatar, &initials, 36);
        avatar.set_valign(gtk::Align::Center);
        self.avatar_slot.append(&avatar);
    }

    pub fn render(
        &self,
        messages: &[Message],
        local_node_id: Option<&str>,
        peer_avatar: Option<&gtk::gdk::Texture>,
        local_avatar: Option<&gtk::gdk::Texture>,
        peer_initials: &str,
    ) {
        while let Some(child) = self.list.first_child() {
            self.list.remove(&child);
        }
        self.empty.set_visible(messages.is_empty());
        let mut previous_day = String::new();
        for (index, message) in messages.iter().enumerate() {
            let day = format_day(message.sent_at);
            if day != previous_day && !day.is_empty() {
                self.list.append(&day_row(&day));
                previous_day = day;
            }
            let local = local_node_id.is_some_and(|local| message.sender_id == local);
            let next_same_sender = messages.get(index + 1).is_some_and(|next| {
                next.sender_id == message.sender_id
                    && next.sent_at.saturating_sub(message.sent_at) < 300
                    && format_day(next.sent_at) == format_day(message.sent_at)
            });
            let avatar = if local { local_avatar } else { peer_avatar };
            self.list.append(&message_row(
                message,
                local,
                avatar,
                !next_same_sender,
                peer_initials,
            ));
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
        stop_typing(
            &self.on_typing,
            &self.typing_active,
            &self.typing_generation,
        );
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

fn update_typing(
    callback: &Rc<dyn Fn(bool)>,
    active: &Rc<Cell<bool>>,
    generation: &Rc<Cell<u64>>,
    has_text: bool,
) {
    let next = generation.get().wrapping_add(1);
    generation.set(next);
    if !has_text {
        if active.replace(false) {
            callback(false);
        }
        return;
    }
    if !active.get() {
        active.set(true);
        callback(true);
    }
    let callback = callback.clone();
    let active = active.clone();
    let generation = generation.clone();
    gtk::glib::timeout_add_seconds_local_once(5, move || {
        if generation.get() == next && active.replace(false) {
            callback(false);
        }
    });
}

fn stop_typing(callback: &Rc<dyn Fn(bool)>, active: &Rc<Cell<bool>>, generation: &Rc<Cell<u64>>) {
    generation.set(generation.get().wrapping_add(1));
    if active.replace(false) {
        callback(false);
    }
}

fn day_row(day: &str) -> gtk::ListBoxRow {
    let row = gtk::ListBoxRow::new();
    row.set_selectable(false);
    row.set_activatable(false);
    let label = gtk::Label::new(Some(day));
    label.add_css_class("day-separator");
    label.set_halign(gtk::Align::Center);
    label.set_margin_top(10);
    label.set_margin_bottom(6);
    row.set_child(Some(&label));
    row
}

fn message_row(
    message: &Message,
    local: bool,
    avatar: Option<&gtk::gdk::Texture>,
    show_avatar: bool,
    peer_initials: &str,
) -> gtk::ListBoxRow {
    let row = gtk::ListBoxRow::new();
    row.set_selectable(false);
    row.set_activatable(false);
    row.add_css_class("message-row");
    row.update_property(&[gtk::accessible::Property::Label(&format!(
        "{} message: {}",
        if local { "Sent" } else { "Received" },
        message.body
    ))]);

    let line = gtk::Box::new(gtk::Orientation::Horizontal, if local { 0 } else { 8 });
    line.set_halign(if local {
        gtk::Align::End
    } else {
        gtk::Align::Start
    });

    if !local {
        let slot = gtk::Box::new(gtk::Orientation::Horizontal, 0);
        slot.set_valign(gtk::Align::End);
        slot.set_size_request(30, -1);
        if show_avatar {
            let avatar = avatar_widget(avatar, peer_initials, 26);
            avatar.set_valign(gtk::Align::End);
            slot.append(&avatar);
        }
        line.append(&slot);
    }

    let bubble = gtk::Box::new(gtk::Orientation::Vertical, 3);
    bubble.add_css_class("message-bubble");
    bubble.add_css_class(if local {
        "message-outgoing"
    } else {
        "message-incoming"
    });
    if !message.attachment_name.is_empty() {
        bubble.append(&attachment_widget(message));
    }
    if !message.body.is_empty() {
        let body = gtk::Label::new(Some(&message.body));
        body.set_wrap(true);
        body.set_xalign(0.0);
        body.set_selectable(true);
        body.set_max_width_chars(60);
        bubble.append(&body);
    }

    let meta = gtk::Box::new(gtk::Orientation::Horizontal, 4);
    meta.add_css_class("message-meta");
    meta.set_halign(gtk::Align::End);
    let time = gtk::Label::new(Some(&format_timestamp(message.sent_at)));
    time.add_css_class("caption");
    time.add_css_class("dim-label");
    meta.append(&time);
    if local {
        meta.append(&state_widget(message.state));
    }
    bubble.append(&meta);

    line.append(&bubble);
    row.set_child(Some(&line));
    row
}

fn state_widget(state: MessageState) -> gtk::Widget {
    let (text, class) = match state {
        MessageState::Queued => ("◷", "tick-pending"),
        MessageState::Sent => ("✓", "tick-sent"),
        MessageState::Delivered => ("✓✓", "tick-delivered"),
        MessageState::Read => ("✓✓", "tick-read"),
        MessageState::Failed => ("!", "tick-failed"),
        MessageState::Expired => ("◷", "tick-failed"),
        MessageState::Unknown => ("", "tick-pending"),
    };
    let label = gtk::Label::new(Some(text));
    label.add_css_class("tick");
    label.add_css_class(class);
    label.set_tooltip_text(Some(state.label()));
    label.update_property(&[gtk::accessible::Property::Label(state.label())]);
    label.upcast()
}

fn attachment_widget(message: &Message) -> gtk::Widget {
    let container = gtk::Box::new(gtk::Orientation::Vertical, 4);
    if message.attachment_mime.starts_with("image/") && !message.attachment_path.is_empty() {
        let picture = gtk::Picture::for_filename(&message.attachment_path);
        picture.set_can_shrink(true);
        picture.set_content_fit(gtk::ContentFit::Contain);
        picture.set_size_request(320, 240);
        picture.add_css_class("message-image");
        container.append(&picture);
    }
    if message.attachment_mime.starts_with("audio/") && !message.attachment_path.is_empty() {
        let media = gtk::MediaFile::for_filename(&message.attachment_path);
        let video = gtk::Video::new();
        video.set_media_stream(Some(&media));
        video.set_height_request(56);
        container.append(&video);
    }
    let label = gtk::Label::new(Some(&format!(
        "{} · {}",
        message.attachment_name,
        format_size(message.attachment_size)
    )));
    label.set_xalign(0.0);
    label.set_ellipsize(gtk::pango::EllipsizeMode::Middle);
    label.add_css_class("caption");
    container.append(&label);
    container.upcast()
}

fn format_size(bytes: u64) -> String {
    const UNITS: [&str; 5] = ["B", "KiB", "MiB", "GiB", "TiB"];
    let mut value = bytes as f64;
    let mut unit = 0;
    while value >= 1024.0 && unit < UNITS.len() - 1 {
        value /= 1024.0;
        unit += 1;
    }
    if unit == 0 {
        format!("{bytes} {}", UNITS[unit])
    } else {
        format!("{value:.1} {}", UNITS[unit])
    }
}

fn buffer_text(buffer: &gtk::TextBuffer) -> String {
    let start = buffer.start_iter();
    let end = buffer.end_iter();
    buffer.text(&start, &end, false).to_string()
}

fn format_day(timestamp: u64) -> String {
    let Ok(timestamp) = i64::try_from(timestamp) else {
        return String::new();
    };
    let Ok(datetime) = gtk::glib::DateTime::from_unix_local(timestamp) else {
        return String::new();
    };
    let now = gtk::glib::DateTime::now_local().ok();
    if now.is_some_and(|now| {
        now.year() == datetime.year()
            && now.month() == datetime.month()
            && now.day_of_month() == datetime.day_of_month()
    }) {
        return "Today".to_owned();
    }
    datetime
        .format("%A, %e %B")
        .map(|value| value.to_string())
        .unwrap_or_default()
}

fn initials(label: &str) -> String {
    let mut parts = label.split_whitespace();
    let first = parts.next().and_then(|part| part.chars().next());
    let last = parts.last().and_then(|part| part.chars().next());
    match (first, last) {
        (Some(first), Some(last)) => format!("{first}{last}").to_uppercase(),
        (Some(first), None) => first.to_uppercase().to_string(),
        _ => "?".to_owned(),
    }
}
