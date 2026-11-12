use std::cell::RefCell;
use std::rc::Rc;

use gtk4 as gtk;
use libadwaita as adw;
use libadwaita::prelude::*;
use qrcodegen::{QrCode, QrCodeEcc};

use crate::client::PairingStart;
use crate::error::{Error, Result};
use crate::model::{Candidate, CandidatePresentation, format_countdown, format_pin};

#[allow(deprecated)]
pub struct PairingView {
    pub dialog: adw::Dialog,
    start_button: gtk::Button,
    pin: gtk::Label,
    countdown: gtk::Label,
    qr: gtk::Picture,
    candidates: gtk::ComboBoxText,
    candidate_ids: Rc<RefCell<Vec<String>>>,
    pin_entry: gtk::Entry,
    payload: gtk::Entry,
    status: gtk::Label,
    cancel_button: gtk::Button,
    expires_at: RefCell<Option<u64>>,
}

impl PairingView {
    #[allow(deprecated)]
    pub fn new(
        on_start: impl Fn() + 'static,
        on_cancel: impl Fn() + 'static,
        on_pair_pin: impl Fn(String, String) + 'static,
        on_pair_qr: impl Fn(String) + 'static,
    ) -> Self {
        let dialog = adw::Dialog::new();
        dialog.set_title("Pair a device");
        dialog.set_content_width(460);

        let content = gtk::Box::new(gtk::Orientation::Vertical, 12);
        content.set_margin_top(16);
        content.set_margin_bottom(16);
        content.set_margin_start(18);
        content.set_margin_end(18);

        let host_title = gtk::Label::new(Some("Pair this device"));
        host_title.set_halign(gtk::Align::Start);
        host_title.add_css_class("heading");
        content.append(&host_title);

        let start_button = gtk::Button::with_label("Create pairing PIN");
        start_button.add_css_class("suggested-action");
        content.append(&start_button);

        let pin = gtk::Label::new(None);
        pin.add_css_class("title-1");
        pin.add_css_class("monospace");
        pin.set_selectable(true);
        pin.set_halign(gtk::Align::Center);
        content.append(&pin);

        let countdown = gtk::Label::new(None);
        countdown.add_css_class("caption");
        countdown.add_css_class("dim-label");
        countdown.set_halign(gtk::Align::Center);
        content.append(&countdown);

        let qr = gtk::Picture::new();
        qr.set_can_shrink(true);
        qr.set_content_fit(gtk::ContentFit::Contain);
        qr.set_size_request(220, 220);
        qr.set_halign(gtk::Align::Center);
        content.append(&qr);

        let join_title = gtk::Label::new(Some("Join another device"));
        join_title.set_halign(gtk::Align::Start);
        join_title.add_css_class("heading");
        join_title.set_margin_top(8);
        content.append(&join_title);

        let candidates = gtk::ComboBoxText::new();
        candidates.update_property(&[gtk::accessible::Property::Label("Discovered device")]);
        content.append(&candidates);

        let pin_entry = gtk::Entry::new();
        pin_entry.set_placeholder_text(Some("6-digit PIN"));
        pin_entry.update_property(&[gtk::accessible::Property::Label("Pairing PIN")]);
        pin_entry.set_input_purpose(gtk::InputPurpose::Digits);
        pin_entry.set_max_length(6);
        content.append(&pin_entry);

        let join_button = gtk::Button::with_label("Pair with PIN");
        content.append(&join_button);

        let payload = gtk::Entry::new();
        payload.set_placeholder_text(Some("QR payload"));
        payload.update_property(&[gtk::accessible::Property::Label("QR pairing payload")]);
        content.append(&payload);

        let qr_row = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        let paste_button = gtk::Button::with_label("Paste");
        let pair_qr_button = gtk::Button::with_label("Pair with QR");
        qr_row.append(&paste_button);
        qr_row.append(&pair_qr_button);
        content.append(&qr_row);

        let status = gtk::Label::new(None);
        status.set_wrap(true);
        status.set_halign(gtk::Align::Start);
        status.set_visible(false);
        content.append(&status);

        let cancel_button = gtk::Button::with_label("Close");
        cancel_button.set_halign(gtk::Align::End);
        content.append(&cancel_button);

        dialog.set_child(Some(&content));

        start_button.connect_clicked(move |_| on_start());
        cancel_button.connect_clicked(move |_| on_cancel());

        let candidate_ids = Rc::new(RefCell::new(Vec::<String>::new()));
        let ids_for_pair = candidate_ids.clone();
        let pin_for_pair = pin_entry.clone();
        let on_pair_pin = Rc::new(on_pair_pin);
        let candidates_for_pair = candidates.clone();
        join_button.connect_clicked(move |_| {
            let Some(node_id) = candidates_for_pair
                .active_id()
                .map(|id| id.to_string())
                .or_else(|| ids_for_pair.borrow().first().cloned())
            else {
                return;
            };
            let pin = pin_for_pair.text().to_string();
            if !pin.is_empty() {
                on_pair_pin(node_id, pin);
            }
        });

        let on_pair_qr = Rc::new(on_pair_qr);
        let payload_for_qr = payload.clone();
        pair_qr_button.connect_clicked(move |_| {
            let payload = payload_for_qr.text().to_string();
            if !payload.is_empty() {
                on_pair_qr(payload);
            }
        });

        let payload_for_paste = payload.clone();
        let paste_widget = paste_button.clone();
        paste_button.connect_clicked(move |_| {
            let clipboard = paste_widget.clipboard();
            let payload = payload_for_paste.clone();
            clipboard.read_text_async(None::<&gtk::gio::Cancellable>, move |result| {
                if let Ok(Some(text)) = result {
                    payload.set_text(&text);
                }
            });
        });

        Self {
            dialog,
            start_button,
            pin,
            countdown,
            qr,
            candidates,
            candidate_ids,
            pin_entry,
            payload,
            status,
            cancel_button,
            expires_at: RefCell::new(None),
        }
    }

    pub fn present(&self, parent: &impl IsA<gtk::Widget>) {
        self.dialog.present(Some(parent));
    }

    #[allow(deprecated)]
    pub fn set_candidates(&self, candidates: &[Candidate]) {
        self.candidates.remove_all();
        let mut ids = self.candidate_ids.borrow_mut();
        ids.clear();
        for candidate in candidates {
            self.candidates
                .append(Some(&candidate.node_id), &candidate.label());
            ids.push(candidate.node_id.clone());
        }
        if let Some(first) = ids.first() {
            self.candidates.set_active_id(Some(first));
        }
        drop(ids);
    }

    pub fn show_pairing(&self, pairing: &PairingStart) -> Result<()> {
        self.pin.set_text(&format_pin(&pairing.pin));
        *self.expires_at.borrow_mut() = Some(pairing.expires_at);
        self.start_button.set_sensitive(false);
        self.set_status(Some("Waiting for the other device..."), false);
        self.qr.set_paintable(Some(&qr_texture(&pairing.qr)?));
        Ok(())
    }

    pub fn tick(&self) {
        let Some(expires_at) = *self.expires_at.borrow() else {
            return;
        };
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|duration| duration.as_secs())
            .unwrap_or_default();
        let remaining = format_countdown(expires_at, now);
        if remaining == 0 {
            self.countdown.set_text("Expired");
            self.start_button.set_sensitive(true);
            self.qr.set_paintable(None::<&gtk::gdk::Paintable>);
        } else {
            self.countdown.set_text(&format!("Expires in {remaining}s"));
        }
    }

    pub fn stop_countdown(&self) {
        *self.expires_at.borrow_mut() = None;
        self.countdown.set_text("");
    }

    pub fn reset(&self) {
        self.pin.set_text("");
        self.countdown.set_text("");
        self.qr.set_paintable(None::<&gtk::gdk::Paintable>);
        self.start_button.set_sensitive(true);
        self.set_status(None, false);
        *self.expires_at.borrow_mut() = None;
    }

    pub fn set_status(&self, message: Option<&str>, error: bool) {
        match message {
            Some(message) => {
                self.status.set_text(message);
                self.status.set_visible(true);
                if error {
                    self.status.add_css_class("error");
                } else {
                    self.status.remove_css_class("error");
                }
            }
            None => {
                self.status.set_text("");
                self.status.set_visible(false);
            }
        }
    }

    pub fn close(&self) {
        self.dialog.close();
        self.pin_entry.set_text("");
        self.payload.set_text("");
    }

    pub fn set_busy(&self, busy: bool) {
        self.start_button.set_sensitive(!busy);
        self.cancel_button.set_sensitive(!busy);
    }
}

fn qr_texture(payload: &str) -> Result<gtk::gdk::Texture> {
    let code = QrCode::encode_text(payload, QrCodeEcc::Medium)
        .map_err(|error| Error::Qr(error.to_string()))?;
    let border = 4_i32;
    let scale = 6_u32;
    let modules = code.size();
    let side = u32::try_from(modules + border * 2)
        .map_err(|_| Error::Qr("QR size overflow".to_owned()))?
        * scale;
    let mut pixels = vec![255_u8; (side * side * 3) as usize];

    for y in -border..modules + border {
        for x in -border..modules + border {
            let module = x >= 0 && y >= 0 && x < modules && y < modules && code.get_module(x, y);
            if !module {
                continue;
            }
            let start_x = u32::try_from(x + border).unwrap_or_default() * scale;
            let start_y = u32::try_from(y + border).unwrap_or_default() * scale;
            for py in start_y..start_y + scale {
                for px in start_x..start_x + scale {
                    let index = ((py * side + px) * 3) as usize;
                    pixels[index] = 0;
                    pixels[index + 1] = 0;
                    pixels[index + 2] = 0;
                }
            }
        }
    }

    let mut encoded = Vec::new();
    {
        let mut encoder = png::Encoder::new(&mut encoded, side, side);
        encoder.set_color(png::ColorType::Rgb);
        encoder.set_depth(png::BitDepth::Eight);
        let mut writer = encoder
            .write_header()
            .map_err(|error| Error::Qr(error.to_string()))?;
        writer
            .write_image_data(&pixels)
            .map_err(|error| Error::Qr(error.to_string()))?;
    }
    let bytes = gtk::glib::Bytes::from_owned(encoded);
    gtk::gdk::Texture::from_bytes(&bytes).map_err(|error| Error::Qr(error.to_string()))
}
