use std::cell::RefCell;
use std::rc::Rc;

use gtk::prelude::*;
use gtk4 as gtk;

pub struct RemoteView {
    pub root: gtk::Box,
    title: gtk::Label,
    status: gtk::Label,
    pin: gtk::Entry,
    accept: gtk::Button,
    deny: gtk::Button,
    end: gtk::Button,
    frame: gtk::Picture,
    indicator: gtk::Label,
    session_id: Rc<RefCell<String>>,
}

impl RemoteView {
    pub fn new(
        on_accept: impl Fn(String, String) + 'static,
        on_deny: impl Fn(String) + 'static,
        on_end: impl Fn(String) + 'static,
    ) -> Self {
        let root = gtk::Box::new(gtk::Orientation::Vertical, 12);
        root.set_margin_top(16);
        root.set_margin_bottom(16);
        root.set_margin_start(16);
        root.set_margin_end(16);
        root.set_vexpand(true);

        let title = gtk::Label::new(Some("Remote desktop"));
        title.add_css_class("title-2");
        title.set_halign(gtk::Align::Start);

        let indicator = gtk::Label::new(Some("Remote session active"));
        indicator.add_css_class("error");
        indicator.set_visible(false);
        indicator.set_halign(gtk::Align::Start);

        let status = gtk::Label::new(Some("No remote session"));
        status.add_css_class("dim-label");
        status.set_halign(gtk::Align::Start);

        let frame = gtk::Picture::new();
        frame.set_content_fit(gtk::ContentFit::Contain);
        frame.set_can_shrink(true);
        frame.set_vexpand(true);
        frame.set_halign(gtk::Align::Center);

        let pin = gtk::Entry::new();
        pin.set_placeholder_text(Some("Session PIN"));
        pin.set_max_length(6);
        pin.set_visibility(false);
        pin.set_width_chars(8);
        pin.update_property(&[gtk::accessible::Property::Label("Session PIN")]);

        let accept = gtk::Button::with_label("Accept");
        accept.add_css_class("suggested-action");
        let deny = gtk::Button::with_label("Deny");
        deny.add_css_class("destructive-action");
        let end = gtk::Button::with_label("Kill remote session");
        end.add_css_class("destructive-action");

        let controls = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        controls.append(&pin);
        controls.append(&accept);
        controls.append(&deny);
        controls.append(&end);

        root.append(&title);
        root.append(&indicator);
        root.append(&status);
        root.append(&frame);
        root.append(&controls);

        let session_id = Rc::new(RefCell::new(String::new()));
        {
            let session_id = session_id.clone();
            let pin = pin.clone();
            accept.connect_clicked(move |_| {
                on_accept(session_id.borrow().clone(), pin.text().to_string());
            });
        }
        {
            let session_id = session_id.clone();
            deny.connect_clicked(move |_| on_deny(session_id.borrow().clone()));
        }
        {
            let session_id = session_id.clone();
            end.connect_clicked(move |_| on_end(session_id.borrow().clone()));
        }

        Self {
            root,
            title,
            status,
            pin,
            accept,
            deny,
            end,
            frame,
            indicator,
            session_id,
        }
    }

    pub fn show_request(&self, session_id: &str, peer: &str, fingerprint: &str, pin: &str) {
        *self.session_id.borrow_mut() = session_id.to_owned();
        self.title.set_text(&format!("Remote request from {peer}"));
        self.status.set_text(&format!(
            "Fingerprint: {fingerprint}\nShare this PIN with the viewer: {pin}"
        ));
        self.pin.set_text(pin);
        self.pin.set_visible(false);
        self.accept.set_visible(true);
        self.deny.set_visible(true);
        self.end.set_visible(false);
        self.frame.set_visible(false);
        self.indicator.set_visible(false);
        self.root.set_visible(true);
    }

    pub fn show_viewer(&self, session_id: &str, peer: &str) {
        *self.session_id.borrow_mut() = session_id.to_owned();
        self.title.set_text(&format!("Remote session with {peer}"));
        self.status
            .set_text("Enter the PIN shown on the host, then wait for consent.");
        self.pin.set_text("");
        self.pin.set_visible(true);
        self.accept.set_visible(true);
        self.deny.set_visible(true);
        self.end.set_visible(false);
        self.frame.set_visible(false);
        self.indicator.set_visible(false);
        self.root.set_visible(true);
    }

    pub fn show_active(&self, session_id: &str, peer: &str) {
        *self.session_id.borrow_mut() = session_id.to_owned();
        self.title.set_text(&format!("Remote · {peer}"));
        self.status
            .set_text("Connected. Input is injected only while this session is active.");
        self.pin.set_visible(false);
        self.accept.set_visible(false);
        self.deny.set_visible(false);
        self.end.set_visible(true);
        self.frame.set_visible(true);
        self.indicator.set_visible(true);
        self.root.set_visible(true);
    }

    pub fn set_status(&self, status: &str) {
        self.status.set_text(status);
    }

    pub fn hide(&self) {
        self.root.set_visible(false);
    }
}
