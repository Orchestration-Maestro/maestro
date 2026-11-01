//! Native content search observations retain original guest context and diagnostics.
use super::{Call, Handle};
use crate::lifecycle::replies::Reply;
use maestro_extensions::DiagnosticErrorInfo;
use wasmtime::component::{ComponentType, Lift, Lower};

/// Original native content search observation.
#[derive(ComponentType, Lower, Clone, Copy)]
#[component(enum)]
#[repr(u8)]
pub(crate) enum Operation {
    /// Inspect the resolved root.
    #[component(name = "is-directory")]
    IsDirectory,
    /// Read original context text.
    #[component(name = "read-file")]
    ReadFile,
}
/// Original directory or context observation.
#[derive(ComponentType, Lift)]
#[component(variant)]
pub(crate) enum Output {
    /// Whether the root is a directory.
    #[component(name = "directory")]
    Directory(bool),
    /// Original context text.
    #[component(name = "text")]
    Text(String),
}
/// One observation on its original Store admission.
pub(crate) struct Invocation<H = Handle> {
    /// Retained authored effects.
    pub(crate) handler: H,
    /// Selected native observation.
    pub(crate) operation: Operation,
    /// Resolved absolute path.
    pub(crate) path: String,
    /// Original typed observation or structured diagnostic.
    pub(crate) reply: Reply<Result<Output, DiagnosticErrorInfo>>,
}
/// Native thread-safe operations backed by their original guest owner.
pub(crate) struct Operations(pub(crate) Handle);
impl Operations {
    /// Dispatches through the existing lifecycle without a registry borrow.
    fn invoke(&self, operation: Operation, path: String) -> Work<Output> {
        let owner = self.0.clone();
        Box::pin(async move {
            owner
                .endpoint()
                .submit(|reply| {
                    Call::Grep(Invocation {
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
impl maestro_extensions::GrepOperations for Operations {
    fn is_directory(&self, path: String) -> Work<bool> {
        let work = self.invoke(Operation::IsDirectory, path);
        Box::pin(async move {
            match work.await? {
                Output::Directory(directory) => Ok(directory),
                Output::Text(_) => Err(DiagnosticErrorInfo::error(
                    "grep operation result kind mismatch",
                )),
            }
        })
    }
    fn read_file(&self, path: String) -> Work<String> {
        let work = self.invoke(Operation::ReadFile, path);
        Box::pin(async move {
            match work.await? {
                Output::Text(text) => Ok(text),
                Output::Directory(_) => Err(DiagnosticErrorInfo::error(
                    "grep operation result kind mismatch",
                )),
            }
        })
    }
}
/// Thread-safe future consumed by the existing native searcher.
type Work<T> =
    std::pin::Pin<Box<dyn std::future::Future<Output = Result<T, DiagnosticErrorInfo>> + Send>>;
