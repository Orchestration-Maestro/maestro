//! Invocation of returned shell operations using owned output and cancellation resources.
use super::{Exports, Imports, signals};
use crate::bindings::exports::maestro::extension::guest::BashRequest;
use crate::{BashExecOptions, BashExecResult, BashOperations, ExtensionResult};
use std::rc::Rc;
/// Host resources retained by one execution.
pub(crate) struct BashCapabilities<I: Imports> {
    /// Output receiver.
    pub(crate) output: I::BashOutput,
    /// Optional cancellation owner.
    pub(crate) signal: Option<I::Signal>,
}
impl<I: Imports> Exports<I> {
    /// Executes an authored operation without imposing process or timer policy.
    pub(crate) async fn invoke_bash(
        &self,
        operations: &BashOperations,
        request: BashRequest,
        resources: BashCapabilities<I>,
    ) -> ExtensionResult<BashExecResult> {
        let imports = self.imports.clone();
        let options = BashExecOptions {
            on_data: Rc::new(move |bytes| imports.bash_output(&resources.output, bytes)),
            signal: resources
                .signal
                .map(|signal| signals::signal(&self.imports, signal)),
            timeout: request.timeout,
            env: request.env.map(|env| env.into_iter().collect()),
        };
        (operations.exec)(request.command, request.cwd, options).await
    }
}
