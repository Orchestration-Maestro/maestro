//! A native watch registration ends with its capability.
use super::Pump;
use crate::{
    local_resources::{Capability, watches::Watcher},
    tests::ended_errors,
};
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
    let (pump, admission) = Pump::new(crate::tests::support::widgets());
    let lease = pump
        .registry()
        .insert(Capability::NativeWatcher(Rc::new(Quiet)))
        .unwrap();
    let watcher = Watcher(lease.clone());
    let errors = ended_errors(lease, pump, admission, move || {
        [watcher.close().unwrap_err()]
    });
    assert_eq!(errors, ["native watch resource ended"]);
}
