#![no_main]

use libfuzzer_sys::fuzz_target;
use ticonnect_core::wire::{Frame, HandshakeFrame};

fuzz_target!(|data: &[u8]| {
    let _ = Frame::decode(data);
    let _ = HandshakeFrame::decode(data);
});
