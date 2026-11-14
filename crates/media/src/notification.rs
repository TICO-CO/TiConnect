use std::sync::Mutex;
use std::sync::PoisonError;

use rodio::{MixerDeviceSink, Player};
use thiserror::Error;

const MESSAGE_SOUND: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../assets/gotmail.mp3"
));

pub trait NotificationSound: Send + Sync {
    fn play(&self) -> Result<(), NotificationSoundError>;
}

#[derive(Debug, Default)]
pub struct NullNotificationSound;

impl NotificationSound for NullNotificationSound {
    fn play(&self) -> Result<(), NotificationSoundError> {
        Ok(())
    }
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
    fn play(&self) -> Result<(), NotificationSoundError> {
        self.ensure_player()?;
        let player = self.player.lock().unwrap_or_else(PoisonError::into_inner);
        let Some(player) = player.as_ref() else {
            return Err(NotificationSoundError::Device(
                "no playback device".to_owned(),
            ));
        };
        player.stop();
        let decoder = rodio::Decoder::try_from(std::io::Cursor::new(MESSAGE_SOUND))
            .map_err(|error| NotificationSoundError::Decode(error.to_string()))?;
        player.append(decoder);
        player.play();
        Ok(())
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
    use rodio::Source;

    use super::*;

    #[test]
    fn embedded_message_sound_decodes() {
        let decoder =
            rodio::Decoder::try_from(std::io::Cursor::new(MESSAGE_SOUND)).expect("decode sound");
        assert!(decoder.sample_rate().get() > 0);
    }

    #[test]
    fn null_sound_is_a_noop() {
        NullNotificationSound.play().expect("null sound");
    }
}
