//! Native operations forwarded without introducing a guest clock.
#![forbid(
    clippy::pedantic,
    clippy::too_many_arguments,
    clippy::excessive_nesting
)]
use super::{Imports, signals::Mirror};
use crate::{AbortSignal, ExtensionFuture, io::NativeIo};
use std::time::Duration;

/// Independently retained import capability.
pub(super) struct Io<I: Imports>(pub(super) I);

impl<I: Imports> NativeIo for Io<I> {
    fn sleep(&self, duration: Duration, signal: Option<AbortSignal>) -> ExtensionFuture<'_, ()> {
        Box::pin(async move {
            let mirror = Mirror::new(&self.0, signal.as_ref())?;
            let result = self
                .0
                .sleep(
                    (duration.as_secs(), duration.subsec_nanos()),
                    mirror.signal(),
                )
                .await;
            mirror.settle(result)
        })
    }
}
