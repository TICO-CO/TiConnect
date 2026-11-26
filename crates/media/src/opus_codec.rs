use opus::{Application, Bitrate, Channels, Signal};
use thiserror::Error;

use crate::audio::{OPUS_CHANNELS, OPUS_FRAME_SAMPLES, OPUS_SAMPLE_RATE};

pub const DEFAULT_OPUS_BITRATE: i32 = 32_000;
pub const MAX_OPUS_PACKET: usize = 1024;

pub struct OpusEncoder {
    encoder: opus::Encoder,
    sequence_input: Vec<f32>,
}

impl OpusEncoder {
    pub fn new(sample_rate: u32, bitrate: i32) -> Result<Self, OpusError> {
        let mut encoder = opus::Encoder::new(
            sample_rate,
            if OPUS_CHANNELS == 1 {
                Channels::Mono
            } else {
                Channels::Stereo
            },
            Application::Voip,
        )
        .map_err(|error| OpusError::Codec(error.to_string()))?;
        encoder
            .set_bitrate(Bitrate::Bits(bitrate))
            .map_err(|error| OpusError::Codec(error.to_string()))?;
        encoder
            .set_complexity(10)
            .map_err(|error| OpusError::Codec(error.to_string()))?;
        encoder
            .set_signal(Signal::Voice)
            .map_err(|error| OpusError::Codec(error.to_string()))?;
        encoder
            .set_inband_fec(true)
            .map_err(|error| OpusError::Codec(error.to_string()))?;
        encoder
            .set_packet_loss_perc(10)
            .map_err(|error| OpusError::Codec(error.to_string()))?;
        Ok(Self {
            encoder,
            sequence_input: Vec::with_capacity(OPUS_FRAME_SAMPLES * 2),
        })
    }

    pub fn default_voip() -> Result<Self, OpusError> {
        Self::new(OPUS_SAMPLE_RATE, DEFAULT_OPUS_BITRATE)
    }

    pub fn encode(&mut self, frame: &[f32]) -> Result<Vec<u8>, OpusError> {
        if frame.len() != OPUS_FRAME_SAMPLES {
            return Err(OpusError::FrameSize(frame.len()));
        }
        let mut output = [0_u8; MAX_OPUS_PACKET];
        let length = self
            .encoder
            .encode_float(frame, &mut output)
            .map_err(|error| OpusError::Codec(error.to_string()))?;
        Ok(output[..length].to_vec())
    }

    pub fn push_samples(&mut self, samples: &[f32]) -> Result<Vec<Vec<u8>>, OpusError> {
        self.sequence_input.extend_from_slice(samples);
        let mut frames = Vec::new();
        while self.sequence_input.len() >= OPUS_FRAME_SAMPLES {
            let frame: Vec<f32> = self.sequence_input.drain(..OPUS_FRAME_SAMPLES).collect();
            frames.push(self.encode(&frame)?);
        }
        Ok(frames)
    }
}

pub struct OpusDecoder {
    decoder: opus::Decoder,
}

impl OpusDecoder {
    pub fn new(sample_rate: u32) -> Result<Self, OpusError> {
        Ok(Self {
            decoder: opus::Decoder::new(
                sample_rate,
                if OPUS_CHANNELS == 1 {
                    Channels::Mono
                } else {
                    Channels::Stereo
                },
            )
            .map_err(|error| OpusError::Codec(error.to_string()))?,
        })
    }

    pub fn default_voip() -> Result<Self, OpusError> {
        Self::new(OPUS_SAMPLE_RATE)
    }

    pub fn decode(&mut self, packet: &[u8]) -> Result<Vec<f32>, OpusError> {
        self.decode_with_fec(packet, false)
    }

    pub fn decode_with_fec(
        &mut self,
        packet: &[u8],
        decode_fec: bool,
    ) -> Result<Vec<f32>, OpusError> {
        if packet.is_empty() {
            return self.conceal();
        }
        let mut output = vec![0.0_f32; OPUS_FRAME_SAMPLES * 3];
        let samples = self
            .decoder
            .decode_float(packet, &mut output, decode_fec)
            .map_err(|error| OpusError::Codec(error.to_string()))?;
        output.truncate(samples);
        Ok(output)
    }

    pub fn conceal(&mut self) -> Result<Vec<f32>, OpusError> {
        let mut output = vec![0.0_f32; OPUS_FRAME_SAMPLES * 3];
        let samples = self
            .decoder
            .decode_float(&[], &mut output, false)
            .map_err(|error| OpusError::Codec(error.to_string()))?;
        output.truncate(samples);
        Ok(output)
    }
}

#[derive(Debug, Error)]
pub enum OpusError {
    #[error("Opus codec failed: {0}")]
    Codec(String),

    #[error("Opus frame has {0} samples, expected {OPUS_FRAME_SAMPLES}")]
    FrameSize(usize),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encodes_and_decodes_twenty_millisecond_frames() {
        let mut encoder = OpusEncoder::default_voip().expect("encoder");
        let mut decoder = OpusDecoder::default_voip().expect("decoder");
        let input: Vec<f32> = (0..OPUS_FRAME_SAMPLES)
            .map(|index| (index as f32 * 0.01).sin() * 0.25)
            .collect();
        let packet = encoder.encode(&input).expect("encode");
        assert!(!packet.is_empty());
        let output = decoder.decode(&packet).expect("decode");
        assert!(!output.is_empty());
    }

    #[test]
    fn fec_decodes_the_frame_before_a_lost_packet() {
        let mut encoder = OpusEncoder::default_voip().expect("encoder");
        let packets = encoder
            .push_samples(&vec![0.1; OPUS_FRAME_SAMPLES * 3])
            .expect("encode");
        let mut decoder = OpusDecoder::default_voip().expect("decoder");
        assert!(
            !decoder
                .decode(&packets[0])
                .expect("first packet")
                .is_empty()
        );
        let recovered = decoder
            .decode_with_fec(&packets[2], true)
            .expect("fec packet");
        assert!(!recovered.is_empty());
        assert!(
            !decoder
                .decode(&packets[2])
                .expect("current packet")
                .is_empty()
        );
    }

    #[test]
    fn buffers_partial_frames() {
        let mut encoder = OpusEncoder::default_voip().expect("encoder");
        let frames = encoder
            .push_samples(&vec![0.0; OPUS_FRAME_SAMPLES - 1])
            .expect("partial");
        assert!(frames.is_empty());
        let frames = encoder.push_samples(&[0.0]).expect("complete");
        assert_eq!(frames.len(), 1);
    }
}
