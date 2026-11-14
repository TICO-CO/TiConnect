use super::{EncodedFrame, VideoError};

pub const MAX_PACKET_BYTES: usize = 96 * 1024;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FramePacket {
    pub sequence: u64,
    pub offset: u64,
    pub total: u64,
    pub keyframe: bool,
    pub last: bool,
    pub data: Vec<u8>,
}

impl FramePacket {
    pub fn encode(&self) -> Vec<u8> {
        let mut bytes = Vec::with_capacity(self.data.len() + 24);
        bytes.extend_from_slice(&self.sequence.to_be_bytes());
        bytes.extend_from_slice(&self.offset.to_be_bytes());
        bytes.extend_from_slice(&self.total.to_be_bytes());
        bytes.push(u8::from(self.keyframe) | (u8::from(self.last) << 1));
        bytes.extend_from_slice(&self.data);
        bytes
    }

    pub fn decode(bytes: &[u8]) -> Result<Self, VideoError> {
        const HEADER: usize = 8 + 8 + 8 + 1;
        if bytes.len() < HEADER {
            return Err(VideoError::Codec("video packet is truncated".to_owned()));
        }
        let sequence = u64::from_be_bytes(bytes[0..8].try_into().expect("length checked"));
        let offset = u64::from_be_bytes(bytes[8..16].try_into().expect("length checked"));
        let total = u64::from_be_bytes(bytes[16..24].try_into().expect("length checked"));
        let flags = bytes[24];
        let data = bytes[HEADER..].to_vec();
        if data.len() > MAX_PACKET_BYTES {
            return Err(VideoError::TooLarge);
        }
        if offset.saturating_add(data.len() as u64) > total {
            return Err(VideoError::Codec(
                "video packet is out of bounds".to_owned(),
            ));
        }
        Ok(Self {
            sequence,
            offset,
            total,
            keyframe: flags & 1 == 1,
            last: flags & 2 == 2,
            data,
        })
    }
}

#[derive(Debug, Default)]
pub struct FramePacketizer {
    assembled: Option<(u64, u64, Vec<u8>)>,
    completed: Option<EncodedFrame>,
}

impl FramePacketizer {
    pub fn packets(&self, frame: &EncodedFrame) -> Vec<FramePacket> {
        let total = frame.data.len() as u64;
        if total == 0 {
            return vec![FramePacket {
                sequence: frame.sequence,
                offset: 0,
                total: 0,
                keyframe: frame.keyframe,
                last: true,
                data: Vec::new(),
            }];
        }
        frame
            .data
            .chunks(MAX_PACKET_BYTES)
            .enumerate()
            .map(|(index, chunk)| {
                let offset = (index * MAX_PACKET_BYTES) as u64;
                let end = offset + chunk.len() as u64;
                FramePacket {
                    sequence: frame.sequence,
                    offset,
                    total,
                    keyframe: frame.keyframe,
                    last: end == total,
                    data: chunk.to_vec(),
                }
            })
            .collect()
    }

    pub fn ingest(&mut self, packet: FramePacket) -> Result<Option<&EncodedFrame>, u8> {
        if packet.keyframe && packet.offset == 0 {
            self.assembled = Some((packet.sequence, packet.total, Vec::new()));
        }
        let Some((sequence, total, data)) = self.assembled.as_mut() else {
            return Ok(None);
        };
        if *sequence != packet.sequence {
            return Ok(None);
        }
        let offset = usize::try_from(packet.offset).map_err(|_| 1)?;
        if data.len() != offset {
            return Err(2);
        }
        data.extend_from_slice(&packet.data);
        if *total == data.len() as u64 && packet.last {
            let (sequence, _, data) = self.assembled.take().expect("present");
            self.completed = Some(EncodedFrame {
                sequence,
                keyframe: packet.keyframe,
                data,
            });
            return Ok(self.completed.as_ref());
        }
        Ok(None)
    }
}
