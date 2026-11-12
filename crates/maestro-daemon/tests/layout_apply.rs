//! Layout apply through the daemon's handler with live processes: replacement, launch inputs,
//! directories, limits, rollback, focus and publication.
#![cfg(target_os = "linux")]
use maestro_daemon::state::test_support::layout_apply::relay_hangup;
use maestro_daemon::state::test_support::run;
use maestro_test_directory::Scratch;
use std::io::{self};
use std::os::unix::net::UnixStream;
/// A peer that closes before announcing returns its I/O error to the layout operation.
#[test]
fn hangup_relay_reports_closed_stream() {
    let scratch = Scratch::new().unwrap();
    let socket = scratch.path().join("hangup");
    let error = run(relay_hangup(&socket, async {
        drop(UnixStream::connect(&socket).unwrap());
    }))
    .unwrap()
    .unwrap_err();
    assert_eq!(error.kind(), io::ErrorKind::UnexpectedEof);
}
