#![no_main]

use libfuzzer_sys::fuzz_target;
use ticonnect_core::message::MessageRecord;
use ticonnect_core::wire::decode_sync_frame;

fuzz_target!(|data: &[u8]| {
    let _ = MessageRecord::from_envelope(data);
    let _ = decode_sync_frame(data);
});
