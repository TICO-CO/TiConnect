use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use cpal::traits::StreamTrait;
use thiserror::Error;

use super::device::DeviceError;

pub trait CaptureSource: Send {
    fn read(&mut self) -> Result<Vec<f32>, CaptureError>;
    fn sample_rate(&self) -> u32;
}

pub struct CaptureHandle {
    stop: Arc<AtomicBool>,
    stream: cpal::Stream,
    sample_rate: u32,
}

impl CaptureHandle {
    pub fn sample_rate(&self) -> u32 {
        self.sample_rate
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
    use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
    let host = cpal::default_host();
    let device = match device_name {
        Some(name) => host.input_devices().map_err(cpal_error)?.find(|device| {
            device
                .description()
                .is_ok_and(|description| description.name() == name)
        }),
        None => Some(host.default_input_device().ok_or(DeviceError::Missing)?),
    }
    .ok_or(DeviceError::Missing)?;
    let config = device.default_input_config().map_err(cpal_error)?;
    let sample_rate = config.sample_rate();
    let stop = Arc::new(AtomicBool::new(false));
    let mut callback = callback;
    let stream = device
        .build_input_stream(
            config.config(),
            move |data: &[f32], _| callback(data.to_vec()),
            |error| tracing::warn!(%error, "audio capture stream error"),
            None,
        )
        .map_err(cpal_error)?;
    stream.play().map_err(cpal_error)?;
    Ok(CaptureHandle {
        stop,
        stream,
        sample_rate,
    })
}

#[derive(Debug, Error)]
pub enum CaptureError {
    #[error(transparent)]
    Device(#[from] DeviceError),

    #[error("audio capture failed: {0}")]
    Host(String),
}

fn cpal_error(error: impl std::fmt::Display) -> CaptureError {
    CaptureError::Host(error.to_string())
}
