use gtk::prelude::*;
use gtk4 as gtk;
use gtk4_layer_shell as layer;
use gtk4_layer_shell::LayerShell;

use crate::card::{ActionHandler, Card, ClickHandler, HoverHandler, render_cards};

pub struct Surface {
    window: gtk::Window,
    cards: gtk::Box,
}

impl Surface {
    pub fn new(application: &gtk::Application, namespace: &str, bottom: bool) -> Option<Self> {
        if !layer::is_supported() {
            return None;
        }

        let window = gtk::Window::builder()
            .application(application)
            .decorated(false)
            .resizable(false)
            .build();
        window.add_css_class("ticonnect-overlay");
        window.add_css_class(if bottom {
            "ticonnect-overlay-bottom"
        } else {
            "ticonnect-overlay-top"
        });
        window.init_layer_shell();
        window.set_anchor(layer::Edge::Right, true);
        window.set_anchor(layer::Edge::Top, !bottom);
        window.set_anchor(layer::Edge::Bottom, bottom);
        window.set_layer(layer::Layer::Overlay);
        window.set_keyboard_mode(layer::KeyboardMode::None);
        window.set_exclusive_zone(0);
        let margin_edge = if bottom {
            layer::Edge::Bottom
        } else {
            layer::Edge::Top
        };
        window.set_margin(margin_edge, 24);
        window.set_margin(layer::Edge::Right, 24);
        window.set_namespace(Some(namespace));

        let cards = gtk::Box::new(gtk::Orientation::Vertical, 8);
        cards.set_valign(if bottom {
            gtk::Align::End
        } else {
            gtk::Align::Start
        });
        cards.set_halign(gtk::Align::End);
        window.set_child(Some(&cards));
        window.set_visible(false);

        Some(Self { window, cards })
    }

    pub fn render(
        &self,
        cards: &[Card],
        on_click: &ClickHandler,
        on_hover: &HoverHandler,
        on_action: &ActionHandler,
    ) {
        render_cards(&self.cards, cards, on_click, on_hover, on_action);
        if cards.is_empty() {
            self.window.set_visible(false);
        } else {
            self.window.present();
        }
    }

    pub fn set_bottom_margin(&self, margin: i32) {
        self.window.set_margin(layer::Edge::Bottom, margin);
    }

    pub fn window(&self) -> &gtk::Window {
        &self.window
    }
}
