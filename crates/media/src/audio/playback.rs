use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use cpal::traits::StreamTrait;
use thiserror::Error;

use super::SharedBuffer;
use super::device::DeviceError;

pub trait PlaybackSink: Send {
    fn push(&mut self, samples: &[f32]) -> Result<(), PlaybackError>;
    fn depth(&self) -> usize;
}

pub struct PlaybackHandle {
    stop: Arc<AtomicBool>,
    stream: cpal::Stream,
    buffer: SharedBuffer,
    sample_rate: u32,
}

impl PlaybackHandle {
    pub fn sample_rate(&self) -> u32 {
        self.sample_rate
    }

    pub fn push(&self, samples: &[f32]) {
        self.buffer.push(samples);
    }

    pub fn depth(&self) -> usize {
        self.buffer.depth()
    }

    pub fn stop(&self) {
        self.stop.store(true, Ordering::Relaxed);
        self.stream.pause().ok();
    }

    pub fn is_running(&self) -> bool {
        !self.stop.load(Ordering::Relaxed)
    }
}

impl Drop for PlaybackHandle {
    fn drop(&mut self) {
        self.stop();
    }
}

pub fn start_playback(device_name: Option<&str>) -> Result<PlaybackHandle, PlaybackError> {
    use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
    let host = cpal::default_host();
    let device = match device_name {
        Some(name) => host.output_devices().map_err(cpal_error)?.find(|device| {
            device
                .description()
                .is_ok_and(|description| description.name() == name)
        }),
        None => Some(host.default_output_device().ok_or(DeviceError::Missing)?),
    }
    .ok_or(DeviceError::Missing)?;
    let config = device.default_output_config().map_err(cpal_error)?;
    let sample_rate = config.sample_rate();
    let stop = Arc::new(AtomicBool::new(false));
    let buffer = SharedBuffer::new();
    let callback_buffer = buffer.clone();
    let stream = device
        .build_output_stream(
            config.config(),
            move |data: &mut [f32], _| {
                let samples = callback_buffer.drain(data.len());
                data[..samples.len()].copy_from_slice(&samples);
                for sample in &mut data[samples.len()..] {
                    *sample = 0.0;
                }
            },
            |error| tracing::warn!(%error, "audio playback stream error"),
            None,
        )
        .map_err(cpal_error)?;
    stream.play().map_err(cpal_error)?;
    Ok(PlaybackHandle {
        stop,
        stream,
        buffer,
        sample_rate,
    })
}

#[derive(Debug, Error)]
pub enum PlaybackError {
    #[error(transparent)]
    Device(#[from] DeviceError),

    #[error("audio playback failed: {0}")]
    Host(String),
}

fn cpal_error(error: impl std::fmt::Display) -> PlaybackError {
    PlaybackError::Host(error.to_string())
}
