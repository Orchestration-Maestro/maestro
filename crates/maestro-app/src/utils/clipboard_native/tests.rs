//! Default admission without operating-system clipboard access.

use super::{ClipboardModule, ClipboardPlatform, LocalBoxFuture, default_clipboard, io};
use std::cell::Cell;

/// Admission marker that never opens a native clipboard.
struct Module;
impl ClipboardModule for Module {
    fn set_text<'a>(&'a self, _: &'a str) -> LocalBoxFuture<'a, io::Result<()>> {
        unreachable!("admission must not write")
    }
}

#[test]
fn default_native_admission_excludes_other_hosts_but_keeps_linux() {
    for (platform, admitted) in [
        (ClipboardPlatform::Other, false),
        (ClipboardPlatform::Linux, true),
        (ClipboardPlatform::Darwin, true),
        (ClipboardPlatform::Win32, true),
    ] {
        let loaded = Cell::new(false);
        let module = default_clipboard(
            platform,
            |name| (name == "DISPLAY").then(|| ":0".to_owned()),
            || {
                loaded.set(true);
                Some(Module)
            },
        );
        assert_eq!(module.is_some(), admitted, "{platform:?}");
        assert_eq!(loaded.get(), admitted, "{platform:?} loader");
    }
}
