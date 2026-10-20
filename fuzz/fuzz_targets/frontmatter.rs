//! Parses skill and prompt metadata headers.
#![no_main]

use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    if let Ok(content) = std::str::from_utf8(data) {
        let _ = std::hint::black_box(maestro_resources::parse_frontmatter(content));
    }
});
