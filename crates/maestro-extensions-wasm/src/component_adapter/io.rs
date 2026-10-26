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
    fn spawn(
        &self,
        command: String,
        args: Vec<String>,
        options: crate::child_process::SpawnOptions,
    ) -> ExtensionFuture<'_, std::rc::Rc<dyn crate::child_process::ChildProcess>> {
        Box::pin(async move {
            let child = self.0.spawn(command, args, options).await?;
            Ok(
                std::rc::Rc::new(super::byte_resources::Child::new(&self.0, child))
                    as std::rc::Rc<dyn crate::child_process::ChildProcess>,
            )
        })
    }
    fn kill(&self, pid: i32, signal: Option<String>) -> crate::ExtensionResult<()> {
        self.0.kill(pid, signal)
    }

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
