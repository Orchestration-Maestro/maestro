//! Native queue forwarding; result values never leave the guest.
#![forbid(
    clippy::pedantic,
    clippy::too_many_arguments,
    clippy::excessive_nesting
)]
use crate::{ExtensionFuture, ExtensionHost};
use std::{cell::RefCell, rc::Rc};

/// Keeps the generic value local while the host retains the native queue future.
pub(crate) fn run<T: 'static>(
    host: &dyn ExtensionHost,
    file_path: String,
    operation: impl FnOnce() -> ExtensionFuture<'static, T> + 'static,
) -> ExtensionFuture<'static, T> {
    let result = Rc::new(RefCell::new(None));
    let returned = result.clone();
    let work = host.with_file_mutation_queue(
        file_path,
        Box::new(move || {
            Box::pin(async move {
                let value = operation().await?;
                *returned.borrow_mut() = Some(value);
                Ok(())
            })
        }),
    );
    Box::pin(async move {
        work.await?;
        result
            .borrow_mut()
            .take()
            .ok_or_else(|| "file mutation callback returned no value".into())
    })
}
