//! A native watch registration ends with its capability.
use super::ended_endpoint;
use crate::local_resources::{Capability, watches::Watcher};
use maestro_extensions::native_io::FsWatcher;
use std::rc::Rc;

/// A registration that closes without effect.
struct Quiet;

impl FsWatcher for Quiet {
    fn close(&self) -> Result<(), String> {
        Ok(())
    }
}

#[test]
fn released_watch_rejects_with_its_ended_text() {
    let errors = ended_endpoint(
        Capability::NativeWatcher(Rc::new(Quiet)),
        Watcher,
        |watcher| [watcher.close().unwrap_err()],
    );
    assert_eq!(errors, ["native watch resource ended"]);
}
