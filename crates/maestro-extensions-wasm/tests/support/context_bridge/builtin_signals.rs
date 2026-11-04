//! Retained wrapped execution follows an arbitrary author signal after event return.
#![forbid(
    clippy::pedantic,
    clippy::too_many_arguments,
    clippy::excessive_nesting
)]
use maestro_extensions_wasm::{
    AbortListener, AbortSignal, BashExecResult, BashOperations, ExtensionAPI,
    ExtensionCommandContext, ExtensionEventResult, ExtensionResult, ReadOperations,
    ReadToolOptions, SignalPort, Subscription, SubscriptionPort, UserBashEventResult,
};
use std::{cell::RefCell, rc::Rc};

thread_local! {
    /// Completed author-side lifecycle observations, independent of native settlement.
    static TRACE: RefCell<Vec<String>> = const { RefCell::new(Vec::new()) };
}

/// Records only actual subscription, notification, removal and completion calls.
fn record(label: &str, phase: &str) {
    TRACE.with_borrow_mut(|trace| trace.push(format!("{label}:{phase}")));
}

/// Installs the event factory or reads its closed execution transcript.
pub(super) fn command(
    api: &ExtensionAPI,
    context: &ExtensionCommandContext,
    action: &str,
) -> ExtensionResult<String> {
    if action == "trace" {
        return TRACE
            .with_borrow(|trace| serde_json::to_string(trace).map_err(|error| error.to_string()));
    }
    if action != "install" {
        return Err("unknown retained signal action".into());
    }
    let (api, context) = (api.clone(), context.clone());
    let factory = api.clone();
    api.on(
        "user_bash",
        Rc::new(move |_, _| {
            let result = returned(&factory, context.clone());
            Box::pin(async move {
                Ok(Some(ExtensionEventResult::UserBash(UserBashEventResult {
                    operations: Some(result?),
                    result: maestro_extensions_wasm::Presence::Missing,
                })))
            })
        }),
    )?;
    Ok("retained signal factory installed".into())
}

/// Constructs inside the event; its executable and effects outlive that handler.
fn returned(
    api: &ExtensionAPI,
    context: ExtensionCommandContext,
) -> ExtensionResult<BashOperations> {
    let operations = ReadOperations {
        access: Rc::new(|_| Box::pin(async { Ok(()) })),
        detect_image_mime_type: None,
        read_file: Rc::new(move |_| {
            let context = context.clone();
            Box::pin(async move {
                context
                    .wait_for_idle()
                    .await
                    .map_err(maestro_extensions_wasm::DiagnosticErrorInfo::error)?;
                Ok(b"retained bytes".to_vec())
            })
        }),
    };
    let tool = Rc::new(api.create_read_tool(
        "/".into(),
        ReadToolOptions {
            operations: Some(operations),
            ..ReadToolOptions::default()
        },
    )?);
    Ok(BashOperations {
        exec: Rc::new(move |label, _, options| {
            let tool = tool.clone();
            Box::pin(async move {
                let signal = options.signal.map(|inner| {
                    AbortSignal::new(Rc::new(Observed {
                        inner,
                        label: label.clone(),
                    }))
                });
                let result = (tool.execute)(
                    label.clone(),
                    serde_json::json!({"path":"virtual.txt"}),
                    signal,
                    None,
                )
                .await;
                record(&label, "completed");
                result?;
                (options.on_data)(b"retained bytes")?;
                Ok(BashExecResult {
                    exit_code: Some(0.0),
                })
            })
        }),
    })
}

/// A caller-defined signal, not a downcastable native adapter signal.
struct Observed {
    /// The particular native caller's live flag.
    inner: AbortSignal,
    /// Distinguishes overlapping calls and the unaffected decoy.
    label: String,
}

impl SignalPort for Observed {
    fn aborted(&self) -> bool {
        self.inner.aborted()
    }
    fn subscribe(&self, listener: AbortListener) -> ExtensionResult<Subscription> {
        let label = self.label.clone();
        let inner = self.inner.subscribe(Rc::new(move |emitter| {
            record(&label, "notified");
            listener(emitter)
        }))?;
        record(&self.label, "subscribed");
        Ok(Subscription::new(Rc::new(Remover {
            inner,
            label: self.label.clone(),
        })))
    }
}

/// Explicitly forwards removal rather than confusing it with dropping the handle.
struct Remover {
    /// The original host subscription.
    inner: Subscription,
    /// Call whose registration must be removed.
    label: String,
}

impl SubscriptionPort for Remover {
    fn unsubscribe(&self) -> ExtensionResult<()> {
        self.inner.unsubscribe()?;
        record(&self.label, "removed");
        Ok(())
    }
}
