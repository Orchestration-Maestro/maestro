//! Native find observations return through their original guest callback owner.
use super::{Call, Handle};
use crate::lifecycle::replies::Reply;
use maestro_extensions::{DiagnosticErrorInfo, FindGlobOptions};
use wasmtime::component::{ComponentType, Lift, Lower};

/// Original glob constraints with no JSON number conversion.
#[derive(ComponentType, Lower, Clone)]
#[component(record)]
pub(crate) struct Options {
    /// Ordered native ignore patterns.
    pub(crate) ignore: Vec<String>,
    /// Original effective limit.
    pub(crate) limit: f64,
}
/// Glob operands beside the separately supplied root.
#[derive(ComponentType, Lower, Clone)]
#[component(record)]
pub(crate) struct Glob {
    /// Original pattern.
    pub(crate) pattern: String,
    /// Native glob constraints.
    pub(crate) options: Options,
}
/// Selected native observation.
#[derive(ComponentType, Lower, Clone)]
#[component(variant)]
pub(crate) enum Operation {
    /// Observe root existence.
    #[component(name = "exists")]
    Exists,
    /// Find ordered paths.
    #[component(name = "glob")]
    Glob(Glob),
}
/// Typed original callback result.
#[derive(ComponentType, Lift)]
#[component(variant)]
pub(crate) enum Output {
    /// Existence observation.
    #[component(name = "exists")]
    Exists(bool),
    /// Original relative or absolute paths.
    #[component(name = "paths")]
    Paths(Vec<String>),
}
/// One observation on its original Store admission.
pub(crate) struct Invocation<H = Handle> {
    /// Retained authored operations.
    pub(crate) handler: H,
    /// Selected native operation.
    pub(crate) operation: Operation,
    /// Resolved absolute root.
    pub(crate) path: String,
    /// Original observation or structured diagnostic.
    pub(crate) reply: Reply<Result<Output, DiagnosticErrorInfo>>,
}
/// Native thread-safe operations retaining their original guest owner.
pub(crate) struct Operations(pub(crate) Handle);
impl Operations {
    /// Dispatches without retaining a callback-table borrow.
    fn invoke(&self, operation: Operation, path: String) -> Work<Output> {
        let owner = self.0.clone();
        Box::pin(async move {
            owner
                .endpoint()
                .submit(|reply| {
                    Call::Find(Invocation {
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
impl maestro_extensions::FindOperations for Operations {
    fn exists(&self, path: String) -> Work<bool> {
        let work = self.invoke(Operation::Exists, path);
        Box::pin(async move {
            match work.await? {
                Output::Exists(exists) => Ok(exists),
                Output::Paths(_) => Err(DiagnosticErrorInfo::error(
                    "find operation result kind mismatch",
                )),
            }
        })
    }
    fn glob(&self, pattern: String, cwd: String, options: FindGlobOptions) -> Work<Vec<String>> {
        let work = self.invoke(
            Operation::Glob(Glob {
                pattern,
                options: Options {
                    ignore: options.ignore,
                    limit: options.limit,
                },
            }),
            cwd,
        );
        Box::pin(async move {
            match work.await? {
                Output::Paths(paths) => Ok(paths),
                Output::Exists(_) => Err(DiagnosticErrorInfo::error(
                    "find operation result kind mismatch",
                )),
            }
        })
    }
}
/// Thread-safe future consumed by the original native finder.
type Work<T> =
    std::pin::Pin<Box<dyn std::future::Future<Output = Result<T, DiagnosticErrorInfo>> + Send>>;
