//! The result and future types every author callback returns.
use crate::DiagnosticErrorInfo;
use std::future::Future;
use std::pin::Pin;

/// Result of an operation that fails with the message the host supplied.
pub type ExtensionResult<T> = Result<T, String>;

/// Future of a fallible operation, local to the extension instance.
pub type ExtensionFuture<'a, T> = Pin<Box<dyn Future<Output = ExtensionResult<T>> + 'a>>;

/// Result of an event hook that fails with the diagnostic fields its author supplied.
pub type HookResult<T> = Result<T, DiagnosticErrorInfo>;

/// Future of a fallible event hook, local to the extension instance.
pub type HookFuture<'a, T> = Pin<Box<dyn Future<Output = HookResult<T>> + 'a>>;
