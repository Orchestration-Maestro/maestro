//! Decode untrusted browser resource routes without filesystem access.
#![no_main]

use libfuzzer_sys::fuzz_target;
use maestro_tooling::browser_driver::resource_path;

fuzz_target!(|data: &str| {
    let _ = resource_path(data);
});
