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

const INITIAL_SCRATCH_FRAMES: usize = 16_384;

pub struct PlaybackHandle {
    stop: Arc<AtomicBool>,
    stream: cpal::Stream,
    buffer: SharedBuffer,
    name: String,
    sample_rate: u32,
    channels: u16,
    sample_format: SampleFormat,
}

impl PlaybackHandle {
    pub fn name(&self) -> &str {
        &self.name
    }

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

    pub fn underruns(&self) -> u64 {
        self.buffer.underruns()
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
    let name = device.description().map_or_else(
        |_| "unknown output".to_owned(),
        |description| description.name().to_owned(),
    );
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
        name,
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
        SampleFormat::I8 => build_typed::<i8>(device, config, channels, buffer),
        SampleFormat::I16 => build_typed::<i16>(device, config, channels, buffer),
        SampleFormat::I24 => build_typed::<cpal::I24>(device, config, channels, buffer),
        SampleFormat::I32 => build_typed::<i32>(device, config, channels, buffer),
        SampleFormat::U8 => build_typed::<u8>(device, config, channels, buffer),
        SampleFormat::U16 => build_typed::<u16>(device, config, channels, buffer),
        SampleFormat::U24 => build_typed::<cpal::U24>(device, config, channels, buffer),
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
    let mut scratch = vec![0.0_f32; INITIAL_SCRATCH_FRAMES];
    let mut last = 0.0_f32;
    device
        .build_output_stream(
            config,
            move |data: &mut [T], _| {
                fill_output(data, channel_count, &buffer, &mut scratch, &mut last);
            },
            |error| tracing::warn!(%error, "audio playback stream error"),
            None,
        )
        .map_err(cpal_error)
}

fn fill_output<T>(
    data: &mut [T],
    channels: usize,
    buffer: &SharedBuffer,
    scratch: &mut Vec<f32>,
    last: &mut f32,
) where
    T: Sample + Copy + FromSample<f32>,
{
    let frames = data.len() / channels;
    if scratch.len() < frames {
        scratch.resize(frames, 0.0);
    }
    let filled = buffer.drain_into(&mut scratch[..frames]);
    if filled > 0 {
        *last = scratch[filled - 1];
    }
    let missing = frames - filled;
    for (index, frame) in data.chunks_mut(channels).enumerate() {
        let value = if index < filled {
            scratch[index]
        } else if missing == 0 {
            0.0
        } else {
            let remaining = frames - index;
            *last * (remaining.saturating_sub(1) as f32 / missing as f32)
        };
        let value = T::from_sample(value);
        for sample in frame {
            *sample = value;
        }
    }
    if missing > 0 {
        *last = 0.0;
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
        let mut scratch = vec![0.0; 8];
        let mut last = 0.0;
        let mut data = [0.0_f32; 4];
        fill_output(&mut data, 2, &buffer, &mut scratch, &mut last);
        assert_eq!(data, [0.25, 0.25, -0.5, -0.5]);
        assert_eq!(buffer.underruns(), 0);
    }

    #[test]
    fn missing_samples_fade_and_count_underruns() {
        let buffer = SharedBuffer::new();
        buffer.push(&[0.5]);
        let mut scratch = vec![0.0; 8];
        let mut last = 0.0;
        let mut data = [1.0_f32; 6];
        fill_output(&mut data, 2, &buffer, &mut scratch, &mut last);
        assert_eq!(data, [0.5, 0.5, 0.25, 0.25, 0.0, 0.0]);
        assert_eq!(buffer.underruns(), 1);
    }

    #[test]
    fn missing_samples_are_silent_when_last_is_zero() {
        let buffer = SharedBuffer::new();
        let mut scratch = vec![0.0; 8];
        let mut last = 0.0;
        let mut data = [1.0_f32; 4];
        fill_output(&mut data, 2, &buffer, &mut scratch, &mut last);
        assert_eq!(data, [0.0, 0.0, 0.0, 0.0]);
    }
}
