use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};

use super::CallMetricsHandle;

use thiserror::Error;
use ticonnect_core::call::{AudioPacket, CallId};
use ticonnect_media::audio::{
    AUDIO_VOLUME_MAX_PERCENT, CaptureHandle, OPUS_SAMPLE_RATE, PlaybackHandle, Resampler,
    apply_gain, percent_gain, start_capture, start_playback,
};
use ticonnect_media::opus_codec::OpusEncoder;
use ticonnect_net::transport::Session;
use tokio::task::JoinHandle;

#[derive(Debug, Clone, Copy)]
pub struct MediaSessionSettings {
    pub input_muted: bool,
    pub output_muted: bool,
    pub input_volume: u32,
    pub output_volume: u32,
}

pub struct MediaSession {
    call_id: CallId,
    capture: CaptureHandle,
    playback: PlaybackHandle,
    output_resampler: Resampler,
    input_muted: Arc<AtomicBool>,
    output_muted: Arc<AtomicBool>,
    input_volume: Arc<AtomicU32>,
    output_volume: Arc<AtomicU32>,
    user_volume: Arc<AtomicU32>,
    encoder_task: JoinHandle<()>,
}

impl MediaSession {
    pub fn start(
        call_id: CallId,
        session: Session,
        input_device: Option<&str>,
        output_device: Option<&str>,
        settings: MediaSessionSettings,
        metrics: Arc<CallMetricsHandle>,
    ) -> Result<Self, MediaError> {
        let (sender, mut receiver) = tokio::sync::mpsc::unbounded_channel();
        let input_volume_flag = Arc::new(AtomicU32::new(settings.input_volume));
        let capture_volume = input_volume_flag.clone();
        let capture = start_capture(input_device, move |mut samples| {
            apply_gain(
                &mut samples,
                percent_gain(capture_volume.load(Ordering::Relaxed)),
            );
            let _ = sender.send(samples);
        })?;
        capture.set_muted(settings.input_muted);
        let input_rate = capture.sample_rate();
        let playback = start_playback(output_device)?;
        let output_rate = playback.sample_rate();
        let input_resampler = Resampler::new(input_rate, OPUS_SAMPLE_RATE)?;
        let output_resampler = Resampler::new(OPUS_SAMPLE_RATE, output_rate)?;
        let input_muted_flag = Arc::new(AtomicBool::new(settings.input_muted));
        let output_muted_flag = Arc::new(AtomicBool::new(settings.output_muted));
        let output_volume_flag = Arc::new(AtomicU32::new(settings.output_volume));
        let user_volume_flag = Arc::new(AtomicU32::new(AUDIO_VOLUME_MAX_PERCENT));
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
            input_volume: input_volume_flag,
            output_volume: output_volume_flag,
            user_volume: user_volume_flag,
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

    pub fn set_input_volume(&self, percent: u32) {
        self.input_volume.store(percent, Ordering::Relaxed);
    }

    pub fn set_output_volume(&self, percent: u32) {
        self.output_volume.store(percent, Ordering::Relaxed);
    }

    pub fn set_user_volume(&self, percent: u32) {
        self.user_volume.store(percent, Ordering::Relaxed);
    }

    pub fn call_id(&self) -> CallId {
        self.call_id
    }

    pub fn push_audio(&mut self, samples: &[f32]) -> Result<(), MediaError> {
        if self.output_muted.load(Ordering::Relaxed) {
            return Ok(());
        }
        let mut samples = self.output_resampler.process(samples)?;
        let gain = percent_gain(self.output_volume.load(Ordering::Relaxed))
            * percent_gain(self.user_volume.load(Ordering::Relaxed));
        apply_gain(&mut samples, gain);
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
