//! Explicit execution observed through the author facade.
#![forbid(
    clippy::pedantic,
    clippy::too_many_arguments,
    clippy::excessive_nesting
)]
use maestro_extensions_wasm::{
    AbortListener, AbortSignal, BashExec, BashExecResult, BashOperations, ExecOptions,
    ExtensionAPI, ExtensionCommandContext, ExtensionEventResult, ExtensionResult, Presence,
    SignalPort, Subscription, SubscriptionPort, UserBashEventResult,
};
use serde::Deserialize;
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};

/// Literal command operands supplied by the native test.
#[derive(Deserialize)]
struct Request {
    /// Program name without shell interpretation.
    command: String,
    /// Ordered literal arguments.
    args: Vec<String>,
    /// Optional ordinary execution choices.
    options: Option<Options>,
    /// Optional caller-defined signal for the execution.
    signal: Option<Signal>,
    /// Names the flag registered when the relayed listener's captures are released.
    label: Option<String>,
    /// Waits for the command context's idle admission before executing.
    #[serde(default)]
    idle: bool,
    /// Replaces the timeout with an authored infinity.
    #[serde(default)]
    infinite: bool,
}

/// Which caller-defined signal the execution receives.
#[derive(Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
enum Signal {
    /// A signal whose abort predates the execution.
    Preaborted,
    /// A signal relaying the context signal's later abort.
    Relay,
}

/// Ordinary options with no JSON representation of a signal.
#[derive(Deserialize)]
struct Options {
    /// Literal supplied directory.
    cwd: Option<String>,
    /// Optional milliseconds.
    timeout: Option<f64>,
}

/// Returns the exact four execution observations as JSON text.
pub(super) async fn observe(
    api: &ExtensionAPI,
    context: &ExtensionCommandContext,
    request: &str,
) -> ExtensionResult<String> {
    let request: Request = serde_json::from_str(request).map_err(|error| error.to_string())?;
    let mut options = request.options.map(|options| ExecOptions {
        cwd: options.cwd,
        timeout: options.timeout,
        signal: None,
    });
    if request.signal == Some(Signal::Preaborted) {
        options.get_or_insert_default().signal = Some(AbortSignal::new(Rc::new(AlreadyAborted)));
    }
    let relay = if request.signal == Some(Signal::Relay) {
        let inner = context.signal()?.ok_or("missing signal")?;
        let capture = request
            .label
            .map(|label| super::signals::Released(api.clone(), label));
        let (signal, notified) = relay(inner.clone(), capture);
        options.get_or_insert_default().signal = Some(signal);
        Some((inner, notified))
    } else {
        None
    };
    if request.infinite {
        options.get_or_insert_default().timeout = Some(f64::INFINITY);
    }
    let before = relay.as_ref().map(|(signal, _)| signal.aborted());
    if request.idle {
        context.wait_for_idle().await?;
    }
    let result = api.exec(&request.command, &request.args, options).await?;
    let mut observed = serde_json::json!({"stdout":result.stdout, "stderr":result.stderr, "code":result.code, "killed":result.killed});
    if let Some((signal, notified)) = relay {
        observed["before"] = before.into();
        observed["notified"] = notified.get().into();
        observed["after"] = signal.aborted().into();
    }
    Ok(observed.to_string())
}

/// Wraps a supplied signal in an arbitrary caller port with observable listener delivery.
pub(super) fn relay(
    inner: AbortSignal,
    capture: Option<super::signals::Released>,
) -> (AbortSignal, Rc<Cell<u32>>) {
    let notified = Rc::default();
    let signal = Relay {
        inner,
        notified: Rc::clone(&notified),
        capture: RefCell::new(capture),
    };
    (AbortSignal::new(Rc::new(signal)), notified)
}

/// A caller-defined signal forwarding to the context signal and announcing each notification.
struct Relay {
    /// The supplied context signal this caller port wraps.
    inner: AbortSignal,
    /// How many relayed notifications reached the execution's listener.
    notified: Rc<Cell<u32>>,
    /// Moved into the first subscribed listener, so only its release drops it.
    capture: RefCell<Option<super::signals::Released>>,
}

impl SignalPort for Relay {
    fn aborted(&self) -> bool {
        self.inner.aborted()
    }
    fn subscribe(&self, listener: AbortListener) -> ExtensionResult<Subscription> {
        let (notified, capture) = (Rc::clone(&self.notified), self.capture.take());
        self.inner.subscribe(Rc::new(move |emitter| {
            let _ = &capture;
            notified.set(notified.get() + 1);
            emitter.emit("abort-observed", serde_json::json!("relay"))?;
            listener(emitter)
        }))
    }
}

/// A caller-owned prior abort, with no admitted listener to remove or replay.
pub(super) struct AlreadyAborted;

impl SignalPort for AlreadyAborted {
    fn aborted(&self) -> bool {
        true
    }
    fn subscribe(&self, _listener: AbortListener) -> ExtensionResult<Subscription> {
        Ok(Subscription::new(Rc::new(Self)))
    }
}

impl SubscriptionPort for AlreadyAborted {
    fn unsubscribe(&self) -> ExtensionResult<()> {
        Ok(())
    }
}

/// Registers a shell handler returning operations that run each command as a script
/// through explicit execution with the supplied signal, report both streams and reject an
/// aborted run.
pub(super) fn shell(api: &ExtensionAPI) -> ExtensionResult<String> {
    let owner = api.clone();
    api.on(
        "user_bash",
        Rc::new(move |_, _| {
            let capture = Rc::new(super::signals::Released(owner.clone(), "shell".into()));
            let operations = BashOperations {
                exec: remote(&owner, capture),
            };
            Box::pin(async move {
                Ok(Some(ExtensionEventResult::UserBash(UserBashEventResult {
                    result: Presence::Missing,
                    operations: Some(operations),
                })))
            })
        }),
    )?;
    Ok("shell registered".into())
}

/// One returned shell executor, keeping `capture` until the host releases it.
fn remote(api: &ExtensionAPI, capture: Rc<super::signals::Released>) -> BashExec {
    let api = api.clone();
    Rc::new(move |command, cwd, options| {
        let (api, capture) = (api.clone(), Rc::clone(&capture));
        Box::pin(async move {
            let _ = &capture;
            let signal = options.signal.clone();
            let exec = ExecOptions {
                cwd: Some(cwd),
                timeout: options.timeout.map(|seconds| seconds * 1000.0),
                signal: options.signal,
            };
            let result = api
                .exec("python3", &["-c".to_owned(), command], Some(exec))
                .await?;
            (options.on_data)(result.stdout.as_bytes())?;
            (options.on_data)(result.stderr.as_bytes())?;
            if signal.is_some_and(|signal| signal.aborted()) {
                return Err("aborted".into());
            }
            Ok(BashExecResult {
                exit_code: Some(result.code.into()),
            })
        })
    })
}

/// Supply the same arbitrary caller port to model invocation and explicit execution.
pub(super) fn preaborted() -> AbortSignal {
    AbortSignal::new(Rc::new(AlreadyAborted))
}
