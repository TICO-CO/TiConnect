use std::collections::BTreeMap;
use std::time::Instant;

use ticonnect_core::call::AudioPacket;

pub const DEFAULT_TARGET_DEPTH: usize = 3;
pub const MAX_TARGET_DEPTH: usize = 12;
pub const MAX_DEPTH: usize = 64;
pub const MAX_JITTER_MS: u64 = 500;
pub const FRAME_MS: u64 = 20;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum JitterFrame {
    Packet(AudioPacket),
    Lost(u32),
}

#[derive(Debug, Clone)]
pub struct JitterBuffer {
    base_target: usize,
    target: usize,
    next_sequence: Option<u32>,
    packets: BTreeMap<u32, AudioPacket>,
    lost: u64,
    received: u64,
    last_arrival: Option<(u32, Instant)>,
    arrival_jitter_ms: u64,
}

impl Default for JitterBuffer {
    fn default() -> Self {
        Self::new(DEFAULT_TARGET_DEPTH)
    }
}

impl JitterBuffer {
    pub fn new(target: usize) -> Self {
        let target = target.clamp(1, MAX_DEPTH);
        Self {
            base_target: target,
            target,
            next_sequence: None,
            packets: BTreeMap::new(),
            lost: 0,
            received: 0,
            last_arrival: None,
            arrival_jitter_ms: 0,
        }
    }

    pub fn target(&self) -> usize {
        self.target
    }

    pub fn depth(&self) -> usize {
        self.packets.len()
    }

    pub fn peek_next(&self) -> Option<&AudioPacket> {
        self.packets.values().next()
    }

    pub fn lost(&self) -> u64 {
        self.lost
    }

    pub fn received(&self) -> u64 {
        self.received
    }

    pub fn jitter_ms(&self) -> u64 {
        self.arrival_jitter_ms.min(MAX_JITTER_MS)
    }

    pub fn push(&mut self, packet: AudioPacket) {
        self.push_at(packet, Instant::now());
    }

    pub fn push_at(&mut self, packet: AudioPacket, now: Instant) {
        if self.packets.len() >= MAX_DEPTH {
            self.drop_oldest();
        }
        if let Some(next) = self.next_sequence
            && packet.sequence < next
        {
            return;
        }
        self.observe_arrival(packet.sequence, now);
        self.received = self.received.saturating_add(1);
        self.packets.entry(packet.sequence).or_insert(packet);
    }

    fn observe_arrival(&mut self, sequence: u32, now: Instant) {
        if let Some((previous_sequence, previous_at)) = self.last_arrival {
            if sequence > previous_sequence {
                let frames = u64::from(sequence - previous_sequence);
                let expected_ms = frames.saturating_mul(FRAME_MS);
                let actual_ms =
                    u64::try_from(now.saturating_duration_since(previous_at).as_millis())
                        .unwrap_or(u64::MAX)
                        .min(MAX_JITTER_MS);
                let deviation = actual_ms.abs_diff(expected_ms);
                self.arrival_jitter_ms = self
                    .arrival_jitter_ms
                    .saturating_mul(7)
                    .saturating_add(deviation)
                    / 8;
                let extra =
                    usize::try_from(self.arrival_jitter_ms / FRAME_MS).unwrap_or(usize::MAX);
                self.target = self
                    .base_target
                    .saturating_add(extra)
                    .clamp(self.base_target, MAX_TARGET_DEPTH.max(self.base_target));
            }
            if sequence > previous_sequence {
                self.last_arrival = Some((sequence, now));
            }
        } else {
            self.last_arrival = Some((sequence, now));
        }
    }

    pub fn pop(&mut self) -> Option<JitterFrame> {
        let next = match self.next_sequence {
            Some(next) => next,
            None => {
                let first = *self.packets.keys().next()?;
                if self.packets.len() < self.target {
                    return None;
                }
                self.next_sequence = Some(first);
                first
            }
        };
        if let Some(packet) = self.packets.remove(&next) {
            self.next_sequence = Some(next.wrapping_add(1));
            return Some(JitterFrame::Packet(packet));
        }
        if let Some(first) = self.packets.keys().next().copied() {
            if first > next {
                self.next_sequence = Some(next.wrapping_add(1));
                self.lost = self.lost.saturating_add(1);
                return Some(JitterFrame::Lost(next));
            }
            self.next_sequence = Some(first);
            return self.pop();
        }
        None
    }

    pub fn drain_ready(&mut self) -> Vec<JitterFrame> {
        let mut frames = Vec::new();
        while let Some(frame) = self.pop() {
            frames.push(frame);
        }
        frames
    }

    fn drop_oldest(&mut self) {
        if let Some(sequence) = self.packets.keys().next().copied() {
            self.packets.remove(&sequence);
            self.lost = self.lost.saturating_add(1);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;
    use ticonnect_core::call::CallId;

    fn packet(sequence: u32) -> AudioPacket {
        AudioPacket::new(
            CallId::generate(),
            sequence,
            sequence * 960,
            vec![sequence as u8],
        )
    }

    #[test]
    fn reorders_packets() {
        let mut buffer = JitterBuffer::new(2);
        buffer.push(packet(1));
        buffer.push(packet(0));
        assert_eq!(buffer.drain_ready().len(), 2);
    }

    #[test]
    fn conceals_lost_packets() {
        let mut buffer = JitterBuffer::new(1);
        buffer.push(packet(0));
        buffer.push(packet(2));
        let frames = buffer.drain_ready();
        assert!(matches!(frames[0], JitterFrame::Packet(_)));
        assert!(matches!(frames[1], JitterFrame::Lost(1)));
        assert!(matches!(frames[2], JitterFrame::Packet(_)));
        assert_eq!(buffer.lost(), 1);
    }

    #[test]
    fn adapts_target_to_arrival_jitter() {
        let mut buffer = JitterBuffer::new(3);
        let start = Instant::now();
        buffer.push_at(packet(0), start);
        buffer.push_at(packet(1), start + Duration::from_millis(20));
        buffer.push_at(packet(2), start + Duration::from_millis(40));
        assert!(buffer.pop().is_some());
        buffer.push_at(packet(4), start + Duration::from_millis(240));
        assert!(buffer.target() > 3, "target {}", buffer.target());
        assert!(buffer.jitter_ms() > 0);
    }

    #[test]
    fn bounds_depth() {
        let mut buffer = JitterBuffer::new(MAX_DEPTH);
        for sequence in 0..(MAX_DEPTH + 10) {
            buffer.push(packet(sequence as u32));
        }
        assert!(buffer.depth() <= MAX_DEPTH);
    }
}
