use openh264::decoder::Decoder;

use super::{VideoConfig, VideoError};

pub struct H264Decoder {
    decoder: Decoder,
    decoded: u64,
}

impl H264Decoder {
    pub fn new(_config: VideoConfig) -> Result<Self, VideoError> {
        Ok(Self {
            decoder: Decoder::new().map_err(|error| VideoError::Codec(error.to_string()))?,
            decoded: 0,
        })
    }

    pub fn decode(
        &mut self,
        data: &[u8],
    ) -> Result<Option<openh264::decoder::DecodedYUV<'_>>, VideoError> {
        let decoded = self
            .decoder
            .decode(data)
            .map_err(|error| VideoError::Codec(error.to_string()))?;
        if decoded.is_some() {
            self.decoded = self.decoded.saturating_add(1);
        }
        Ok(decoded)
    }

    pub fn decoded_frames(&self) -> u64 {
        self.decoded
    }
}
