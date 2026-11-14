pub mod capture;
pub mod device;
pub mod playback;
pub mod resample;

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, PoisonError};

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

    pub fn depth(&self) -> usize {
        self.0.lock().unwrap_or_else(PoisonError::into_inner).len()
    }
}
