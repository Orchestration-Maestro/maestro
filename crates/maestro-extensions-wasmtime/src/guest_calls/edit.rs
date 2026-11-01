//! Native edit effects retain the guest's structured diagnostics until their consumer.
use super::{Call, Handle, read::Output};
use crate::lifecycle::replies::Reply;
use maestro_extensions::DiagnosticErrorInfo;
use wasmtime::component::{ComponentType, Lower};

/// One original native edit effect.
#[derive(ComponentType, Lower, Clone)]
#[component(variant)]
pub(crate) enum Operation {
    /// Read original file bytes.
    #[component(name = "read-file")]
    ReadFile,
    /// Write the native replacement text.
    #[component(name = "write-file")]
    WriteFile(String),
    /// Check read and write access.
    #[component(name = "access")]
    Access,
}
/// An edit effect on its original guest owner and Store admission.
pub(crate) struct Invocation<H = Handle> {
    /// Retained original effect owner.
    pub(crate) handler: H,
    /// Requested native effect.
    pub(crate) operation: Operation,
    /// Resolved absolute target.
    pub(crate) path: String,
    /// Bytes or completion, preserving original diagnostic fields on failure.
    pub(crate) reply: Reply<Result<Output, DiagnosticErrorInfo>>,
}
/// Native thread-safe edit interface backed by its original guest callbacks.
pub(crate) struct Operations(pub(crate) Handle);
impl Operations {
    /// Dispatches through the existing Store lifecycle without a registry borrow.
    fn invoke(&self, operation: Operation, path: String) -> Work<Output> {
        let owner = self.0.clone();
        Box::pin(async move {
            owner
                .endpoint()
                .submit(|reply| {
                    Call::Edit(Invocation {
                        handler: owner.clone(),
                        operation,
                        path,
                        reply,
                    })
                })
                .await
                .map_err(DiagnosticErrorInfo::error)?
        })
    }
    /// Accepts only the successful unit response expected by access and write.
    fn unit(&self, operation: Operation, path: String) -> Work<()> {
        let result = self.invoke(operation, path);
        Box::pin(async move {
            match result.await? {
                Output::Unit => Ok(()),
                _ => Err(DiagnosticErrorInfo::error(
                    "edit operation result kind mismatch",
                )),
            }
        })
    }
}
impl maestro_extensions::EditOperations for Operations {
    fn access(&self, path: String) -> Work<()> {
        self.unit(Operation::Access, path)
    }
    fn write_file(&self, path: String, content: String) -> Work<()> {
        self.unit(Operation::WriteFile(content), path)
    }
    fn read_file(&self, path: String) -> Work<Vec<u8>> {
        let result = self.invoke(Operation::ReadFile, path);
        Box::pin(async move {
            match result.await? {
                Output::Bytes(bytes) => Ok(bytes),
                _ => Err(DiagnosticErrorInfo::error(
                    "edit operation result kind mismatch",
                )),
            }
        })
    }
}
/// Thread-safe future required by the existing native edit owner.
type Work<T> =
    std::pin::Pin<Box<dyn std::future::Future<Output = Result<T, DiagnosticErrorInfo>> + Send>>;
