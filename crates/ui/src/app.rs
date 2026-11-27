use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use gtk4 as gtk;
use gtk4::prelude::*;
use tokio::sync::{broadcast, mpsc};
use webkit6::prelude::*;

use ticonnect_settings::Appearance;

use crate::activation::{self, ActivationEvent};
use crate::bridge::{self, security::Origins};
use crate::client;
use crate::timing;

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

fn application_id_from(dev_build: bool) -> &'static str {
    if dev_build {
        "org.ticonnect.UI.Dev"
    } else {
        "org.ticonnect.UI"
    }
}

fn application_id() -> &'static str {
    application_id_from(std::env::var_os("TICONNECT_DEV_BUILD").is_some())
}

fn audio_only_media_allowed(is_audio: bool, is_video: bool) -> bool {
    is_audio && !is_video
}

fn youtube_frame(uri: &str) -> bool {
    [
        "https://www.youtube-nocookie.com/",
        "https://www.youtube.com/",
        "https://consent.youtube.com/",
        "https://accounts.google.com/",
    ]
    .iter()
    .any(|prefix| uri.starts_with(prefix))
}

fn write_bridge_file(path: &std::path::Path, url: &str) -> std::io::Result<()> {
    use std::io::Write;

    let mut options = std::fs::OpenOptions::new();
    options.write(true).create(true).truncate(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file = options.open(path)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        file.set_permissions(std::fs::Permissions::from_mode(0o600))?;
    }
    file.write_all(url.as_bytes())?;
    file.write_all(b"\n")
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
    if std::env::var_os("TICONNECT_DISABLE_GPU").is_some_and(|value| !value.is_empty()) {
        settings.set_hardware_acceleration_policy(webkit6::HardwareAccelerationPolicy::Never);
    }

    let webview = webkit6::WebView::builder()
        .network_session(&session)
        .build();
    webview.set_settings(&settings);
    webview.connect_permission_request(|_, request| {
        if let Some(media) = request.downcast_ref::<webkit6::UserMediaPermissionRequest>()
            && audio_only_media_allowed(media.is_for_audio_device(), media.is_for_video_device())
        {
            media.allow();
            return true;
        }
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
        let Some(request) = action.request() else {
            return false;
        };
        let Some(uri) = request.uri() else {
            return false;
        };
        let uri = uri.to_string();
        if origins.allows_navigation(&uri) || youtube_frame(&uri) {
            false
        } else {
            if origins.dev_mode() {
                eprintln!("blocked webview navigation: {uri}");
            }
            decision.ignore();
            true
        }
    });
    webview.connect_load_changed(|_, event| {
        if event == webkit6::LoadEvent::Finished {
            crate::timing::mark("webview load finished");
        }
    });
    webview.load_uri(url);
    webview
}

pub fn run() -> anyhow::Result<()> {
    timing::mark("app run start");
    let settings = ticonnect_settings::Settings::load().ok();
    let appearance = settings
        .as_ref()
        .and_then(ticonnect_settings::Settings::stored_appearance_color_scheme)
        .unwrap_or(Appearance::System);
    timing::mark("settings loaded");

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
    timing::mark("bridge ready");
    let activation_receiver = activation_sender.subscribe();
    let ui_active = Arc::new(AtomicBool::new(false));
    runtime.spawn(activation::serve(activation_sender, ui_active.clone()));

    let app = gtk::Application::builder()
        .application_id(application_id())
        .flags(gtk::gio::ApplicationFlags::NON_UNIQUE)
        .build();
    app.connect_startup(move |_| apply_appearance(appearance));
    timing::mark("gtk application built");

    let url = bridge::Bridge::dev_url()
        .map(|dev| format!("{dev}?bridge=127.0.0.1:{}", bridge.port))
        .unwrap_or_else(|| bridge.url());
    if let Some(path) = std::env::var_os("TICONNECT_UI_BRIDGE_FILE")
        && let Err(error) = write_bridge_file(std::path::Path::new(&path), &url)
    {
        eprintln!("failed to write UI bridge file: {error}");
    }
    let origins = Origins::new(bridge.port);
    let window: Rc<RefCell<Option<gtk::ApplicationWindow>>> = Rc::new(RefCell::new(None));

    {
        let window = Rc::clone(&window);
        let url = url.clone();
        let origins = origins.clone();
        let ui_active = Arc::clone(&ui_active);
        app.connect_activate(move |app| {
            if let Some(window) = window.borrow().as_ref() {
                window.present();
                return;
            }
            timing::mark("activate");
            let application_window = gtk::ApplicationWindow::builder()
                .application(app)
                .title(window_title())
                .default_width(1040)
                .default_height(720)
                .build();
            *window.borrow_mut() = Some(application_window.clone());
            let active = Arc::clone(&ui_active);
            application_window.connect_is_active_notify(move |window| {
                active.store(window.is_active(), Ordering::Relaxed);
            });
            application_window.present();
            ui_active.store(application_window.is_active(), Ordering::Relaxed);
            timing::mark("window presented");
            let url = url.clone();
            let origins = origins.clone();
            gtk::glib::idle_add_local_once(move || {
                timing::mark("webview build start");
                let webview = build_webview(&url, &origins);
                timing::mark("webview built");
                application_window.set_child(Some(&webview));
            });
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
    app.run_with_args::<&str>(&[]);
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
    use super::{application_id_from, audio_only_media_allowed, window_title_from, youtube_frame};

    #[test]
    fn media_permissions_allow_only_audio_capture() {
        assert!(audio_only_media_allowed(true, false));
        assert!(!audio_only_media_allowed(false, false));
        assert!(!audio_only_media_allowed(true, true));
        assert!(!audio_only_media_allowed(false, true));
    }

    #[test]
    fn youtube_frame_allows_consent_and_account_navigation() {
        assert!(youtube_frame("https://www.youtube-nocookie.com/embed/x"));
        assert!(youtube_frame("https://www.youtube.com/embed/x"));
        assert!(youtube_frame("https://consent.youtube.com/m?continue=x"));
        assert!(youtube_frame("https://accounts.google.com/ServiceLogin"));
        assert!(!youtube_frame("https://example.com/"));
    }

    #[test]
    fn window_title_defaults_and_accepts_lab_label() {
        assert_eq!(window_title_from(None), "TiConnect");
        assert_eq!(window_title_from(Some("   ")), "TiConnect");
        assert_eq!(
            window_title_from(Some("TiConnect · Lab Node A")),
            "TiConnect · Lab Node A"
        );
    }

    #[test]
    fn dev_build_uses_a_separate_application_id() {
        assert_eq!(application_id_from(false), "org.ticonnect.UI");
        assert_eq!(application_id_from(true), "org.ticonnect.UI.Dev");
    }
}
