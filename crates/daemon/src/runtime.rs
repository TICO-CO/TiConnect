use std::sync::Arc;
use std::time::{Duration, Instant};

use anyhow::{Context, Result};
use ticonnect_core::call::{CallDirection, CallState};
use ticonnect_core::ipc;
use ticonnect_media::notification::{
    CallTone, CallToneSound, NotificationSound, RodioCallToneSound, RodioNotificationSound,
};
use ticonnect_net::discovery::Discovery;
use tokio::signal::unix::{SignalKind, signal};
use tokio::sync::watch;
use zbus::object_server::SignalEmitter;

use crate::attachments;
use crate::auto_trust;
use crate::calls::{self, CallEvent};
use crate::config_watch;
use crate::discovery_service;
use crate::messaging::{self, MessagingEvent};
use crate::profile;
use crate::reconnect_service;
use crate::remote;
use crate::service::{DaemonService, DaemonState};
use crate::tray;
use crate::watch_party;

const MESSAGE_SOUND_COOLDOWN: Duration = Duration::from_secs(5);
const CALL_TONE_SETTINGS_INTERVAL: Duration = Duration::from_millis(100);

fn desired_call_tone(
    event: &CallEvent,
    call_sounds_enabled: bool,
    dnd_enabled: bool,
) -> Option<CallTone> {
    if dnd_enabled || !call_sounds_enabled {
        return None;
    }
    let call = match event {
        CallEvent::IncomingCall(call) | CallEvent::StateChanged(call) => call,
        CallEvent::Ended { .. } => return None,
    };
    match (call.direction, call.state) {
        (CallDirection::Incoming, CallState::Ringing) => Some(CallTone::Ringing),
        (CallDirection::Outgoing, CallState::Connecting | CallState::Ringing) => {
            Some(CallTone::Calling)
        }
        _ => None,
    }
}

fn sync_call_tone(
    sound: &dyn CallToneSound,
    current: &mut Option<CallTone>,
    event: &CallEvent,
    call_sounds_enabled: bool,
    dnd_enabled: bool,
    output_volume: u32,
) {
    let desired = desired_call_tone(event, call_sounds_enabled, dnd_enabled);
    if *current == desired {
        if desired.is_some() {
            sound.set_volume(output_volume);
        }
        return;
    }
    if let Some(tone) = desired {
        if let Err(error) = sound.play_loop(tone, output_volume) {
            tracing::warn!(%error, "failed to play call tone");
        }
    } else {
        sound.stop();
    }
    *current = desired;
}

struct MessageSoundCooldown {
    last_played: Option<Instant>,
}

impl MessageSoundCooldown {
    fn new() -> Self {
        Self { last_played: None }
    }

    fn try_play(&mut self, now: Instant) -> bool {
        if self
            .last_played
            .is_some_and(|last| now.duration_since(last) < MESSAGE_SOUND_COOLDOWN)
        {
            return false;
        }
        self.last_played = Some(now);
        true
    }
}

async fn ui_is_active(connection: &zbus::Connection) -> Result<bool> {
    let dbus = zbus::fdo::DBusProxy::new(connection)
        .await
        .context("create D-Bus proxy")?;
    if !dbus
        .name_has_owner(ipc::UI_BUS_NAME.try_into()?)
        .await
        .context("query UI bus owner")?
    {
        return Ok(false);
    }
    let proxy = zbus::Proxy::new(
        connection,
        ipc::UI_BUS_NAME,
        ipc::UI_PATH,
        ipc::UI_INTERFACE,
    )
    .await
    .context("create UI proxy")?;
    proxy
        .call("IsActive", &())
        .await
        .context("query UI active state")
}

#[derive(Clone)]
pub struct Shutdown {
    requested: watch::Sender<bool>,
}

impl Shutdown {
    pub fn new() -> Self {
        let (requested, _receiver) = watch::channel(false);
        Self { requested }
    }

    pub fn request(&self) {
        self.requested.send_replace(true);
    }

    pub fn is_requested(&self) -> bool {
        *self.requested.borrow()
    }

    pub async fn wait(&self) {
        let mut receiver = self.requested.subscribe();
        if *receiver.borrow() {
            return;
        }
        while receiver.changed().await.is_ok() {
            if *receiver.borrow() {
                return;
            }
        }
    }
}

impl Default for Shutdown {
    fn default() -> Self {
        Self::new()
    }
}

pub async fn run(state: Arc<DaemonState>) -> Result<()> {
    let service = DaemonService::new(state.clone());

    let connection = ipc::connection_builder()
        .name(ipc::DAEMON_BUS_NAME)
        .allow_name_replacements(false)
        .replace_existing_names(false)
        .serve_at(ipc::DAEMON_PATH, service)
        .build()
        .await
        .context("acquire daemon bus name")?;

    tracing::info!(bus = ipc::DAEMON_BUS_NAME, "daemon started");
    emit_started(&connection).await?;

    let watcher =
        config_watch::spawn(connection.clone(), state.clone()).context("start config watcher")?;
    if let Err(error) = state.discovery().start() {
        tracing::warn!(%error, "failed to start discovery");
    }
    let discovery_task = discovery_service::spawn(connection.clone(), state.clone());
    let auto_trust_task = auto_trust::spawn(connection.clone(), state.auto_trust().clone());
    let profile_task = profile::spawn(
        connection.clone(),
        state.discovery().clone(),
        state.profile().clone(),
        state.transport().clone(),
    );
    let (transfer_sender, transfer_receiver) = tokio::sync::mpsc::unbounded_channel();
    state.messaging().set_transfer_sender(transfer_sender);
    let (call_sender, call_receiver) = tokio::sync::mpsc::unbounded_channel();
    let (audio_sender, audio_receiver) = tokio::sync::mpsc::unbounded_channel();
    state
        .messaging()
        .set_call_channels(call_sender, audio_sender);
    let (remote_sender, remote_receiver) = tokio::sync::mpsc::unbounded_channel();
    state.messaging().set_remote_sender(remote_sender);
    let remote_task = remote::spawn(state.remote().as_ref().clone(), remote_receiver);
    let (watch_party_sender, watch_party_receiver) = tokio::sync::mpsc::unbounded_channel();
    state.messaging().set_watch_party_sender(watch_party_sender);
    let watch_party_task = watch_party::spawn(state.watch_party().clone(), watch_party_receiver);
    let mut watch_party_events = state.watch_party().subscribe();
    let watch_party_connection = connection.clone();
    let watch_party_event_task = tokio::spawn(async move {
        loop {
            match watch_party_events.recv().await {
                Ok(event) => {
                    if let Err(error) =
                        crate::service::emit_watch_party_event(&watch_party_connection, event).await
                    {
                        tracing::warn!(%error, "failed to emit watch party signal");
                    }
                }
                Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => {}
                Err(tokio::sync::broadcast::error::RecvError::Closed) => break,
            }
        }
    });
    let mut remote_events = state.remote().subscribe();
    let remote_connection = connection.clone();
    let remote_event_task = tokio::spawn(async move {
        loop {
            match remote_events.recv().await {
                Ok(event) => {
                    if let Err(error) =
                        crate::service::emit_remote_event(&remote_connection, event).await
                    {
                        tracing::warn!(%error, "failed to emit remote signal");
                    }
                }
                Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => {}
                Err(tokio::sync::broadcast::error::RecvError::Closed) => break,
            }
        }
    });
    let calls_task = calls::spawn(
        state.calls().as_ref().clone(),
        call_receiver,
        audio_receiver,
    );
    let mut call_events = state.calls().subscribe();
    let call_connection = connection.clone();
    let call_event_task = tokio::spawn(async move {
        loop {
            match call_events.recv().await {
                Ok(event) => {
                    if let Err(error) =
                        crate::service::emit_call_event(&call_connection, event).await
                    {
                        tracing::warn!(%error, "failed to emit call signal");
                    }
                }
                Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => {}
                Err(tokio::sync::broadcast::error::RecvError::Closed) => break,
            }
        }
    });
    let attachments_task = attachments::spawn(
        state.attachments().as_ref().clone(),
        state.messaging().subscribe(),
        transfer_receiver,
    );
    let mut transfer_events = state.attachments().subscribe();
    let transfer_connection = connection.clone();
    let transfer_event_task = tokio::spawn(async move {
        loop {
            match transfer_events.recv().await {
                Ok(event) => {
                    if let Err(error) =
                        crate::service::emit_transfer_event(&transfer_connection, event).await
                    {
                        tracing::warn!(%error, "failed to emit transfer signal");
                    }
                }
                Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => {}
                Err(tokio::sync::broadcast::error::RecvError::Closed) => break,
            }
        }
    });
    let messaging_task = messaging::spawn(connection.clone(), state.messaging().as_ref().clone());
    let message_sound = RodioNotificationSound::new();
    let message_sound_connection = connection.clone();
    let message_sound_state = Arc::clone(&state);
    let mut message_events = state.messaging().subscribe();
    let message_sound_task = tokio::spawn(async move {
        let mut cooldown = MessageSoundCooldown::new();
        loop {
            match message_events.recv().await {
                Ok(MessagingEvent::MessageReceived { notify: true, .. }) => {
                    let settings = message_sound_state.settings();
                    if !settings.message_sounds_enabled() {
                        continue;
                    }
                    let output_volume = settings.audio_output_volume();
                    match ui_is_active(&message_sound_connection).await {
                        Ok(true) => continue,
                        Ok(false) => {}
                        Err(error) => {
                            tracing::warn!(
                                %error,
                                "failed to check UI focus; skipping message sound"
                            );
                            continue;
                        }
                    }
                    if !cooldown.try_play(Instant::now()) {
                        continue;
                    }
                    if let Err(error) = message_sound.play(output_volume) {
                        tracing::warn!(%error, "failed to play message sound");
                    }
                }
                Ok(_) => {}
                Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => {}
                Err(tokio::sync::broadcast::error::RecvError::Closed) => break,
            }
        }
    });
    let call_tone = Arc::new(RodioCallToneSound::new());
    state.calls().set_call_tone(call_tone.clone());
    let call_tone_player = Arc::clone(&call_tone);
    let call_tone_state = Arc::clone(&state);
    let mut call_tone_events = state.calls().subscribe();
    let call_tone_task = tokio::spawn(async move {
        let mut current = None;
        let mut last_event = None;
        let mut settings_ticker = tokio::time::interval(CALL_TONE_SETTINGS_INTERVAL);
        settings_ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
        loop {
            tokio::select! {
                event = call_tone_events.recv() => {
                    match event {
                        Ok(event) => {
                            let settings = call_tone_state.settings();
                            sync_call_tone(
                                call_tone_player.as_ref(),
                                &mut current,
                                &event,
                                settings.call_sounds_enabled(),
                                settings.dnd_enabled(),
                                settings.audio_output_volume(),
                            );
                            last_event = Some(event);
                        }
                        Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => {}
                        Err(tokio::sync::broadcast::error::RecvError::Closed) => break,
                    }
                }
                _ = settings_ticker.tick() => {
                    let Some(event) = last_event.as_ref() else {
                        continue;
                    };
                    let settings = call_tone_state.settings();
                    sync_call_tone(
                        call_tone_player.as_ref(),
                        &mut current,
                        event,
                        settings.call_sounds_enabled(),
                        settings.dnd_enabled(),
                        settings.audio_output_volume(),
                    );
                }
            }
        }
    });
    let tray_task = tray::spawn(
        connection.clone(),
        state.messaging().as_ref().clone(),
        state.shutdown().clone(),
    );
    let reconnect_task = reconnect_service::spawn(
        state.transport().clone(),
        state.discovery().clone(),
        state.trust().clone(),
    );

    wait_for_shutdown(&connection, state.shutdown()).await?;
    watcher.abort();
    state.discovery().stop_and_wait().await;
    discovery_task.abort();
    auto_trust_task.abort();
    profile_task.abort();
    messaging_task.abort();
    message_sound_task.abort();
    call_tone.stop();
    call_tone_task.abort();
    attachments_task.abort();
    transfer_event_task.abort();
    calls_task.abort();
    call_event_task.abort();
    remote_task.abort();
    watch_party_task.abort();
    watch_party_event_task.abort();
    remote_event_task.abort();
    tray_task.abort();
    reconnect_task.abort();
    state.transport().shutdown().await;

    if let Err(error) = emit_stopping(&connection).await {
        tracing::warn!(%error, "failed to emit DaemonStopping");
    }
    if let Err(error) = connection.release_name(ipc::DAEMON_BUS_NAME).await {
        tracing::warn!(%error, "failed to release daemon bus name");
    }
    tracing::info!("daemon stopped");
    Ok(())
}

async fn emit_started(connection: &zbus::Connection) -> Result<()> {
    let emitter =
        SignalEmitter::new(connection, ipc::DAEMON_PATH).context("create daemon signal emitter")?;
    DaemonService::daemon_started(emitter)
        .await
        .context("emit DaemonStarted")
}

async fn emit_stopping(connection: &zbus::Connection) -> Result<()> {
    let emitter =
        SignalEmitter::new(connection, ipc::DAEMON_PATH).context("create daemon signal emitter")?;
    DaemonService::daemon_stopping(emitter)
        .await
        .context("emit DaemonStopping")
}

async fn wait_for_shutdown(connection: &zbus::Connection, shutdown: &Shutdown) -> Result<()> {
    let mut terminate = signal(SignalKind::terminate()).context("install SIGTERM handler")?;
    let mut interrupt = signal(SignalKind::interrupt()).context("install SIGINT handler")?;

    tokio::select! {
        _ = shutdown.wait() => tracing::info!("shutdown requested"),
        _ = terminate.recv() => tracing::info!("received SIGTERM"),
        _ = interrupt.recv() => tracing::info!("received SIGINT"),
        _ = connection.closed() => tracing::warn!("daemon bus connection closed"),
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use std::sync::Mutex;

    use ticonnect_media::notification::NotificationSoundError;

    use super::*;

    #[derive(Default)]
    struct FakeCallTone {
        played: Mutex<Vec<(CallTone, u32)>>,
        volume_updates: Mutex<Vec<u32>>,
        stops: Mutex<u32>,
    }

    impl CallToneSound for FakeCallTone {
        fn play_loop(&self, tone: CallTone, volume: u32) -> Result<(), NotificationSoundError> {
            self.played
                .lock()
                .expect("played lock")
                .push((tone, volume));
            Ok(())
        }

        fn set_volume(&self, volume: u32) {
            self.volume_updates
                .lock()
                .expect("volume lock")
                .push(volume);
        }

        fn stop(&self) {
            *self.stops.lock().expect("stops lock") += 1;
        }
    }

    #[tokio::test]
    async fn shutdown_wait_wakes_after_request() {
        let shutdown = Shutdown::new();
        let waiter = tokio::spawn({
            let shutdown = shutdown.clone();
            async move { shutdown.wait().await }
        });

        shutdown.request();
        waiter.await.expect("shutdown wait");
        assert!(shutdown.is_requested());
    }

    #[tokio::test]
    async fn shutdown_wait_returns_when_already_requested() {
        let shutdown = Shutdown::new();
        shutdown.request();
        shutdown.wait().await;
    }

    fn call_info(direction: CallDirection, state: CallState) -> crate::calls::state::CallInfo {
        crate::calls::state::CallInfo {
            id: ticonnect_core::call::CallId::generate(),
            user_id: "user".to_owned(),
            direction,
            state,
            created_at: 0,
            connected_at: None,
            input_muted: false,
            output_muted: false,
        }
    }

    #[test]
    fn incoming_ringing_plays_ringtone() {
        let event = CallEvent::IncomingCall(call_info(CallDirection::Incoming, CallState::Ringing));
        assert_eq!(
            desired_call_tone(&event, true, false),
            Some(CallTone::Ringing)
        );
    }

    #[test]
    fn outgoing_connecting_and_ringing_play_calling_tone() {
        for state in [CallState::Connecting, CallState::Ringing] {
            let event = CallEvent::StateChanged(call_info(CallDirection::Outgoing, state));
            assert_eq!(
                desired_call_tone(&event, true, false),
                Some(CallTone::Calling)
            );
        }
    }

    #[test]
    fn connected_and_ended_calls_stop_tone() {
        let connected =
            CallEvent::StateChanged(call_info(CallDirection::Outgoing, CallState::InCall));
        assert_eq!(desired_call_tone(&connected, true, false), None);
        let ended = CallEvent::Ended {
            call_id: ticonnect_core::call::CallId::generate(),
            user_id: "user".to_owned(),
            reason: "hangup".to_owned(),
        };
        assert_eq!(desired_call_tone(&ended, true, false), None);
    }

    #[test]
    fn call_tone_respects_sound_setting_and_dnd() {
        let event = CallEvent::IncomingCall(call_info(CallDirection::Incoming, CallState::Ringing));
        assert_eq!(desired_call_tone(&event, false, false), None);
        assert_eq!(desired_call_tone(&event, true, true), None);
    }

    #[test]
    fn sync_call_tone_updates_volume_without_restarting() {
        let sound = FakeCallTone::default();
        let mut current = None;
        let first = CallEvent::IncomingCall(call_info(CallDirection::Incoming, CallState::Ringing));
        let same = CallEvent::StateChanged(call_info(CallDirection::Incoming, CallState::Ringing));
        let connected =
            CallEvent::StateChanged(call_info(CallDirection::Incoming, CallState::InCall));
        sync_call_tone(&sound, &mut current, &first, true, false, 80);
        assert_eq!(current, Some(CallTone::Ringing));
        sync_call_tone(&sound, &mut current, &same, true, false, 35);
        assert_eq!(current, Some(CallTone::Ringing));
        sync_call_tone(&sound, &mut current, &connected, true, false, 35);
        assert_eq!(current, None);
        assert_eq!(
            *sound.played.lock().expect("played lock"),
            vec![(CallTone::Ringing, 80)]
        );
        assert_eq!(*sound.volume_updates.lock().expect("volume lock"), vec![35]);
        assert_eq!(*sound.stops.lock().expect("stops lock"), 1);
    }

    #[test]
    fn message_sound_cooldown_allows_first_play() {
        let mut cooldown = MessageSoundCooldown::new();
        assert!(cooldown.try_play(Instant::now()));
    }

    #[test]
    fn message_sound_cooldown_skips_quick_repeat() {
        let mut cooldown = MessageSoundCooldown::new();
        let first = Instant::now();
        assert!(cooldown.try_play(first));
        assert!(!cooldown.try_play(first + Duration::from_secs(1)));
    }

    #[test]
    fn message_sound_cooldown_allows_after_interval() {
        let mut cooldown = MessageSoundCooldown::new();
        let first = Instant::now();
        assert!(cooldown.try_play(first));
        assert!(cooldown.try_play(first + MESSAGE_SOUND_COOLDOWN));
    }
}
