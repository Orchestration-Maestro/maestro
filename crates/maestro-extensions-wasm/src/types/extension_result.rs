//! The result and future types every author callback returns.
use std::future::Future;
use std::pin::Pin;

/// Result of an operation that fails with the message the host supplied.
pub type ExtensionResult<T> = Result<T, String>;

/// Future of a fallible operation, local to the extension instance.
pub type ExtensionFuture<'a, T> = Pin<Box<dyn Future<Output = ExtensionResult<T>> + 'a>>;
