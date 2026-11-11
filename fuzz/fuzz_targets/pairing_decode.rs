#![no_main]

use libfuzzer_sys::fuzz_target;
use ticonnect_net::pairing::decode_message;

fuzz_target!(|data: &[u8]| {
    let _ = decode_message(data);
});
