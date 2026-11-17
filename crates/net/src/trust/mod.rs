pub mod endpoint;
pub mod frames;
pub mod spake2;

pub use endpoint::{
    EndpointError, InitiatorRequest, ResponderRequest, TrustOutcome, TrustUser, run_initiator,
    run_responder,
};
pub use frames::{
    FrameError, TrustMessage, decode_frame, decode_message, encode_frame, encode_message,
};
pub use spake2::{
    Confirmation, Role, Spake2Error, Spake2Session, TrustSecret, TrustTranscript, UserIdentity,
};

pub const TRUST_ALPN: &[u8] = b"ticonnect-trust/1";
pub const TRUST_TIMEOUT_SECS: u64 = 120;
