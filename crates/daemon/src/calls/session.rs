use std::sync::Arc;

use thiserror::Error;
use ticonnect_core::call::{AudioPacket, CallId};
use ticonnect_media::audio::{
    CaptureHandle, OPUS_SAMPLE_RATE, PlaybackHandle, Resampler, start_capture, start_playback,
};
use ticonnect_media::opus_codec::{OpusDecoder, OpusEncoder};
use ticonnect_net::transport::Session;
use tokio::task::JoinHandle;

pub struct MediaSession {
    call_id: CallId,
    capture: CaptureHandle,
    playback: PlaybackHandle,
    output_resampler: Resampler,
    encoder_task: JoinHandle<()>,
}

impl MediaSession {
    pub fn start(
        call_id: CallId,
        session: Session,
        input_device: Option<&str>,
        output_device: Option<&str>,
    ) -> Result<Self, MediaError> {
        let playback = start_playback(output_device)?;
        let output_rate = playback.sample_rate();
        let (sender, mut receiver) = tokio::sync::mpsc::unbounded_channel();
        let capture = start_capture(input_device, move |samples| {
            let _ = sender.send(samples);
        })?;
        let input_rate = capture.sample_rate();
        let encoder_task = tokio::spawn(async move {
            let mut encoder = match OpusEncoder::default_voip() {
                Ok(encoder) => encoder,
                Err(error) => {
                    tracing::warn!(%error, "Opus encoder unavailable");
                    return;
                }
            };
            let mut resampler = Resampler::new(input_rate, OPUS_SAMPLE_RATE);
            let mut sequence = 0_u32;
            let mut timestamp = 0_u32;
            while let Some(samples) = receiver.recv().await {
                let samples = resampler.process(&samples);
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
                            }
                        }
                        Err(error) => tracing::debug!(%error, "audio packet encode failed"),
                    }
                }
            }
        });
        Ok(Self {
            call_id,
            capture,
            playback,
            output_resampler: Resampler::new(OPUS_SAMPLE_RATE, output_rate),
            encoder_task,
        })
    }

    pub fn call_id(&self) -> CallId {
        self.call_id
    }

    pub fn push_audio(&mut self, samples: &[f32]) {
        let samples = self.output_resampler.process(samples);
        self.playback.push(&samples);
    }

    pub fn output_depth(&self) -> usize {
        self.playback.depth()
    }

    pub fn stop(&self) {
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

pub struct AudioReceiver {
    decoder: OpusDecoder,
}

impl AudioReceiver {
    pub fn new() -> Result<Self, MediaError> {
        Ok(Self {
            decoder: OpusDecoder::default_voip()?,
        })
    }

    pub fn decode(&mut self, packet: &AudioPacket) -> Result<Vec<f32>, MediaError> {
        Ok(self.decoder.decode(&packet.payload)?)
    }

    pub fn conceal(&mut self) -> Result<Vec<f32>, MediaError> {
        Ok(self.decoder.conceal()?)
    }
}

#[derive(Debug, Error)]
pub enum MediaError {
    #[error(transparent)]
    Capture(#[from] ticonnect_media::audio::capture::CaptureError),

    #[error(transparent)]
    Playback(#[from] ticonnect_media::audio::playback::PlaybackError),

    #[error(transparent)]
    Opus(#[from] ticonnect_media::opus_codec::OpusError),
}

impl Clone for AudioReceiver {
    fn clone(&self) -> Self {
        Self::new().expect("Opus decoder")
    }
}

impl std::fmt::Debug for AudioReceiver {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("AudioReceiver")
    }
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
