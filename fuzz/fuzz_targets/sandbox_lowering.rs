//! Compile untrusted asynchronous bodies and supplied provider expressions.
#![no_main]

use libfuzzer_sys::fuzz_target;
use maestro_web::components::sandboxed_iframe::lowering;

fuzz_target!(|source: &str| {
    let _ = lowering::program(source);
    let _ = lowering::provider(source);
});
