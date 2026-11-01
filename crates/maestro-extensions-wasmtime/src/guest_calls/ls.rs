//! Directory observations return through the original retained guest callback owner.
use super::{Call, Handle};
use crate::lifecycle::replies::Reply;
use maestro_extensions::DiagnosticErrorInfo;
use wasmtime::component::{ComponentType, Lift, Lower};

/// Selected directory observation, without path or sorting policy.
#[derive(ComponentType, Lower, Clone, Copy)]
#[component(enum)]
#[repr(u8)]
pub(crate) enum Operation {
    /// Observe root existence.
    #[component(name = "exists")]
    Exists,
    /// Observe whether the root or entry is a directory.
    #[component(name = "stat")]
    Stat,
    /// Read original entry names.
    #[component(name = "readdir")]
    Readdir,
}
/// Original callback observation.
#[derive(ComponentType, Lift)]
#[component(variant)]
pub(crate) enum Output {
    /// Existence or directory observation.
    #[component(name = "boolean")]
    Boolean(bool),
    /// Original entry names in producer order.
    #[component(name = "names")]
    Names(Vec<String>),
}
/// One observation on the existing Store admission.
pub(crate) struct Invocation<H = Handle> {
    /// Original retained operations.
    pub(crate) handler: H,
    /// Selected observation.
    pub(crate) operation: Operation,
    /// Native resolved absolute path.
    pub(crate) path: String,
    /// Typed observation or structured diagnostic.
    pub(crate) reply: Reply<Result<Output, DiagnosticErrorInfo>>,
}
/// Thread-safe native operations retaining their original guest owner.
pub(crate) struct Operations(pub(crate) Handle);
impl Operations {
    /// Dispatches through the existing callback lifecycle outside table borrows.
    fn invoke(&self, operation: Operation, path: String) -> Work<Output> {
        let owner = self.0.clone();
        Box::pin(async move {
            owner
                .endpoint()
                .submit(|reply| {
                    Call::Ls(Invocation {
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
    /// Projects the native existence/stat observation without synthesizing filesystem metadata.
    fn boolean(&self, operation: Operation, path: String) -> Work<bool> {
        let work = self.invoke(operation, path);
        Box::pin(async move {
            match work.await? {
                Output::Boolean(value) => Ok(value),
                Output::Names(_) => Err(DiagnosticErrorInfo::error(
                    "ls operation result kind mismatch",
                )),
            }
        })
    }
}
impl maestro_extensions::LsOperations for Operations {
    fn exists(&self, path: String) -> Work<bool> {
        self.boolean(Operation::Exists, path)
    }
    fn stat(&self, path: String) -> Work<bool> {
        self.boolean(Operation::Stat, path)
    }
    fn readdir(&self, path: String) -> Work<Vec<String>> {
        let work = self.invoke(Operation::Readdir, path);
        Box::pin(async move {
            match work.await? {
                Output::Names(names) => Ok(names),
                Output::Boolean(_) => Err(DiagnosticErrorInfo::error(
                    "ls operation result kind mismatch",
                )),
            }
        })
    }
}
/// Thread-safe future consumed by the original native directory lister.
type Work<T> =
    std::pin::Pin<Box<dyn std::future::Future<Output = Result<T, DiagnosticErrorInfo>> + Send>>;
