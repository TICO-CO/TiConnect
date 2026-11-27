use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use cpal::traits::{DeviceTrait, StreamTrait};
use cpal::{FromSample, Sample, SampleFormat, SizedSample};
use thiserror::Error;

use super::device::DeviceError;

pub trait CaptureSource: Send {
    fn read(&mut self) -> Result<Vec<f32>, CaptureError>;
    fn sample_rate(&self) -> u32;
}

pub struct CaptureHandle {
    stop: Arc<AtomicBool>,
    stream: cpal::Stream,
    name: String,
    sample_rate: u32,
    channels: u16,
    sample_format: SampleFormat,
}

impl CaptureHandle {
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

    pub fn set_muted(&self, muted: bool) {
        if muted {
            self.stream.pause().ok();
        } else if self.is_running() {
            self.stream.play().ok();
        }
    }

    pub fn stop(&self) {
        self.stop.store(true, Ordering::Relaxed);
        self.stream.pause().ok();
    }

    pub fn is_running(&self) -> bool {
        !self.stop.load(Ordering::Relaxed)
    }
}

impl Drop for CaptureHandle {
    fn drop(&mut self) {
        self.stop();
    }
}

pub fn start_capture(
    device_name: Option<&str>,
    callback: impl FnMut(Vec<f32>) + Send + 'static,
) -> Result<CaptureHandle, CaptureError> {
    use cpal::traits::{HostTrait, StreamTrait};
    let host = cpal::default_host();
    let device = match device_name {
        Some(name) => host
            .input_devices()
            .map_err(cpal_error)?
            .find(|device| {
                device
                    .description()
                    .is_ok_and(|description| description.name() == name)
            })
            .ok_or(DeviceError::Missing)?,
        None => host.default_input_device().ok_or(DeviceError::Missing)?,
    };
    let name = device.description().map_or_else(
        |_| "unknown input".to_owned(),
        |description| description.name().to_owned(),
    );
    if name.to_ascii_lowercase().contains("monitor") {
        tracing::warn!(
            device = %name,
            "call capture is using an output monitor instead of a microphone"
        );
    }
    let supported = device.default_input_config().map_err(cpal_error)?;
    let sample_rate = supported.sample_rate();
    let channels = supported.channels();
    let sample_format = supported.sample_format();
    let stop = Arc::new(AtomicBool::new(false));
    let stream = build_stream(
        &device,
        supported.config(),
        channels,
        sample_format,
        callback,
    )?;
    stream.play().map_err(cpal_error)?;
    Ok(CaptureHandle {
        stop,
        stream,
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
    callback: impl FnMut(Vec<f32>) + Send + 'static,
) -> Result<cpal::Stream, CaptureError> {
    match sample_format {
        SampleFormat::F32 => build_typed::<f32>(device, config, channels, callback),
        SampleFormat::F64 => build_typed::<f64>(device, config, channels, callback),
        SampleFormat::I8 => build_typed::<i8>(device, config, channels, callback),
        SampleFormat::I16 => build_typed::<i16>(device, config, channels, callback),
        SampleFormat::I24 => build_typed::<cpal::I24>(device, config, channels, callback),
        SampleFormat::I32 => build_typed::<i32>(device, config, channels, callback),
        SampleFormat::U8 => build_typed::<u8>(device, config, channels, callback),
        SampleFormat::U16 => build_typed::<u16>(device, config, channels, callback),
        SampleFormat::U24 => build_typed::<cpal::U24>(device, config, channels, callback),
        SampleFormat::U32 => build_typed::<u32>(device, config, channels, callback),
        other => Err(CaptureError::UnsupportedSampleFormat(other.to_string())),
    }
}

fn build_typed<T>(
    device: &cpal::Device,
    config: cpal::StreamConfig,
    channels: u16,
    mut callback: impl FnMut(Vec<f32>) + Send + 'static,
) -> Result<cpal::Stream, CaptureError>
where
    T: Sample + SizedSample + Send + 'static,
    f32: FromSample<T>,
{
    let channel_count = usize::from(channels);
    device
        .build_input_stream(
            config,
            move |data: &[T], _| callback(downmix(data, channel_count)),
            |error| tracing::warn!(%error, "audio capture stream error"),
            None,
        )
        .map_err(cpal_error)
}

fn downmix<T>(data: &[T], channels: usize) -> Vec<f32>
where
    T: Sample + Copy,
    f32: FromSample<T>,
{
    if channels <= 1 {
        return data
            .iter()
            .map(|sample| f32::from_sample(*sample))
            .collect();
    }
    data.chunks_exact(channels)
        .map(|frame| {
            let sum = frame
                .iter()
                .map(|sample| f32::from_sample(*sample))
                .sum::<f32>();
            sum / channels as f32
        })
        .collect()
}

#[derive(Debug, Error)]
pub enum CaptureError {
    #[error(transparent)]
    Device(#[from] DeviceError),

    #[error("audio capture failed: {0}")]
    Host(String),

    #[error("audio capture sample format is unsupported: {0}")]
    UnsupportedSampleFormat(String),
}

fn cpal_error(error: impl std::fmt::Display) -> CaptureError {
    CaptureError::Host(error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn downmixes_stereo_to_mono() {
        let samples = [0.5_f32, -0.5, 0.25, 0.75];
        assert_eq!(downmix(&samples, 2), vec![0.0, 0.5]);
    }

    #[test]
    fn keeps_mono_samples() {
        let samples = [0.5_f32, -0.5];
        assert_eq!(downmix(&samples, 1), samples);
    }
}
