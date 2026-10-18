//! Caller-supplied synchronous shell operation adapter for boundary tests.
#![cfg(test)]
use maestro_tools::{BashError, BashExecOptions, BashExecResult, BashOperations};
use std::{future::Future, pin::Pin, sync::Arc};
/// Immediate supplied operation for non-scripted boundary witnesses.
pub struct Immediate(pub Arc<ImmediateCallback>);
/// Synchronous body of a supplied test operation.
pub type ImmediateCallback =
    dyn Fn(&str, &str, BashExecOptions<'_>) -> Result<BashExecResult, BashError> + Send + Sync;
impl BashOperations for Immediate {
    fn exec<'a>(
        &'a self,
        command: &'a str,
        cwd: &'a str,
        options: BashExecOptions<'a>,
    ) -> Pin<Box<dyn Future<Output = Result<BashExecResult, BashError>> + Send + 'a>> {
        Box::pin(async move { (self.0)(command, cwd, options) })
    }
}
