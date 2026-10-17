//! Hidden execution settlement on the retained frame owner.
use super::super::execution::ExecutionState;
use super::{
    BrowserSandboxValue, DiagnosticErrorInfo, MessageConsumer, Owner, Provider, SandboxIframe,
    SharedHandle, diagnostic,
};
use crate::browser::timer::{self, Timer};
use maestro_app::browser::components::{
    sandbox::console_runtime_provider::ConsoleRuntimeProvider,
    sandboxed_iframe::{SandboxError, SandboxExecution},
};
use maestro_app::{BoxFuture, Cancellation, SandboxResult};
use std::{
    cell::RefCell,
    future::{Future as _, poll_fn},
    pin::Pin,
    rc::{Rc, Weak},
    task::Poll,
};
use tokio::sync::oneshot;

/// Native SDK result retaining file content handles.
type NativeResult = SandboxResult<BrowserSandboxValue, BrowserSandboxValue>;
/// One execution's state, retained only until its terminal pass.
pub(super) struct Execution {
    /// Identifier supplied to every provider entry.
    id: String,
    /// Ordered entries, including repeated allocations.
    providers: Vec<Provider>,
    /// Existing console owner; no second console collector.
    console: SharedHandle<ConsoleRuntimeProvider<BrowserSandboxValue>>,
    /// Native file handles and single terminal admission.
    collected: RefCell<ExecutionState<BrowserSandboxValue, BrowserSandboxValue>>,
    /// Owned one-shot result observer.
    sender: RefCell<Option<oneshot::Sender<Result<NativeResult, DiagnosticErrorInfo>>>>,
    /// Existing cancel-on-drop production timer.
    timer: RefCell<Option<Timer>>,
}
/// Internal consumer is last and cannot retain its containing owner strongly.
struct Consumer {
    /// Containing lifecycle without a strong cycle.
    owner: Weak<Owner>,
    /// Captured execution identity, independent of later reuse.
    run: Weak<Execution>,
}
impl MessageConsumer<BrowserSandboxValue> for Consumer {
    fn handle_message(
        &self,
        message: BrowserSandboxValue,
    ) -> BoxFuture<Result<(), DiagnosticErrorInfo>> {
        let result = match (self.owner.upgrade(), self.run.upgrade()) {
            (Some(owner), Some(run)) => {
                owner.receive(&run, &message);
                Ok(())
            }
            _ => Ok(()),
        };
        Box::pin(async { result })
    }
}
/// Dropped pending futures terminate only the execution they admitted.
struct Pending {
    /// Native owner kept alive until settlement or cancellation.
    owner: Rc<Owner>,
    /// Exact run identity, not an identifier lookup.
    run: Rc<Execution>,
}
impl Drop for Pending {
    fn drop(&mut self) {
        let current = self
            .owner
            .execution
            .borrow()
            .as_ref()
            .is_some_and(|run| Rc::ptr_eq(run, &self.run));
        if current {
            self.owner.finish(Err(aborted()));
        }
    }
}
impl SandboxIframe {
    /// Execute through the retained frame, preserving native return and file handles.
    /// # Errors
    /// Rejects cancellation, preparation, validation and native lifecycle failures.
    #[must_use]
    pub fn execute(
        &self,
        request: SandboxExecution<BrowserSandboxValue>,
        consumers: Vec<SharedHandle<dyn MessageConsumer<BrowserSandboxValue>>>,
        is_html_artifact: bool,
    ) -> BoxFuture<Result<NativeResult, DiagnosticErrorInfo>> {
        Box::pin(execute(
            Rc::clone(&self.owner),
            request,
            consumers,
            is_html_artifact,
        ))
    }
}
/// Keep the execution's owned cleanup guard across the awaited result.
async fn execute(
    owner: Rc<Owner>,
    mut request: SandboxExecution<BrowserSandboxValue>,
    consumers: Vec<SharedHandle<dyn MessageConsumer<BrowserSandboxValue>>>,
    is_html_artifact: bool,
) -> Result<NativeResult, DiagnosticErrorInfo> {
    if request
        .signal
        .as_ref()
        .is_some_and(Cancellation::is_aborted)
    {
        return Err(aborted());
    }
    let occupied = owner.execution.borrow().is_some()
        || owner.state.borrow().id.is_some()
        || owner.subscription.borrow().is_some();
    if occupied {
        owner.remove()?;
    }
    let (sender, receiver) = oneshot::channel();
    let console = SharedHandle::new(ConsoleRuntimeProvider::new());
    let providers = std::iter::once(console.clone() as Provider)
        .chain(std::mem::take(&mut request.runtime_providers))
        .collect();
    let run = Rc::new(Execution {
        id: std::mem::take(&mut request.sandbox_id),
        providers,
        console,
        collected: RefCell::default(),
        sender: RefCell::new(Some(sender)),
        timer: RefCell::new(None),
    });
    owner.execution.replace(Some(Rc::clone(&run)));
    let _pending = Pending {
        owner: Rc::clone(&owner),
        run: Rc::clone(&run),
    };
    if let Err(error) = owner.start(&run, &request, consumers, is_html_artifact) {
        owner.finish(Err(error));
    }
    wait(&owner, receiver, request.signal.as_ref()).await
}
/// Observe the existing cancellation future and owned one-shot without another task.
async fn wait(
    owner: &Owner,
    mut receiver: oneshot::Receiver<Result<NativeResult, DiagnosticErrorInfo>>,
    signal: Option<&Cancellation>,
) -> Result<NativeResult, DiagnosticErrorInfo> {
    let mut cancellation = Box::pin(cancelled(signal));
    poll_fn(|cx| {
        if let Poll::Ready(result) = Pin::new(&mut receiver).poll(cx) {
            return Poll::Ready(result.unwrap_or_else(|_| Err(aborted())));
        }
        if cancellation.as_mut().poll(cx).is_ready() {
            owner.finish(Err(aborted()));
            cx.waker().wake_by_ref();
        }
        Poll::Pending
    })
    .await
}
/// Absence of a caller signal never wins settlement.
async fn cancelled(signal: Option<&Cancellation>) {
    if let Some(signal) = signal {
        signal.cancelled().await;
    } else {
        std::future::pending::<()>().await;
    }
}
impl Owner {
    /// Admit only this run's selected fields and settle projection failures immediately.
    fn receive(&self, run: &Rc<Execution>, message: &BrowserSandboxValue) {
        let current = self
            .execution
            .borrow()
            .as_ref()
            .is_some_and(|active| Rc::ptr_eq(active, run));
        if !current {
            return;
        }
        match super::values::select(message) {
            Ok(message) => {
                let result = run.collected.borrow_mut().receive(message);
                if let Some(result) = result {
                    self.finish(Ok(result));
                }
            }
            Err(error) => self.finish(Err(error)),
        }
    }
    /// Invoke the exact production deadline outcome once.
    fn timeout(&self) {
        let run = self.execution.borrow().clone();
        let Some(run) = run else {
            return;
        };
        let result = run.collected.borrow_mut().error(SandboxError {
            message: "Execution timeout (120s)".into(),
            stack: String::new(),
        });
        if let Some(result) = result {
            self.finish(Ok(result));
        }
    }
    /// Register before callbacks or mounting, then recheck live cancellation.
    fn start(
        self: &Rc<Self>,
        run: &Rc<Execution>,
        request: &SandboxExecution<BrowserSandboxValue>,
        consumers: Vec<SharedHandle<dyn MessageConsumer<BrowserSandboxValue>>>,
        is_html_artifact: bool,
    ) -> Result<(), DiagnosticErrorInfo> {
        self.state.borrow_mut().id = Some(run.id.clone());
        crate::RUNTIME_MESSAGE_ROUTER.with(|router| {
            router.register_sandbox(
                run.id.clone(),
                Rc::new(RefCell::new(run.providers.clone())),
                consumers,
            )
        })?;
        for provider in &run.providers {
            provider.on_execution_start(&run.id, request.signal.clone())?;
        }
        if request
            .signal
            .as_ref()
            .is_some_and(Cancellation::is_aborted)
            || self.execution.borrow().is_none()
        {
            return Err(aborted());
        }
        crate::RUNTIME_MESSAGE_ROUTER.with(|router| {
            router.add_consumer(
                &run.id,
                SharedHandle::new(Consumer {
                    owner: Rc::downgrade(self),
                    run: Rc::downgrade(run),
                }),
            );
        });
        let weak = Rc::downgrade(self);
        let callback = Rc::new(move || {
            if let Some(owner) = weak.upgrade() {
                owner.timeout();
            }
        });
        let window = web_sys::window()
            .ok_or_else(|| DiagnosticErrorInfo::error("Missing browser window"))?;
        let timer =
            timer::native(window, 120_000, false, callback).map_err(|error| diagnostic(&error))?;
        run.timer.replace(Some(timer));
        let html = self.prepare_html_document(
            &run.id,
            &request.code,
            &run.providers,
            Some(super::PrepareHtmlOptions {
                is_html_artifact,
                is_standalone: false,
            }),
        )?;
        self.mount(&run.id, html, super::frame::Mount::Execution)
    }
    /// Take terminal ownership before calling out, then attempt every independent release.
    pub(super) fn finish(&self, mut result: Result<NativeResult, DiagnosticErrorInfo>) {
        let Some(run) = self.execution.take() else {
            return;
        };
        let mut failure = None;
        for provider in &run.providers {
            if let Err(error) = provider.on_execution_end(&run.id) {
                failure.get_or_insert(error);
            }
        }
        let timer = run.timer.take();
        drop(timer);
        if let Err(error) = self.cleanup() {
            failure.get_or_insert(error);
        }
        if let Ok(value) = &mut result {
            value.console = super::values::console_entries(&run.console);
        }
        let result = match (result, failure) {
            (Ok(value), Some(error)) if value.success => Err(error),
            (result, _) => result,
        };
        let sender = run.sender.take();
        if let Some(sender) = sender {
            let _delivery = sender.send(result);
        }
    }
}
/// Shared source-authored cancellation diagnostic.
fn aborted() -> DiagnosticErrorInfo {
    DiagnosticErrorInfo::error("Execution aborted")
}
