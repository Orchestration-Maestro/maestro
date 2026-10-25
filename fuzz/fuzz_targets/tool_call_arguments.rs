//! Decodes streamed tool-call argument text.
#![no_main]

use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    if let Ok(partial) = std::str::from_utf8(data) {
        std::hint::black_box(maestro_models::parsed_arguments(partial));
    }
});
