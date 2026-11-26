use std::time::Instant;

use thiserror::Error;
use ticonnect_core::call::AudioPacket;

use crate::jitter::{JitterBuffer, JitterFrame};
use crate::opus_codec::{OpusDecoder, OpusError};

pub struct AudioPlayout {
    jitter: JitterBuffer,
    decoder: OpusDecoder,
    started: bool,
    start_ticks: u32,
    concealed: u64,
}

impl AudioPlayout {
    pub fn new() -> Result<Self, AudioPlayoutError> {
        Ok(Self {
            jitter: JitterBuffer::default(),
            decoder: OpusDecoder::default_voip()?,
            started: false,
            start_ticks: 0,
            concealed: 0,
        })
    }

    pub fn push(&mut self, packet: AudioPacket) {
        self.jitter.push(packet);
    }

    pub fn push_at(&mut self, packet: AudioPacket, now: Instant) {
        self.jitter.push_at(packet, now);
    }

    pub fn tick(&mut self) -> Result<Option<Vec<f32>>, AudioPlayoutError> {
        if !self.started {
            let depth = self.jitter.depth();
            if depth == 0 {
                return Ok(None);
            }
            if depth < self.jitter.target() && self.start_ticks < 15 {
                self.start_ticks = self.start_ticks.saturating_add(1);
                return Ok(None);
            }
            self.started = true;
        }
        match self.jitter.pop() {
            Some(JitterFrame::Packet(packet)) => Ok(Some(self.decoder.decode(&packet.payload)?)),
            Some(JitterFrame::Lost(sequence)) => {
                let fec = self.jitter.peek_next().and_then(|packet| {
                    (packet.sequence == sequence.wrapping_add(1)).then(|| packet.payload.clone())
                });
                if let Some(payload) = fec {
                    match self.decoder.decode_with_fec(&payload, true) {
                        Ok(samples) => return Ok(Some(samples)),
                        Err(error) => {
                            tracing::debug!(%error, "Opus FEC decode failed; concealing");
                        }
                    }
                }
                self.concealed = self.concealed.saturating_add(1);
                Ok(Some(self.decoder.conceal()?))
            }
            None => {
                self.concealed = self.concealed.saturating_add(1);
                Ok(Some(self.decoder.conceal()?))
            }
        }
    }

    pub fn reset(&mut self) {
        self.jitter = JitterBuffer::default();
        self.started = false;
        self.start_ticks = 0;
        self.concealed = 0;
    }

    pub fn depth(&self) -> usize {
        self.jitter.depth()
    }

    pub fn target(&self) -> usize {
        self.jitter.target()
    }

    pub fn lost(&self) -> u64 {
        self.jitter.lost()
    }

    pub fn received(&self) -> u64 {
        self.jitter.received()
    }

    pub fn concealed(&self) -> u64 {
        self.concealed
    }

    pub fn jitter_ms(&self) -> u64 {
        self.jitter.jitter_ms()
    }
}

#[derive(Debug, Error)]
pub enum AudioPlayoutError {
    #[error(transparent)]
    Opus(#[from] OpusError),
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;
    use ticonnect_core::call::CallId;

    use crate::audio::OPUS_FRAME_SAMPLES;
    use crate::opus_codec::OpusEncoder;

    fn packets(count: usize) -> Vec<AudioPacket> {
        let mut encoder = OpusEncoder::default_voip().expect("encoder");
        let call_id = CallId::generate();
        encoder
            .push_samples(&vec![0.1; OPUS_FRAME_SAMPLES * count])
            .expect("encode")
            .into_iter()
            .enumerate()
            .map(|(index, payload)| {
                AudioPacket::new(
                    call_id,
                    u32::try_from(index).expect("sequence"),
                    u32::try_from(index * OPUS_FRAME_SAMPLES).expect("timestamp"),
                    payload,
                )
            })
            .collect()
    }

    #[test]
    fn waits_for_target_before_starting() {
        let mut playout = AudioPlayout::new().expect("playout");
        let packets = packets(3);
        let start = Instant::now();
        playout.push_at(packets[0].clone(), start);
        playout.push_at(packets[1].clone(), start);
        assert!(playout.tick().expect("tick").is_none());
        playout.push_at(packets[2].clone(), start);
        assert!(playout.tick().expect("tick").is_some());
    }

    #[test]
    fn starts_after_a_deadline_without_full_target() {
        let mut playout = AudioPlayout::new().expect("playout");
        playout.push(packets(1).remove(0));
        let mut started = false;
        for _ in 0..16 {
            started |= playout.tick().expect("tick").is_some();
        }
        assert!(started);
    }

    #[test]
    fn conceals_during_a_stall_instead_of_dropping_audio() {
        let mut playout = AudioPlayout::new().expect("playout");
        let packets = packets(3);
        let start = Instant::now();
        for (index, packet) in packets.into_iter().enumerate() {
            playout.push_at(packet, start + Duration::from_millis(20 * index as u64));
        }
        for _ in 0..3 {
            assert!(playout.tick().expect("tick").is_some());
        }
        let concealed = playout.tick().expect("tick").expect("concealment");
        assert!(!concealed.is_empty());
        assert_eq!(playout.concealed(), 1);
    }

    #[test]
    fn recovers_after_a_lost_packet() {
        let mut playout = AudioPlayout::new().expect("playout");
        let mut packets = packets(5);
        packets.remove(3);
        let start = Instant::now();
        for (index, packet) in packets.into_iter().enumerate() {
            playout.push_at(packet, start + Duration::from_millis(20 * index as u64));
        }
        for _ in 0..4 {
            assert!(playout.tick().expect("tick").is_some());
        }
        assert!(playout.tick().expect("tick").is_some());
        assert!(playout.lost() >= 1);
    }
}
