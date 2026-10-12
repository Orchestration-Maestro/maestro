//! Private result mapping.
use super::project;
use std::io;

#[test]
fn wait_failure_preserves_collected_streams() {
    let result = project(
        Err(io::Error::other("wait failed")),
        (b"out\xff".to_vec(), b"err".to_vec()),
        true,
    );
    assert_eq!(
        (
            result.stdout.as_str(),
            result.stderr.as_str(),
            result.code,
            result.killed
        ),
        ("out\u{fffd}", "err", 1, true)
    );
}
