use gtk::prelude::*;
use gtk4 as gtk;
use gtk4_layer_shell as layer;
use gtk4_layer_shell::LayerShell;

use crate::card::{Card, ClickHandler, HoverHandler, render_cards};

pub struct Surface {
    window: gtk::Window,
    cards: gtk::Box,
}

impl Surface {
    pub fn new(application: &gtk::Application) -> Option<Self> {
        if !layer::is_supported() {
            return None;
        }

        let window = gtk::Window::builder()
            .application(application)
            .decorated(false)
            .resizable(false)
            .build();
        window.add_css_class("ticonnect-overlay");
        window.init_layer_shell();
        window.set_anchor(layer::Edge::Bottom, true);
        window.set_anchor(layer::Edge::Right, true);
        window.set_layer(layer::Layer::Overlay);
        window.set_keyboard_mode(layer::KeyboardMode::None);
        window.set_exclusive_zone(0);
        window.set_margin(layer::Edge::Bottom, 24);
        window.set_margin(layer::Edge::Right, 24);
        window.set_namespace(Some("ticonnect-overlay"));

        let cards = gtk::Box::new(gtk::Orientation::Vertical, 8);
        cards.set_valign(gtk::Align::End);
        cards.set_halign(gtk::Align::End);
        window.set_child(Some(&cards));
        window.set_visible(false);

        Some(Self { window, cards })
    }

    pub fn render(&self, cards: &[Card], on_click: &ClickHandler, on_hover: &HoverHandler) {
        render_cards(&self.cards, cards, on_click, on_hover);
        if cards.is_empty() {
            self.window.set_visible(false);
        } else {
            self.window.present();
        }
    }

    pub fn window(&self) -> &gtk::Window {
        &self.window
    }
}
