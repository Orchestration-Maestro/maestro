//! Native writes call retained guest effects without converting their diagnostics to text.
use super::{Call, Handle};
use crate::lifecycle::replies::Reply;
use maestro_extensions::DiagnosticErrorInfo;
use wasmtime::component::{ComponentType, Lower};

/// One native write effect, with content only when replacing a file.
#[derive(ComponentType, Lower, Clone)]
#[component(variant)]
pub(crate) enum Operation {
    /// Replace a file with original UTF-8 text.
    #[component(name = "write-file")]
    WriteFile(String),
    /// Create parents recursively.
    #[component(name = "mkdir")]
    Mkdir,
}
/// Original write operands and the owning native completion.
pub(crate) struct Invocation<H = Handle> {
    /// Original guest effect owner.
    pub(crate) handler: H,
    /// Requested native effect.
    pub(crate) operation: Operation,
    /// Resolved absolute target.
    pub(crate) path: String,
    /// Original diagnostic fields, independent of Store faults.
    pub(crate) reply: Reply<Result<(), DiagnosticErrorInfo>>,
}
/// The existing native thread-safe effect interface, backed by its original guest owner.
pub(crate) struct Operations(pub(crate) Handle);
impl Operations {
    /// Retains the effect owner through its actual callback settlement.
    fn invoke(&self, operation: Operation, path: String) -> Work {
        let owner = self.0.clone();
        Box::pin(async move {
            owner
                .endpoint()
                .submit(|reply| {
                    Call::Write(Invocation {
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
}
impl maestro_extensions::WriteOperations for Operations {
    fn mkdir(&self, path: String) -> Work {
        self.invoke(Operation::Mkdir, path)
    }
    fn write_file(&self, path: String, content: String) -> Work {
        self.invoke(Operation::WriteFile(content), path)
    }
}
/// Thread-safe future required by the native write owner.
type Work =
    std::pin::Pin<Box<dyn std::future::Future<Output = Result<(), DiagnosticErrorInfo>> + Send>>;
