use openh264::encoder::{BitRate, Encoder, EncoderConfig, FrameRate, FrameType, IntraFramePeriod};
use openh264::formats::YUVSource;

use super::{EncodedFrame, VideoConfig, VideoError};

pub struct H264Encoder {
    encoder: Encoder,
    config: VideoConfig,
    sequence: u64,
    force_keyframe: bool,
}

impl H264Encoder {
    pub fn new(config: VideoConfig) -> Result<Self, VideoError> {
        config.validate()?;
        let encoder_config = EncoderConfig::new()
            .bitrate(BitRate::from_bps(config.bitrate_kbps * 1000))
            .max_frame_rate(FrameRate::from_hz(config.fps as f32))
            .intra_frame_period(IntraFramePeriod::from_num_frames(config.keyframe_interval));
        let encoder =
            Encoder::with_api_config(openh264::OpenH264API::from_source(), encoder_config)
                .map_err(|error| VideoError::Codec(error.to_string()))?;
        Ok(Self {
            encoder,
            config,
            sequence: 0,
            force_keyframe: false,
        })
    }

    pub fn config(&self) -> VideoConfig {
        self.config
    }

    pub fn request_keyframe(&mut self) {
        self.force_keyframe = true;
    }

    pub fn encode<Y: YUVSource>(&mut self, source: &Y) -> Result<EncodedFrame, VideoError> {
        if self.force_keyframe {
            self.encoder.force_intra_frame();
            self.force_keyframe = false;
        }
        let bitstream = self
            .encoder
            .encode(source)
            .map_err(|error| VideoError::Codec(error.to_string()))?;
        let data = bitstream.to_vec();
        if data.len() > super::MAX_FRAME_BYTES {
            return Err(VideoError::TooLarge);
        }
        let frame = EncodedFrame {
            sequence: self.sequence,
            keyframe: matches!(bitstream.frame_type(), FrameType::IDR | FrameType::I),
            data,
        };
        self.sequence = self.sequence.saturating_add(1);
        Ok(frame)
    }

    pub fn next_sequence(&self) -> u64 {
        self.sequence
    }
}
