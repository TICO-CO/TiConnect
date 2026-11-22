use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use cpal::traits::{DeviceTrait, StreamTrait};
use cpal::{FromSample, Sample, SampleFormat, SizedSample};
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
    channels: u16,
    sample_format: SampleFormat,
}

impl PlaybackHandle {
    pub fn sample_rate(&self) -> u32 {
        self.sample_rate
    }

    pub fn channels(&self) -> u16 {
        self.channels
    }

    pub fn sample_format(&self) -> SampleFormat {
        self.sample_format
    }

    pub fn push(&self, samples: &[f32]) {
        self.buffer.push(samples);
    }

    pub fn clear(&self) {
        self.buffer.clear();
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
    use cpal::traits::{HostTrait, StreamTrait};
    let host = cpal::default_host();
    let device = match device_name {
        Some(name) => host
            .output_devices()
            .map_err(cpal_error)?
            .find(|device| {
                device
                    .description()
                    .is_ok_and(|description| description.name() == name)
            })
            .ok_or(DeviceError::Missing)?,
        None => host.default_output_device().ok_or(DeviceError::Missing)?,
    };
    let supported = device.default_output_config().map_err(cpal_error)?;
    let sample_rate = supported.sample_rate();
    let channels = supported.channels();
    let sample_format = supported.sample_format();
    let stop = Arc::new(AtomicBool::new(false));
    let buffer = SharedBuffer::new();
    let stream = build_stream(
        &device,
        supported.config(),
        channels,
        sample_format,
        buffer.clone(),
    )?;
    stream.play().map_err(cpal_error)?;
    Ok(PlaybackHandle {
        stop,
        stream,
        buffer,
        sample_rate,
        channels,
        sample_format,
    })
}

fn build_stream(
    device: &cpal::Device,
    config: cpal::StreamConfig,
    channels: u16,
    sample_format: SampleFormat,
    buffer: SharedBuffer,
) -> Result<cpal::Stream, PlaybackError> {
    match sample_format {
        SampleFormat::F32 => build_typed::<f32>(device, config, channels, buffer),
        SampleFormat::F64 => build_typed::<f64>(device, config, channels, buffer),
        SampleFormat::I16 => build_typed::<i16>(device, config, channels, buffer),
        SampleFormat::I32 => build_typed::<i32>(device, config, channels, buffer),
        SampleFormat::U16 => build_typed::<u16>(device, config, channels, buffer),
        SampleFormat::U32 => build_typed::<u32>(device, config, channels, buffer),
        other => Err(PlaybackError::UnsupportedSampleFormat(other.to_string())),
    }
}

fn build_typed<T>(
    device: &cpal::Device,
    config: cpal::StreamConfig,
    channels: u16,
    buffer: SharedBuffer,
) -> Result<cpal::Stream, PlaybackError>
where
    T: Sample + SizedSample + Copy + Send + 'static,
    T: FromSample<f32>,
{
    let channel_count = usize::from(channels).max(1);
    device
        .build_output_stream(
            config,
            move |data: &mut [T], _| fill_output(data, channel_count, &buffer),
            |error| tracing::warn!(%error, "audio playback stream error"),
            None,
        )
        .map_err(cpal_error)
}

fn fill_output<T>(data: &mut [T], channels: usize, buffer: &SharedBuffer)
where
    T: Sample + Copy + FromSample<f32>,
{
    let frames = data.len() / channels;
    let samples = buffer.drain(frames);
    for (index, frame) in data.chunks_mut(channels).enumerate() {
        let value = T::from_sample(samples.get(index).copied().unwrap_or(0.0));
        for sample in frame {
            *sample = value;
        }
    }
}

#[derive(Debug, Error)]
pub enum PlaybackError {
    #[error(transparent)]
    Device(#[from] DeviceError),

    #[error("audio playback failed: {0}")]
    Host(String),

    #[error("audio playback sample format is unsupported: {0}")]
    UnsupportedSampleFormat(String),
}

fn cpal_error(error: impl std::fmt::Display) -> PlaybackError {
    PlaybackError::Host(error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn expands_mono_samples_to_every_output_channel() {
        let buffer = SharedBuffer::new();
        buffer.push(&[0.25, -0.5]);
        let mut data = [0.0_f32; 4];
        fill_output(&mut data, 2, &buffer);
        assert_eq!(data, [0.25, 0.25, -0.5, -0.5]);
    }

    #[test]
    fn missing_samples_are_silent() {
        let buffer = SharedBuffer::new();
        let mut data = [1.0_f32; 4];
        fill_output(&mut data, 2, &buffer);
        assert_eq!(data, [0.0, 0.0, 0.0, 0.0]);
    }
}
