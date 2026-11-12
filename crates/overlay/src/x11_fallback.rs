use std::time::Duration;

use gtk::prelude::*;
use gtk4 as gtk;
use x11rb::connection::Connection;
use x11rb::protocol::xproto::{AtomEnum, ConfigureWindowAux, ConnectionExt, PropMode};
use x11rb::rust_connection::RustConnection;
use x11rb::wrapper::ConnectionExt as _;

use crate::card::{Card, ClickHandler, HoverHandler, render_cards};

const MARGIN: i32 = 24;
const TITLE: &str = "TiConnect Overlay";

pub struct Surface {
    window: gtk::Window,
    cards: gtk::Box,
}

impl Surface {
    pub fn new(application: &gtk::Application) -> Self {
        let window = gtk::Window::builder()
            .application(application)
            .decorated(false)
            .resizable(false)
            .build();
        window.add_css_class("ticonnect-x11-overlay");
        window.set_title(Some(TITLE));
        window.set_focusable(false);

        let cards = gtk::Box::new(gtk::Orientation::Vertical, 8);
        cards.set_valign(gtk::Align::End);
        cards.set_halign(gtk::Align::End);
        window.set_child(Some(&cards));
        window.set_visible(false);

        let surface_window = window.clone();
        window.connect_map(move |_| {
            let window = surface_window.clone();
            gtk::glib::timeout_add_local_once(Duration::from_millis(100), move || {
                if let Err(error) = apply_hints(&window) {
                    tracing::warn!(%error, "failed to apply X11 overlay hints");
                }
            });
        });

        Self { window, cards }
    }

    pub fn render(&self, cards: &[Card], on_click: &ClickHandler, on_hover: &HoverHandler) {
        render_cards(&self.cards, cards, on_click, on_hover);
        if cards.is_empty() {
            self.window.set_visible(false);
        } else {
            self.window.present();
            let window = self.window.clone();
            gtk::glib::timeout_add_local_once(Duration::from_millis(50), move || {
                if let Err(error) = position(&window) {
                    tracing::warn!(%error, "failed to position X11 overlay");
                }
            });
        }
    }

    pub fn window(&self) -> &gtk::Window {
        &self.window
    }
}

fn apply_hints(window: &gtk::Window) -> anyhow::Result<()> {
    let Some((connection, window_id)) = x11_window(window) else {
        return Ok(());
    };
    let above = intern(&connection, b"_NET_WM_STATE_ABOVE")?;
    let skip_taskbar = intern(&connection, b"_NET_WM_STATE_SKIP_TASKBAR")?;
    let skip_pager = intern(&connection, b"_NET_WM_STATE_SKIP_PAGER")?;
    let state = intern(&connection, b"_NET_WM_STATE")?;
    let window_type = intern(&connection, b"_NET_WM_WINDOW_TYPE")?;
    let notification = intern(&connection, b"_NET_WM_WINDOW_TYPE_NOTIFICATION")?;

    connection.change_property32(
        PropMode::REPLACE,
        window_id,
        state,
        AtomEnum::ATOM,
        &[above, skip_taskbar, skip_pager],
    )?;
    connection.change_property32(
        PropMode::REPLACE,
        window_id,
        window_type,
        AtomEnum::ATOM,
        &[notification],
    )?;
    connection.flush()?;
    Ok(())
}

fn position(window: &gtk::Window) -> anyhow::Result<()> {
    let Some((connection, window_id)) = x11_window(window) else {
        return Ok(());
    };
    let Some(display) = gtk::gdk::Display::default() else {
        return Ok(());
    };
    let Some(monitor) = display.monitors().item(0) else {
        return Ok(());
    };
    let Ok(monitor) = monitor.downcast::<gtk::gdk::Monitor>() else {
        return Ok(());
    };
    let geometry = monitor.geometry();
    let scale = monitor.scale_factor();
    let width = window.width().max(320) * scale;
    let height = window.height().max(80) * scale;
    let x = geometry.x() * scale + geometry.width() - width - MARGIN * scale;
    let y = geometry.y() * scale + geometry.height() - height - MARGIN * scale;

    connection.configure_window(
        window_id,
        &ConfigureWindowAux::new().x(x.max(0)).y(y.max(0)),
    )?;
    connection.flush()?;
    Ok(())
}

fn x11_window(_window: &gtk::Window) -> Option<(RustConnection, u32)> {
    let (connection, screen) = x11rb::connect(None).ok()?;
    let root = connection.setup().roots.get(screen)?.root;
    let tree = connection.query_tree(root).ok()?.reply().ok()?;
    for window_id in tree.children {
        let Some(title) = window_title(&connection, window_id) else {
            continue;
        };
        if title == TITLE.as_bytes() {
            return Some((connection, window_id));
        }
    }
    None
}

fn window_title(connection: &RustConnection, window_id: u32) -> Option<Vec<u8>> {
    let name = connection
        .get_property(
            false,
            window_id,
            AtomEnum::WM_NAME,
            AtomEnum::STRING,
            0,
            1024,
        )
        .ok()?
        .reply()
        .ok()?;
    if !name.value.is_empty() {
        return Some(name.value);
    }

    let net_wm_name = intern(connection, b"_NET_WM_NAME").ok()?;
    let utf8_string = intern(connection, b"UTF8_STRING").ok()?;
    let name = connection
        .get_property(false, window_id, net_wm_name, utf8_string, 0, 1024)
        .ok()?
        .reply()
        .ok()?;
    Some(name.value)
}

fn intern(connection: &RustConnection, name: &[u8]) -> anyhow::Result<u32> {
    Ok(connection.intern_atom(false, name)?.reply()?.atom)
}
