use std::cell::RefCell;
use std::rc::Rc;

use gtk::prelude::*;
use gtk4 as gtk;

pub struct CallView {
    pub root: gtk::Box,
    title: gtk::Label,
    state: gtk::Label,
    duration: gtk::Label,
    accept: gtk::Button,
    decline: gtk::Button,
    hangup: gtk::Button,
    mute: gtk::ToggleButton,
    muted: Rc<RefCell<bool>>,
}

impl CallView {
    pub fn new(
        on_accept: impl Fn() + 'static,
        on_decline: impl Fn() + 'static,
        on_hangup: impl Fn() + 'static,
    ) -> Self {
        let root = gtk::Box::new(gtk::Orientation::Vertical, 12);
        root.set_margin_top(24);
        root.set_margin_bottom(24);
        root.set_margin_start(24);
        root.set_margin_end(24);
        root.set_halign(gtk::Align::Center);
        root.set_valign(gtk::Align::Center);
        root.set_vexpand(true);

        let icon = gtk::Image::from_icon_name("call-start-symbolic");
        icon.set_pixel_size(48);
        icon.add_css_class("dim-label");

        let title = gtk::Label::new(Some("Call"));
        title.add_css_class("title-1");

        let state = gtk::Label::new(Some("idle"));
        state.add_css_class("dim-label");

        let duration = gtk::Label::new(Some("00:00"));
        duration.add_css_class("title-3");
        duration.add_css_class("monospace");

        let accept = gtk::Button::with_label("Accept");
        accept.add_css_class("suggested-action");
        let decline = gtk::Button::with_label("Decline");
        decline.add_css_class("destructive-action");
        let hangup = gtk::Button::with_label("Hang up");
        hangup.add_css_class("destructive-action");
        let mute = gtk::ToggleButton::with_label("Mute");
        let muted = Rc::new(RefCell::new(false));
        {
            let muted = muted.clone();
            mute.connect_toggled(move |button| {
                *muted.borrow_mut() = button.is_active();
            });
        }

        let controls = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        controls.set_halign(gtk::Align::Center);
        controls.append(&mute);
        controls.append(&accept);
        controls.append(&decline);
        controls.append(&hangup);

        root.append(&icon);
        root.append(&title);
        root.append(&state);
        root.append(&duration);
        root.append(&controls);

        accept.connect_clicked(move |_| on_accept());
        decline.connect_clicked(move |_| on_decline());
        hangup.connect_clicked(move |_| on_hangup());

        Self {
            root,
            title,
            state,
            duration,
            accept,
            decline,
            hangup,
            mute,
            muted,
        }
    }

    pub fn show_incoming(&self, peer: &str) {
        self.title.set_text(peer);
        self.state.set_text("Incoming call");
        self.duration.set_text("00:00");
        self.accept.set_visible(true);
        self.decline.set_visible(true);
        self.hangup.set_visible(false);
        self.mute.set_visible(false);
        self.root.set_visible(true);
    }

    pub fn show_ringing(&self, peer: &str) {
        self.title.set_text(peer);
        self.state.set_text("Ringing");
        self.duration.set_text("00:00");
        self.accept.set_visible(false);
        self.decline.set_visible(true);
        self.hangup.set_visible(false);
        self.mute.set_visible(false);
        self.root.set_visible(true);
    }

    pub fn show_active(&self, peer: &str, state: &str) {
        self.title.set_text(peer);
        self.state.set_text(state);
        self.accept.set_visible(false);
        self.decline.set_visible(false);
        self.hangup.set_visible(true);
        self.mute.set_visible(true);
        self.root.set_visible(true);
    }

    pub fn set_duration(&self, seconds: u64) {
        self.duration
            .set_text(&format!("{:02}:{:02}", seconds / 60, seconds % 60));
    }

    pub fn muted(&self) -> bool {
        *self.muted.borrow()
    }

    pub fn hide(&self) {
        self.root.set_visible(false);
    }
}
