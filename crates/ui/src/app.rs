use std::cell::RefCell;
use std::rc::Rc;
use std::time::Duration;

use gtk4 as gtk;
use gtk4::prelude::*;
use tokio::sync::{broadcast, mpsc};
use webkit6::prelude::*;

use ticonnect_settings::Appearance;

use crate::activation::{self, ActivationEvent};
use crate::bridge::{self, security::Origins};
use crate::client;

fn window_title() -> String {
    window_title_from(std::env::var("TICONNECT_UI_TITLE").ok().as_deref())
}

fn window_title_from(label: Option<&str>) -> String {
    label
        .map(str::trim)
        .filter(|label| !label.is_empty())
        .unwrap_or("TiConnect")
        .to_owned()
}

fn apply_appearance(appearance: Appearance) {
    let Some(settings) = gtk::Settings::default() else {
        return;
    };
    match appearance {
        Appearance::System => {}
        Appearance::Light => settings.set_gtk_application_prefer_dark_theme(false),
        Appearance::Dark => settings.set_gtk_application_prefer_dark_theme(true),
    }
}

fn build_webview(url: &str, origins: &Origins) -> webkit6::WebView {
    let session = webkit6::NetworkSession::new_ephemeral();
    let settings = webkit6::Settings::new();
    let dev = origins.dev_mode();
    settings.set_enable_developer_extras(dev);
    settings.set_enable_write_console_messages_to_stdout(dev);
    settings.set_allow_file_access_from_file_urls(false);
    settings.set_allow_universal_access_from_file_urls(false);

    let webview = webkit6::WebView::builder()
        .network_session(&session)
        .build();
    webview.set_settings(&settings);
    webview.connect_permission_request(|_, request| {
        request.deny();
        true
    });
    webview.connect_create(|_, _| None);
    let origins = origins.clone();
    webview.connect_decide_policy(move |_, decision, decision_type| {
        if decision_type != webkit6::PolicyDecisionType::NavigationAction {
            return false;
        }
        let Some(navigation) = decision.downcast_ref::<webkit6::NavigationPolicyDecision>() else {
            return false;
        };
        let Some(action) = navigation.navigation_action() else {
            return false;
        };
        let uri = action
            .request()
            .and_then(|request| request.uri())
            .map(|uri| uri.to_string())
            .unwrap_or_default();
        if origins.allows_navigation(&uri) {
            false
        } else {
            decision.ignore();
            true
        }
    });
    webview.load_uri(url);
    webview
}

pub fn run() -> anyhow::Result<()> {
    let settings = ticonnect_settings::Settings::load().ok();
    let appearance = settings
        .as_ref()
        .and_then(ticonnect_settings::Settings::stored_appearance_color_scheme)
        .unwrap_or(Appearance::System);

    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()?;
    let _guard = runtime.enter();

    let (daemon_events, daemon_receiver) = mpsc::unbounded_channel();
    let link = client::spawn_link(daemon_events);
    let (activation_sender, _) = broadcast::channel(64);
    let bridge = runtime.block_on(bridge::start(
        link,
        daemon_receiver,
        activation_sender.subscribe(),
    ))?;
    let activation_receiver = activation_sender.subscribe();
    runtime.spawn(activation::serve(activation_sender));

    let app = gtk::Application::builder()
        .application_id("org.ticonnect.UI")
        .flags(gtk::gio::ApplicationFlags::NON_UNIQUE)
        .build();
    app.connect_startup(move |_| apply_appearance(appearance));

    let url = bridge::Bridge::dev_url().unwrap_or_else(|| bridge.url());
    let origins = Origins::new(bridge.port);
    let window: Rc<RefCell<Option<gtk::ApplicationWindow>>> = Rc::new(RefCell::new(None));

    {
        let window = Rc::clone(&window);
        let url = url.clone();
        let origins = origins.clone();
        app.connect_activate(move |app| {
            if let Some(window) = window.borrow().as_ref() {
                window.present();
                return;
            }
            let application_window = gtk::ApplicationWindow::builder()
                .application(app)
                .title(window_title())
                .default_width(1040)
                .default_height(720)
                .build();
            let webview = build_webview(&url, &origins);
            application_window.set_child(Some(&webview));
            application_window.present();
            *window.borrow_mut() = Some(application_window);
        });
    }

    {
        let app = app.clone();
        let window = Rc::clone(&window);
        let mut activation_receiver = activation_receiver;
        glib_timeout_add(Duration::from_millis(100), move || {
            loop {
                match activation_receiver.try_recv() {
                    Ok(ActivationEvent::Duplicate) => {
                        app.quit();
                        return gtk::glib::ControlFlow::Break;
                    }
                    Ok(_) => {
                        if let Some(window) = window.borrow().as_ref() {
                            window.present();
                        }
                    }
                    Err(broadcast::error::TryRecvError::Lagged(_)) => continue,
                    Err(broadcast::error::TryRecvError::Empty) => {
                        return gtk::glib::ControlFlow::Continue;
                    }
                    Err(broadcast::error::TryRecvError::Closed) => {
                        return gtk::glib::ControlFlow::Break;
                    }
                }
            }
        });
    }

    let _bridge = bridge;
    app.run();
    drop(_guard);
    Ok(())
}

fn glib_timeout_add(
    interval: Duration,
    callback: impl FnMut() -> gtk::glib::ControlFlow + 'static,
) -> gtk::glib::SourceId {
    gtk::glib::timeout_add_local(interval, callback)
}

#[cfg(test)]
mod tests {
    use super::window_title_from;

    #[test]
    fn window_title_defaults_and_accepts_lab_label() {
        assert_eq!(window_title_from(None), "TiConnect");
        assert_eq!(window_title_from(Some("   ")), "TiConnect");
        assert_eq!(
            window_title_from(Some("TiConnect · Lab Node A")),
            "TiConnect · Lab Node A"
        );
    }
}
