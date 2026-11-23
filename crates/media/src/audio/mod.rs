pub mod capture;
pub mod device;
pub mod playback;
pub mod resample;

use std::sync::atomic::{AtomicU16, AtomicU64, Ordering};
use std::sync::{Arc, Mutex, PoisonError};
use std::time::{SystemTime, UNIX_EPOCH};

pub use capture::{CaptureHandle, CaptureSource, start_capture};
pub use device::{AudioDevice, AudioDevices, DeviceError};
pub use playback::{PlaybackHandle, PlaybackSink, start_playback};
pub use resample::Resampler;

pub const OPUS_SAMPLE_RATE: u32 = 48_000;
pub const OPUS_CHANNELS: u8 = 1;
pub const OPUS_FRAME_MS: u32 = 20;
pub const OPUS_FRAME_SAMPLES: usize = (OPUS_SAMPLE_RATE as usize / 1000) * OPUS_FRAME_MS as usize;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct AudioMetrics {
    pub frames_captured: u64,
    pub frames_played: u64,
    pub frames_encoded: u64,
    pub frames_decoded: u64,
    pub packets_lost: u64,
    pub buffer_depth: u64,
    pub jitter_ms: u64,
}

#[derive(Debug, Default)]
pub struct AudioMetricsHandle {
    frames_captured: AtomicU64,
    frames_played: AtomicU64,
    frames_encoded: AtomicU64,
    frames_decoded: AtomicU64,
    packets_lost: AtomicU64,
    buffer_depth: AtomicU64,
    jitter_ms: AtomicU64,
}

impl AudioMetricsHandle {
    pub fn snapshot(&self) -> AudioMetrics {
        AudioMetrics {
            frames_captured: self.frames_captured.load(Ordering::Relaxed),
            frames_played: self.frames_played.load(Ordering::Relaxed),
            frames_encoded: self.frames_encoded.load(Ordering::Relaxed),
            frames_decoded: self.frames_decoded.load(Ordering::Relaxed),
            packets_lost: self.packets_lost.load(Ordering::Relaxed),
            buffer_depth: self.buffer_depth.load(Ordering::Relaxed),
            jitter_ms: self.jitter_ms.load(Ordering::Relaxed),
        }
    }

    pub fn note_captured(&self) {
        self.frames_captured.fetch_add(1, Ordering::Relaxed);
    }

    pub fn note_played(&self) {
        self.frames_played.fetch_add(1, Ordering::Relaxed);
    }

    pub fn note_encoded(&self) {
        self.frames_encoded.fetch_add(1, Ordering::Relaxed);
    }

    pub fn note_decoded(&self) {
        self.frames_decoded.fetch_add(1, Ordering::Relaxed);
    }

    pub fn note_lost(&self) {
        self.packets_lost.fetch_add(1, Ordering::Relaxed);
    }

    pub fn set_buffer_depth(&self, depth: u64) {
        self.buffer_depth.store(depth, Ordering::Relaxed);
    }

    pub fn set_jitter_ms(&self, jitter: u64) {
        self.jitter_ms.store(jitter, Ordering::Relaxed);
    }
}

const LEVEL_FLOOR_DB: f32 = -60.0;
const LEVEL_ATTACK_PERCENT: u32 = 55;
const LEVEL_RELEASE_PERCENT: u32 = 15;
const LEVEL_HOLD_MS: u64 = 120;
const LEVEL_FADE_MS: u64 = 480;
pub const LEVEL_MAX_PERMILLE: u16 = 1_000;

#[derive(Debug, Default)]
pub struct LevelMeter {
    level: AtomicU16,
    updated_ms: AtomicU64,
}

impl LevelMeter {
    pub fn observe(&self, samples: &[f32]) {
        self.observe_at(samples, now_ms());
    }

    pub fn silence(&self) {
        self.level.store(0, Ordering::Relaxed);
        self.updated_ms.store(now_ms(), Ordering::Relaxed);
    }

    pub fn permille(&self) -> u16 {
        self.permille_at(now_ms())
    }

    fn observe_at(&self, samples: &[f32], now: u64) {
        let target = level_from_rms(rms(samples));
        let current = self.permille_at(now);
        self.level
            .store(smoothed(current, target), Ordering::Relaxed);
        self.updated_ms.store(now, Ordering::Relaxed);
    }

    fn permille_at(&self, now: u64) -> u16 {
        let updated = self.updated_ms.load(Ordering::Relaxed);
        if updated == 0 {
            return 0;
        }
        decayed(
            self.level.load(Ordering::Relaxed),
            now.saturating_sub(updated),
        )
    }
}

pub fn level_from_rms(rms: f32) -> u16 {
    if !rms.is_finite() || rms <= 0.0 {
        return 0;
    }
    let dbfs = 20.0 * rms.log10();
    let normalized = ((dbfs - LEVEL_FLOOR_DB) / -LEVEL_FLOOR_DB).clamp(0.0, 1.0);
    u16::try_from((normalized * f32::from(LEVEL_MAX_PERMILLE)).round() as u32)
        .unwrap_or(LEVEL_MAX_PERMILLE)
}

fn rms(samples: &[f32]) -> f32 {
    if samples.is_empty() {
        return 0.0;
    }
    let energy = samples.iter().map(|sample| sample * sample).sum::<f32>();
    (energy / samples.len() as f32).sqrt()
}

fn smoothed(current: u16, target: u16) -> u16 {
    let weight = if target >= current {
        LEVEL_ATTACK_PERCENT
    } else {
        LEVEL_RELEASE_PERCENT
    };
    let current = u32::from(current);
    let target = u32::from(target);
    let next = (current * (100 - weight) + target * weight) / 100;
    u16::try_from(next).unwrap_or(u16::MAX)
}

fn decayed(level: u16, elapsed_ms: u64) -> u16 {
    if elapsed_ms <= LEVEL_HOLD_MS {
        return level;
    }
    let remaining = LEVEL_FADE_MS.saturating_sub(elapsed_ms - LEVEL_HOLD_MS);
    let faded = f32::from(level) * (remaining as f32 / LEVEL_FADE_MS as f32);
    u16::try_from(faded.round() as u32).unwrap_or(0)
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |elapsed| {
            u64::try_from(elapsed.as_millis()).unwrap_or(u64::MAX)
        })
}

#[derive(Clone, Default)]
pub struct SharedBuffer(Arc<Mutex<Vec<f32>>>);

impl SharedBuffer {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn push(&self, samples: &[f32]) {
        let mut buffer = self.0.lock().unwrap_or_else(PoisonError::into_inner);
        if buffer.len() + samples.len() > OPUS_SAMPLE_RATE as usize * 2 {
            let capacity = OPUS_SAMPLE_RATE as usize * 2;
            let overflow = buffer.len() + samples.len() - capacity;
            let remove = overflow.min(buffer.len());
            buffer.drain(..remove);
        }
        buffer.extend_from_slice(samples);
    }

    pub fn drain(&self, count: usize) -> Vec<f32> {
        let mut buffer = self.0.lock().unwrap_or_else(PoisonError::into_inner);
        let take = count.min(buffer.len());
        buffer.drain(..take).collect()
    }

    pub fn clear(&self) {
        self.0
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clear();
    }

    pub fn depth(&self) -> usize {
        self.0.lock().unwrap_or_else(PoisonError::into_inner).len()
    }
}

#[cfg(test)]
mod level_tests {
    use super::*;

    #[test]
    fn silence_maps_to_zero() {
        assert_eq!(level_from_rms(0.0), 0);
        assert_eq!(level_from_rms(-1.0), 0);
        assert_eq!(level_from_rms(f32::NAN), 0);
    }

    #[test]
    fn full_scale_maps_to_maximum() {
        assert_eq!(level_from_rms(1.0), LEVEL_MAX_PERMILLE);
    }

    #[test]
    fn quiet_samples_stay_above_zero() {
        assert!(level_from_rms(0.01) > 0);
        assert!(level_from_rms(0.01) < LEVEL_MAX_PERMILLE);
    }

    #[test]
    fn meter_attacks_and_releases() {
        let meter = LevelMeter::default();
        meter.observe_at(&[1.0; 480], 1_000);
        let first = meter.permille_at(1_000);
        assert!(first > 0 && first < LEVEL_MAX_PERMILLE);
        meter.observe_at(&[1.0; 480], 1_020);
        let peak = meter.permille_at(1_020);
        assert!(peak > first);
        meter.observe_at(&[0.0; 480], 1_040);
        assert!(meter.permille_at(1_040) < peak);
    }

    #[test]
    fn meter_decays_when_audio_stops() {
        let meter = LevelMeter::default();
        for step in 0..40 {
            meter.observe_at(&[1.0; 480], 1_000 + step * 20);
        }
        assert!(meter.permille_at(1_800) > 900);
        assert!(meter.permille_at(2_000) < 900);
        assert_eq!(meter.permille_at(2_400), 0);
    }

    #[test]
    fn silence_resets_the_meter() {
        let meter = LevelMeter::default();
        meter.observe_at(&[1.0; 480], 1_000);
        meter.silence();
        assert_eq!(meter.permille_at(now_ms()), 0);
    }
}
