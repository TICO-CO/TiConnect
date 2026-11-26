use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use super::CallMetricsHandle;

use thiserror::Error;
use ticonnect_core::call::{AudioPacket, CallId};
use ticonnect_media::audio::{
    CaptureHandle, OPUS_SAMPLE_RATE, PlaybackHandle, Resampler, start_capture, start_playback,
};
use ticonnect_media::opus_codec::OpusEncoder;
use ticonnect_net::transport::Session;
use tokio::task::JoinHandle;

pub struct MediaSession {
    call_id: CallId,
    capture: CaptureHandle,
    playback: PlaybackHandle,
    output_resampler: Resampler,
    input_muted: Arc<AtomicBool>,
    output_muted: Arc<AtomicBool>,
    encoder_task: JoinHandle<()>,
}

impl MediaSession {
    pub fn start(
        call_id: CallId,
        session: Session,
        input_device: Option<&str>,
        output_device: Option<&str>,
        input_muted: bool,
        output_muted: bool,
        metrics: Arc<CallMetricsHandle>,
    ) -> Result<Self, MediaError> {
        let (sender, mut receiver) = tokio::sync::mpsc::unbounded_channel();
        let capture = start_capture(input_device, move |samples| {
            let _ = sender.send(samples);
        })?;
        capture.set_muted(input_muted);
        let input_rate = capture.sample_rate();
        let playback = start_playback(output_device)?;
        let output_rate = playback.sample_rate();
        let input_resampler = Resampler::new(input_rate, OPUS_SAMPLE_RATE)?;
        let output_resampler = Resampler::new(OPUS_SAMPLE_RATE, output_rate)?;
        let input_muted_flag = Arc::new(AtomicBool::new(input_muted));
        let output_muted_flag = Arc::new(AtomicBool::new(output_muted));
        let encoder_muted = input_muted_flag.clone();
        let encoder_task = tokio::spawn(async move {
            let mut encoder = match OpusEncoder::default_voip() {
                Ok(encoder) => encoder,
                Err(error) => {
                    tracing::warn!(%error, "Opus encoder unavailable");
                    return;
                }
            };
            let mut resampler = input_resampler;
            let mut sequence = 0_u32;
            let mut timestamp = 0_u32;
            while let Some(samples) = receiver.recv().await {
                if encoder_muted.load(Ordering::Relaxed) {
                    continue;
                }
                let samples = match resampler.process(&samples) {
                    Ok(samples) => samples,
                    Err(error) => {
                        tracing::debug!(%error, "audio resample failed");
                        continue;
                    }
                };
                let frames = match encoder.push_samples(&samples) {
                    Ok(frames) => frames,
                    Err(error) => {
                        tracing::debug!(%error, "audio encode failed");
                        continue;
                    }
                };
                for payload in frames {
                    let packet = AudioPacket::new(call_id, sequence, timestamp, payload);
                    sequence = sequence.wrapping_add(1);
                    timestamp = timestamp.wrapping_add(960);
                    match packet.encode() {
                        Ok(bytes) => {
                            if let Err(error) = session.send_datagram(&bytes) {
                                tracing::debug!(%error, "audio datagram send failed");
                            } else {
                                metrics.note_sent(bytes.len());
                            }
                        }
                        Err(error) => tracing::debug!(%error, "audio packet encode failed"),
                    }
                }
            }
        });
        tracing::info!(
            call_id = %call_id,
            input_device = capture.name(),
            input_rate = input_rate,
            input_channels = capture.channels(),
            input_format = ?capture.sample_format(),
            output_device = playback.name(),
            output_rate = output_rate,
            output_channels = playback.channels(),
            output_format = ?playback.sample_format(),
            "call media session started"
        );
        Ok(Self {
            call_id,
            capture,
            playback,
            output_resampler,
            input_muted: input_muted_flag,
            output_muted: output_muted_flag,
            encoder_task,
        })
    }

    pub fn set_input_muted(&self, muted: bool) {
        self.input_muted.store(muted, Ordering::Relaxed);
        self.capture.set_muted(muted);
    }

    pub fn set_output_muted(&self, muted: bool) {
        self.output_muted.store(muted, Ordering::Relaxed);
        if muted {
            self.playback.clear();
        }
    }

    pub fn call_id(&self) -> CallId {
        self.call_id
    }

    pub fn push_audio(&mut self, samples: &[f32]) -> Result<(), MediaError> {
        if self.output_muted.load(Ordering::Relaxed) {
            return Ok(());
        }
        let samples = self.output_resampler.process(samples)?;
        self.playback.push(&samples);
        Ok(())
    }

    pub fn output_depth(&self) -> usize {
        self.playback.depth()
    }

    pub fn output_underruns(&self) -> u64 {
        self.playback.underruns()
    }

    pub fn stop(&self) {
        tracing::info!(
            call_id = %self.call_id,
            playback_underruns = self.output_underruns(),
            "call media session stopped"
        );
        self.capture.stop();
        self.playback.stop();
        self.encoder_task.abort();
    }
}

impl Drop for MediaSession {
    fn drop(&mut self) {
        self.capture.stop();
        self.playback.stop();
        self.encoder_task.abort();
    }
}

#[derive(Debug, Error)]
pub enum MediaError {
    #[error(transparent)]
    Capture(#[from] ticonnect_media::audio::capture::CaptureError),

    #[error(transparent)]
    Playback(#[from] ticonnect_media::audio::playback::PlaybackError),

    #[error(transparent)]
    Resample(#[from] ticonnect_media::audio::ResampleError),

    #[error(transparent)]
    Playout(#[from] ticonnect_media::audio::AudioPlayoutError),

    #[error(transparent)]
    Opus(#[from] ticonnect_media::opus_codec::OpusError),
}

impl std::fmt::Debug for MediaSession {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("MediaSession")
            .field("call_id", &self.call_id)
            .field("capture_rate", &self.capture.sample_rate())
            .field("playback_rate", &self.playback.sample_rate())
            .finish()
    }
}

pub type SharedMedia = Arc<std::sync::Mutex<Option<MediaSession>>>;
