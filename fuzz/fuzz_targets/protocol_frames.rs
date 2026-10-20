//! Decodes consecutive workspace protocol frames in both directions.
#![no_main]

use libfuzzer_sys::fuzz_target;
use maestro_protocol::frames::{
    ClientMessage, MAX_FRAME_SIZE, MAX_GRAPHICS_FRAME_SIZE, ServerMessage, read_message,
};

fuzz_target!(|data: &[u8]| {
    let mut daemon = data;
    while read_message::<_, ClientMessage>(&mut daemon, MAX_FRAME_SIZE).is_ok() {}
    let mut client = data;
    while read_message::<_, ServerMessage>(&mut client, MAX_GRAPHICS_FRAME_SIZE).is_ok() {}
});
