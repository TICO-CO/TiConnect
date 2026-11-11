use serde::{Deserialize, Serialize};
use thiserror::Error;
use ticonnect_core::wire::{Frame, FrameKind, MAX_PAIRING_FRAME};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum PairingMessage {
    Hello {
        node_id: String,
        fingerprint: String,
        certificate_fingerprint: String,
        mode: u8,
    },
    Spake2 {
        message: Vec<u8>,
    },
    Confirm {
        value: [u8; 32],
    },
    Complete,
    Error {
        code: u8,
    },
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum FrameError {
    #[error("pairing message exceeds {MAX_PAIRING_FRAME} bytes")]
    TooLarge,

    #[error("pairing message could not be encoded: {0}")]
    Encode(String),

    #[error("pairing message could not be decoded: {0}")]
    Decode(String),

    #[error("wire frame is not a pairing frame")]
    WrongKind,
}

pub fn encode_message(message: &PairingMessage) -> Result<Vec<u8>, FrameError> {
    let bytes =
        postcard::to_allocvec(message).map_err(|error| FrameError::Encode(error.to_string()))?;
    if bytes.len() > MAX_PAIRING_FRAME {
        return Err(FrameError::TooLarge);
    }
    Ok(bytes)
}

pub fn decode_message(bytes: &[u8]) -> Result<PairingMessage, FrameError> {
    if bytes.len() > MAX_PAIRING_FRAME {
        return Err(FrameError::TooLarge);
    }
    postcard::from_bytes(bytes).map_err(|error| FrameError::Decode(error.to_string()))
}

pub fn encode_frame(message: &PairingMessage) -> Result<Frame, FrameError> {
    Frame::new(FrameKind::Pairing, encode_message(message)?)
        .map_err(|error| FrameError::Encode(error.to_string()))
}

pub fn decode_frame(frame: &Frame) -> Result<PairingMessage, FrameError> {
    if frame.kind() != FrameKind::Pairing {
        return Err(FrameError::WrongKind);
    }
    decode_message(frame.payload())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pairing_messages_round_trip() {
        let messages = [
            PairingMessage::Hello {
                node_id: "018f0000-0000-7000-8000-000000000001".to_owned(),
                fingerprint: "11".repeat(32),
                certificate_fingerprint: "22".repeat(32),
                mode: 1,
            },
            PairingMessage::Spake2 {
                message: vec![7_u8; 33],
            },
            PairingMessage::Confirm { value: [3_u8; 32] },
            PairingMessage::Complete,
            PairingMessage::Error { code: 4 },
        ];

        for message in messages {
            let bytes = encode_message(&message).expect("encode");
            assert_eq!(decode_message(&bytes).expect("decode"), message);

            let frame = encode_frame(&message).expect("frame");
            assert_eq!(decode_frame(&frame).expect("decode frame"), message);
        }
    }

    #[test]
    fn pairing_decoder_rejects_malformed_bytes_without_panicking() {
        for length in 0..=MAX_PAIRING_FRAME.min(256) {
            let bytes = vec![0xa5_u8; length];
            let _ = decode_message(&bytes);
        }

        let mut state = 0x1234_5678_u32;
        for _ in 0..512 {
            state = state.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
            let length = usize::try_from(state % 512).unwrap_or_default();
            let bytes: Vec<u8> = (0..length)
                .map(|index| {
                    state = state.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
                    (state ^ index as u32) as u8
                })
                .collect();
            let _ = decode_message(&bytes);
        }
    }

    #[test]
    fn rejects_wrong_frame_kind_and_oversized_payload() {
        let frame = Frame::new(FrameKind::Handshake, Vec::new()).expect("frame");
        assert_eq!(decode_frame(&frame), Err(FrameError::WrongKind));

        assert_eq!(
            decode_message(&vec![0_u8; MAX_PAIRING_FRAME + 1]),
            Err(FrameError::TooLarge)
        );
    }
}
