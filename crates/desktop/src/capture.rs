use std::os::fd::OwnedFd;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use thiserror::Error;

use crate::portal::PortalSession;

pub const MAX_FRAME_WIDTH: u32 = 1920;
pub const MAX_FRAME_HEIGHT: u32 = 1080;
pub const MAX_FRAME_BYTES: usize = 8 * 1024 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CaptureConfig {
    pub width: u32,
    pub height: u32,
    pub fps: u32,
}

impl Default for CaptureConfig {
    fn default() -> Self {
        Self {
            width: MAX_FRAME_WIDTH,
            height: MAX_FRAME_HEIGHT,
            fps: 30,
        }
    }
}

impl CaptureConfig {
    pub fn validate(&self) -> Result<(), CaptureError> {
        if self.width == 0
            || self.height == 0
            || self.width > MAX_FRAME_WIDTH
            || self.height > MAX_FRAME_HEIGHT
        {
            return Err(CaptureError::InvalidConfig);
        }
        if self.fps == 0 || self.fps > 60 {
            return Err(CaptureError::InvalidConfig);
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CapturedFrame {
    pub width: u32,
    pub height: u32,
    pub stride: u32,
    pub data: Vec<u8>,
}

pub trait Capture: Send {
    fn next_frame(&mut self) -> Result<Option<CapturedFrame>, CaptureError>;
    fn stop(&mut self);
}

pub struct PortalCapture {
    config: CaptureConfig,
    pipewire: Option<OwnedFd>,
    stopped: Arc<AtomicBool>,
    last_frame: Arc<Mutex<Option<CapturedFrame>>>,
}

impl PortalCapture {
    pub fn start(session: &mut PortalSession, config: CaptureConfig) -> Result<Self, CaptureError> {
        config.validate()?;
        let pipewire = session.take_pipewire();
        if pipewire.is_none() {
            return Err(CaptureError::NoPipeWire);
        }
        Ok(Self {
            config,
            pipewire,
            stopped: Arc::new(AtomicBool::new(false)),
            last_frame: Arc::new(Mutex::new(None)),
        })
    }

    pub fn config(&self) -> CaptureConfig {
        self.config
    }
}

impl Capture for PortalCapture {
    fn next_frame(&mut self) -> Result<Option<CapturedFrame>, CaptureError> {
        if self.stopped.load(Ordering::Relaxed) {
            return Ok(None);
        }
        if self.pipewire.is_none() {
            return Err(CaptureError::NoPipeWire);
        }
        Ok(self
            .last_frame
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone())
    }

    fn stop(&mut self) {
        self.stopped.store(true, Ordering::Relaxed);
        self.pipewire.take();
    }
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum CaptureError {
    #[error("capture configuration is invalid")]
    InvalidConfig,

    #[error("portal did not provide a PipeWire file descriptor")]
    NoPipeWire,

    #[error("capture frame exceeds the maximum size")]
    FrameTooLarge,
}

pub fn frame_interval(fps: u32) -> Duration {
    Duration::from_millis(1000 / u64::from(fps.max(1)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validates_capture_config() {
        assert!(CaptureConfig::default().validate().is_ok());
        assert!(
            CaptureConfig {
                width: 4096,
                ..CaptureConfig::default()
            }
            .validate()
            .is_err()
        );
        assert!(
            CaptureConfig {
                fps: 0,
                ..CaptureConfig::default()
            }
            .validate()
            .is_err()
        );
    }
}
