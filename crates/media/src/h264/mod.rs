pub const DEFAULT_WIDTH: u32 = 1920;
pub const DEFAULT_HEIGHT: u32 = 1080;
pub const DEFAULT_FPS: u32 = 30;
pub const MAX_FRAME_BYTES: usize = 1024 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VideoConfig {
    pub width: u32,
    pub height: u32,
    pub fps: u32,
    pub bitrate_kbps: u32,
    pub keyframe_interval: u32,
}

impl Default for VideoConfig {
    fn default() -> Self {
        Self {
            width: DEFAULT_WIDTH,
            height: DEFAULT_HEIGHT,
            fps: DEFAULT_FPS,
            bitrate_kbps: 2500,
            keyframe_interval: 60,
        }
    }
}

impl VideoConfig {
    pub fn validate(&self) -> Result<(), VideoError> {
        if self.width == 0 || self.height == 0 || self.width > 1920 || self.height > 1080 {
            return Err(VideoError::InvalidConfig(
                "dimensions must be between 1x1 and 1920x1080".to_owned(),
            ));
        }
        if self.fps == 0 || self.fps > 60 {
            return Err(VideoError::InvalidConfig(
                "frame rate must be between 1 and 60".to_owned(),
            ));
        }
        if self.bitrate_kbps == 0 || self.bitrate_kbps > 20_000 {
            return Err(VideoError::InvalidConfig(
                "bitrate must be between 1 and 20000 kbps".to_owned(),
            ));
        }
        if self.keyframe_interval == 0 || self.keyframe_interval > 600 {
            return Err(VideoError::InvalidConfig(
                "keyframe interval must be between 1 and 600".to_owned(),
            ));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct VideoMetrics {
    pub frames_encoded: u64,
    pub frames_decoded: u64,
    pub frames_dropped: u64,
    pub keyframes: u64,
    pub bytes_sent: u64,
    pub bytes_received: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EncodedFrame {
    pub sequence: u64,
    pub keyframe: bool,
    pub data: Vec<u8>,
}

#[derive(Debug, thiserror::Error)]
pub enum VideoError {
    #[error("video configuration is invalid: {0}")]
    InvalidConfig(String),

    #[error("H.264 codec failed: {0}")]
    Codec(String),

    #[error("video frame exceeds the maximum size")]
    TooLarge,
}

pub mod decoder;
pub mod encoder;
pub mod packetize;

pub use decoder::H264Decoder;
pub use encoder::H264Encoder;
pub use packetize::{FramePacket, FramePacketizer};

#[cfg(test)]
mod tests {
    use super::*;
    use openh264::formats::YUVBuffer;

    fn frame(width: usize, height: usize, value: u8) -> YUVBuffer {
        YUVBuffer::from_vec(vec![value; (3 * width * height) / 2], width, height)
    }

    #[test]
    fn encoder_decoder_round_trip() {
        let config = VideoConfig {
            width: 320,
            height: 240,
            fps: 30,
            bitrate_kbps: 500,
            keyframe_interval: 30,
        };
        let mut encoder = H264Encoder::new(config).expect("encoder");
        let mut decoder = H264Decoder::new(config).expect("decoder");
        let encoded = encoder.encode(&frame(320, 240, 120)).expect("encode");
        assert!(!encoded.data.is_empty());
        assert!(encoded.keyframe);
        let decoded = decoder.decode(&encoded.data).expect("decode");
        assert!(decoded.is_some());
    }

    #[test]
    fn packetizer_splits_and_reassembles() {
        use super::packetize::MAX_PACKET_BYTES;
        let frame = EncodedFrame {
            sequence: 1,
            keyframe: true,
            data: vec![9_u8; MAX_PACKET_BYTES + 10],
        };
        let packetizer = FramePacketizer::default();
        let packets = packetizer.packets(&frame);
        assert_eq!(packets.len(), 2);
        for packet in &packets {
            assert_eq!(
                FramePacket::decode(&packet.encode()).expect("decode"),
                *packet
            );
        }
        let mut assembler = FramePacketizer::default();
        let mut completed = None;
        for packet in packets {
            if let Ok(Some(result)) = assembler.ingest(packet) {
                completed = Some(result.clone());
            }
        }
        assert_eq!(completed.expect("frame"), frame);
    }

    #[test]
    fn rejects_invalid_configuration() {
        let config = VideoConfig {
            width: 4096,
            ..VideoConfig::default()
        };
        assert!(config.validate().is_err());
        assert!(H264Encoder::new(config).is_err());
    }
}
