use std::path::Path;
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use thiserror::Error;

pub trait RingtoneControl: Send + Sync {
    fn start(&self, path: Option<&Path>) -> Result<(), RingtoneError>;
    fn stop(&self);
    fn duck(&self, ducked: bool);
    fn is_playing(&self) -> bool;
}

#[derive(Debug, Default)]
pub struct NullRingtone {
    playing: AtomicBool,
    ducked: AtomicBool,
}

impl RingtoneControl for NullRingtone {
    fn start(&self, _path: Option<&Path>) -> Result<(), RingtoneError> {
        self.playing.store(true, Ordering::Relaxed);
        Ok(())
    }

    fn stop(&self) {
        self.playing.store(false, Ordering::Relaxed);
    }

    fn duck(&self, ducked: bool) {
        self.ducked.store(ducked, Ordering::Relaxed);
    }

    fn is_playing(&self) -> bool {
        self.playing.load(Ordering::Relaxed)
    }
}

pub struct RodioRingtone {
    stream: Mutex<Option<rodio::MixerDeviceSink>>,
    player: Mutex<Option<rodio::Player>>,
    playing: AtomicBool,
    ducked: AtomicBool,
}

impl Default for RodioRingtone {
    fn default() -> Self {
        Self::new()
    }
}

impl RodioRingtone {
    pub fn new() -> Self {
        Self {
            stream: Mutex::new(None),
            player: Mutex::new(None),
            playing: AtomicBool::new(false),
            ducked: AtomicBool::new(false),
        }
    }

    fn ensure_player(&self) -> Result<(), RingtoneError> {
        if self.player.lock().is_ok_and(|player| player.is_some()) {
            return Ok(());
        }
        let stream = rodio::DeviceSinkBuilder::open_default_sink()
            .map_err(|error| RingtoneError::Device(error.to_string()))?;
        let player = rodio::Player::connect_new(stream.mixer());
        *self
            .stream
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = Some(stream);
        *self
            .player
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = Some(player);
        Ok(())
    }
}

impl RingtoneControl for RodioRingtone {
    fn start(&self, path: Option<&Path>) -> Result<(), RingtoneError> {
        self.ensure_player()?;
        let player = self
            .player
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let Some(player) = player.as_ref() else {
            return Err(RingtoneError::Device("no playback device".to_owned()));
        };
        player.stop();
        player.set_volume(if self.ducked.load(Ordering::Relaxed) {
            0.2
        } else {
            0.8
        });
        let mut played = false;
        if let Some(path) = path
            && let Ok(file) = std::fs::File::open(path)
            && let Ok(decoder) = rodio::Decoder::try_from(std::io::BufReader::new(file))
        {
            use rodio::Source;
            player.append(decoder.repeat_infinite());
            played = true;
        }
        if !played {
            use rodio::Source;
            let tone = rodio::source::SineWave::new(740.0)
                .amplify(0.15)
                .take_duration(Duration::from_millis(500))
                .repeat_infinite();
            player.append(tone);
        }
        player.play();
        self.playing.store(true, Ordering::Relaxed);
        Ok(())
    }

    fn stop(&self) {
        if let Ok(player) = self.player.lock()
            && let Some(player) = player.as_ref()
        {
            player.stop();
        }
        self.playing.store(false, Ordering::Relaxed);
    }

    fn duck(&self, ducked: bool) {
        self.ducked.store(ducked, Ordering::Relaxed);
        if let Ok(player) = self.player.lock()
            && let Some(player) = player.as_ref()
        {
            player.set_volume(if ducked { 0.2 } else { 0.8 });
        }
    }

    fn is_playing(&self) -> bool {
        self.playing.load(Ordering::Relaxed)
    }
}

#[derive(Debug, Error)]
pub enum RingtoneError {
    #[error("ringtone playback device is unavailable: {0}")]
    Device(String),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn null_ringtone_tracks_state() {
        let ringtone = NullRingtone::default();
        assert!(!ringtone.is_playing());
        ringtone.start(None).expect("start");
        assert!(ringtone.is_playing());
        ringtone.duck(true);
        ringtone.stop();
        assert!(!ringtone.is_playing());
    }
}
