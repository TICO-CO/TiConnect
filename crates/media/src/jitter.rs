use std::collections::BTreeMap;

use ticonnect_core::call::AudioPacket;

pub const DEFAULT_TARGET_DEPTH: usize = 3;
pub const MAX_DEPTH: usize = 64;
pub const MAX_JITTER_MS: u64 = 500;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum JitterFrame {
    Packet(AudioPacket),
    Lost(u32),
}

#[derive(Debug, Clone)]
pub struct JitterBuffer {
    target: usize,
    next_sequence: Option<u32>,
    packets: BTreeMap<u32, AudioPacket>,
    lost: u64,
    received: u64,
    max_depth: usize,
}

impl Default for JitterBuffer {
    fn default() -> Self {
        Self::new(DEFAULT_TARGET_DEPTH)
    }
}

impl JitterBuffer {
    pub fn new(target: usize) -> Self {
        Self {
            target: target.clamp(1, MAX_DEPTH),
            next_sequence: None,
            packets: BTreeMap::new(),
            lost: 0,
            received: 0,
            max_depth: 0,
        }
    }

    pub fn target(&self) -> usize {
        self.target
    }

    pub fn depth(&self) -> usize {
        self.packets.len()
    }

    pub fn lost(&self) -> u64 {
        self.lost
    }

    pub fn received(&self) -> u64 {
        self.received
    }

    pub fn jitter_ms(&self) -> u64 {
        u64::try_from(self.max_depth.saturating_sub(self.target))
            .unwrap_or_default()
            .saturating_mul(20)
            .min(MAX_JITTER_MS)
    }

    pub fn push(&mut self, packet: AudioPacket) {
        if self.packets.len() >= MAX_DEPTH {
            self.drop_oldest();
        }
        if let Some(next) = self.next_sequence
            && packet.sequence < next
        {
            return;
        }
        self.received = self.received.saturating_add(1);
        self.packets.entry(packet.sequence).or_insert(packet);
        self.max_depth = self.max_depth.max(self.packets.len());
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
    fn bounds_depth() {
        let mut buffer = JitterBuffer::new(MAX_DEPTH);
        for sequence in 0..(MAX_DEPTH + 10) {
            buffer.push(packet(sequence as u32));
        }
        assert!(buffer.depth() <= MAX_DEPTH);
    }
}
