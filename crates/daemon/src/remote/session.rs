use thiserror::Error;

use crate::remote::state::{RemoteSessionId, RemoteState};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct RemoteMetrics {
    pub fps: u32,
    pub bitrate_kbps: u32,
    pub dropped_frames: u64,
    pub input_events: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RemoteCaptureState {
    pub session_id: RemoteSessionId,
    pub state: RemoteState,
    pub width: u32,
    pub height: u32,
    pub metrics: RemoteMetrics,
}

impl RemoteCaptureState {
    pub fn new(session_id: RemoteSessionId, width: u32, height: u32) -> Self {
        Self {
            session_id,
            state: RemoteState::Consented,
            width,
            height,
            metrics: RemoteMetrics::default(),
        }
    }

    pub fn start(&mut self) {
        self.state = RemoteState::Active;
    }

    pub fn stop(&mut self) {
        self.state = RemoteState::Ending;
    }

    pub fn frame_sent(&mut self) {
        self.metrics.fps = self.metrics.fps.saturating_add(1);
    }

    pub fn frame_dropped(&mut self) {
        self.metrics.dropped_frames = self.metrics.dropped_frames.saturating_add(1);
    }

    pub fn input_sent(&mut self) {
        self.metrics.input_events = self.metrics.input_events.saturating_add(1);
    }
}

#[derive(Debug, Error)]
pub enum RemoteSessionError {
    #[error("remote session is not active")]
    Inactive,

    #[error("remote session state is invalid")]
    InvalidState,
}
