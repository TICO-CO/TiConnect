use std::sync::Mutex;
use std::sync::PoisonError;

use rodio::{MixerDeviceSink, Player, Source};
use thiserror::Error;

use crate::audio::percent_gain;

const MESSAGE_SOUND: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../assets/gotmail.mp3"
));

const INCOMING_CALL_SOUND: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../assets/incoming_call.mp3"
));

const OUTGOING_CALL_SOUND: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../assets/outgoing_call.ogg"
));

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CallTone {
    Ringing,
    Calling,
}

impl CallTone {
    fn sound(self) -> &'static [u8] {
        match self {
            Self::Ringing => INCOMING_CALL_SOUND,
            Self::Calling => OUTGOING_CALL_SOUND,
        }
    }
}

pub trait NotificationSound: Send + Sync {
    fn play(&self, volume: u32) -> Result<(), NotificationSoundError>;
}

pub trait CallToneSound: Send + Sync {
    fn play_loop(&self, tone: CallTone, volume: u32) -> Result<(), NotificationSoundError>;
    fn set_volume(&self, volume: u32);
    fn stop(&self);
}

#[derive(Debug, Default)]
pub struct NullNotificationSound;

impl NotificationSound for NullNotificationSound {
    fn play(&self, _volume: u32) -> Result<(), NotificationSoundError> {
        Ok(())
    }
}

#[derive(Debug, Default)]
pub struct NullCallToneSound;

impl CallToneSound for NullCallToneSound {
    fn play_loop(&self, _tone: CallTone, _volume: u32) -> Result<(), NotificationSoundError> {
        Ok(())
    }

    fn set_volume(&self, _volume: u32) {}

    fn stop(&self) {}
}

pub struct RodioNotificationSound {
    stream: Mutex<Option<MixerDeviceSink>>,
    player: Mutex<Option<Player>>,
}

impl Default for RodioNotificationSound {
    fn default() -> Self {
        Self::new()
    }
}

impl RodioNotificationSound {
    pub fn new() -> Self {
        Self {
            stream: Mutex::new(None),
            player: Mutex::new(None),
        }
    }

    fn ensure_player(&self) -> Result<(), NotificationSoundError> {
        if self.player.lock().is_ok_and(|player| player.is_some()) {
            return Ok(());
        }
        let stream = rodio::DeviceSinkBuilder::open_default_sink()
            .map_err(|error| NotificationSoundError::Device(error.to_string()))?;
        let player = rodio::Player::connect_new(stream.mixer());
        *self.stream.lock().unwrap_or_else(PoisonError::into_inner) = Some(stream);
        *self.player.lock().unwrap_or_else(PoisonError::into_inner) = Some(player);
        Ok(())
    }
}

impl NotificationSound for RodioNotificationSound {
    fn play(&self, volume: u32) -> Result<(), NotificationSoundError> {
        self.ensure_player()?;
        let player = self.player.lock().unwrap_or_else(PoisonError::into_inner);
        let Some(player) = player.as_ref() else {
            return Err(NotificationSoundError::Device(
                "no playback device".to_owned(),
            ));
        };
        player.stop();
        player.set_volume(percent_gain(volume));
        let decoder = rodio::Decoder::try_from(std::io::Cursor::new(MESSAGE_SOUND))
            .map_err(|error| NotificationSoundError::Decode(error.to_string()))?;
        player.append(decoder);
        player.play();
        Ok(())
    }
}

pub struct RodioCallToneSound {
    stream: Mutex<Option<MixerDeviceSink>>,
    player: Mutex<Option<Player>>,
}

impl Default for RodioCallToneSound {
    fn default() -> Self {
        Self::new()
    }
}

impl RodioCallToneSound {
    pub fn new() -> Self {
        Self {
            stream: Mutex::new(None),
            player: Mutex::new(None),
        }
    }

    fn ensure_player(&self) -> Result<(), NotificationSoundError> {
        if self.player.lock().is_ok_and(|player| player.is_some()) {
            return Ok(());
        }
        let stream = rodio::DeviceSinkBuilder::open_default_sink()
            .map_err(|error| NotificationSoundError::Device(error.to_string()))?;
        let player = rodio::Player::connect_new(stream.mixer());
        *self.stream.lock().unwrap_or_else(PoisonError::into_inner) = Some(stream);
        *self.player.lock().unwrap_or_else(PoisonError::into_inner) = Some(player);
        Ok(())
    }
}

impl CallToneSound for RodioCallToneSound {
    fn play_loop(&self, tone: CallTone, volume: u32) -> Result<(), NotificationSoundError> {
        self.ensure_player()?;
        let player = self.player.lock().unwrap_or_else(PoisonError::into_inner);
        let Some(player) = player.as_ref() else {
            return Err(NotificationSoundError::Device(
                "no playback device".to_owned(),
            ));
        };
        player.stop();
        player.set_volume(percent_gain(volume));
        let decoder = rodio::Decoder::try_from(std::io::Cursor::new(tone.sound()))
            .map_err(|error| NotificationSoundError::Decode(error.to_string()))?;
        player.append(decoder.repeat_infinite());
        player.play();
        Ok(())
    }

    fn set_volume(&self, volume: u32) {
        let player = self.player.lock().unwrap_or_else(PoisonError::into_inner);
        if let Some(player) = player.as_ref() {
            player.set_volume(percent_gain(volume));
        }
    }

    fn stop(&self) {
        let player = self.player.lock().unwrap_or_else(PoisonError::into_inner);
        if let Some(player) = player.as_ref() {
            player.stop();
        }
    }
}

#[derive(Debug, Error)]
pub enum NotificationSoundError {
    #[error("notification sound playback device is unavailable: {0}")]
    Device(String),

    #[error("notification sound decode failed: {0}")]
    Decode(String),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn embedded_message_sound_decodes() {
        let decoder =
            rodio::Decoder::try_from(std::io::Cursor::new(MESSAGE_SOUND)).expect("decode sound");
        assert!(decoder.sample_rate().get() > 0);
    }

    #[test]
    fn embedded_call_tones_decode() {
        for tone in [CallTone::Ringing, CallTone::Calling] {
            let decoder = rodio::Decoder::try_from(std::io::Cursor::new(tone.sound()))
                .expect("decode call tone");
            assert!(decoder.sample_rate().get() > 0);
            assert!(!tone.sound().is_empty());
        }
    }

    #[test]
    fn null_sound_is_a_noop() {
        NullNotificationSound.play(60).expect("null sound");
        NullCallToneSound
            .play_loop(CallTone::Ringing, 60)
            .expect("null call tone");
        NullCallToneSound.set_volume(60);
        NullCallToneSound.stop();
    }
}
