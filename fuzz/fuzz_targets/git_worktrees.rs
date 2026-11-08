//! Exercise newline-framed Git worktree records with arbitrary bytes.
#![no_main]

use libfuzzer_sys::fuzz_target;
use maestro_daemon::git::checkout::ExistingWorktree;

fuzz_target!(|data: &[u8]| {
    let _ = ExistingWorktree::parse_porcelain(data);
});
