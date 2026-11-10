//! Native clipboard admission and text/image interface.

use super::copying::ClipboardPlatform;
use futures_util::future::LocalBoxFuture;
use std::io;

/// Native clipboard interface shared by default and supplied adapters.
pub trait ClipboardModule {
    /// Probes image availability, retaining native failures.
    ///
    /// # Errors
    /// Returns the native context or probe failure.
    fn has_image(&self) -> io::Result<bool>;
    /// Completes with owned PNG bytes or absent image data.
    fn get_image_binary(&self) -> LocalBoxFuture<'_, io::Result<Option<Vec<u8>>>>;
    /// Completes after the native write.
    fn set_text<'a>(&'a self, text: &'a str) -> LocalBoxFuture<'a, io::Result<()>>;
}

/// Loads a module only outside Termux and, on Linux, with a display.
pub fn clipboard<T: ClipboardModule>(
    platform: ClipboardPlatform,
    env: impl Fn(&str) -> Option<String>,
    load: impl FnOnce() -> Option<T>,
) -> Option<T> {
    let present = |name| env(name).is_some_and(|value| !value.is_empty());
    if present("TERMUX_VERSION")
        || (platform == ClipboardPlatform::Linux
            && !present("DISPLAY")
            && !present("WAYLAND_DISPLAY"))
    {
        return None;
    }
    load()
}

/// Applies default native binding availability before environment admission.
#[cfg(any(test, target_os = "macos", target_os = "linux", windows))]
fn default_clipboard<T: ClipboardModule>(
    platform: ClipboardPlatform,
    env: impl Fn(&str) -> Option<String>,
    load: impl FnOnce() -> Option<T>,
) -> Option<T> {
    (platform != ClipboardPlatform::Other)
        .then(|| clipboard(platform, env, load))
        .flatten()
}

/// Native library bridge on supported desktop targets.
#[cfg(any(target_os = "macos", target_os = "linux", windows))]
mod native {
    use super::{ClipboardModule, LocalBoxFuture, default_clipboard, io};
    use crate::utils::copying::{ClipboardOperations as _, NativeClipboardOperations, environment};
    use clipboard_rs::{Clipboard as _, ClipboardContext, ContentFormat, common::RustImage as _};

    /// Stateless bridge: each operation owns its context.
    struct NativeClipboard;
    impl ClipboardModule for NativeClipboard {
        fn has_image(&self) -> io::Result<bool> {
            let context = ClipboardContext::new().map_err(io::Error::other)?;
            Ok(context.has(ContentFormat::Image))
        }
        fn get_image_binary(&self) -> LocalBoxFuture<'_, io::Result<Option<Vec<u8>>>> {
            Box::pin(async {
                let context = ClipboardContext::new().map_err(io::Error::other)?;
                let image = context.get_image().map_err(io::Error::other)?;
                let png = image.to_png().map_err(io::Error::other)?;
                Ok(Some(png.get_bytes().to_vec()))
            })
        }
        fn set_text<'a>(&'a self, text: &'a str) -> LocalBoxFuture<'a, io::Result<()>> {
            Box::pin(async move {
                let context = ClipboardContext::new().map_err(io::Error::other)?;
                context.set_text(text.to_owned()).map_err(io::Error::other)
            })
        }
    }

    /// Caches admission, never a live native clipboard context.
    pub(in crate::utils) fn native_clipboard() -> Option<&'static dyn ClipboardModule> {
        /// Once-cached eligibility, not a native clipboard resource.
        static MODULE: std::sync::OnceLock<Option<NativeClipboard>> = std::sync::OnceLock::new();
        MODULE
            .get_or_init(|| {
                default_clipboard(NativeClipboardOperations.platform(), environment, || {
                    Some(NativeClipboard)
                })
            })
            .as_ref()
            .map(|module| module as &dyn ClipboardModule)
    }
}
#[cfg(any(target_os = "macos", target_os = "linux", windows))]
pub(super) use native::native_clipboard;

/// Unsupported hosts retain the helper and terminal routes.
#[cfg(not(any(target_os = "macos", target_os = "linux", windows)))]
pub(super) fn native_clipboard() -> Option<&'static dyn ClipboardModule> {
    None
}

#[cfg(test)]
mod tests;
