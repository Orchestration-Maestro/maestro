//! Authored interaction for native login, with observable pending input and final release.
use maestro_extensions_wasm::{
    AbortSignal, ExtensionAPI, ExtensionContext, ExtensionResult, ModelRegistry, OAuthAuthCallback,
    OAuthLoginCallbacks, OAuthPromptCallback,
};
use serde_json::{Value, json};
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
    task::{Poll, Waker},
};

/// Original callback observations in delivery order.
type Events = Rc<RefCell<Vec<Value>>>;
thread_local! {
    /// Destruction of the original callback capture.
    static DROPS: Cell<u32> = const { Cell::new(0) };
    /// Explicit acknowledgment of the awaited text prompt.
    static RELEASED: Cell<bool> = const { Cell::new(false) };
    /// Wakeup belonging to the actual authored prompt future.
    static WAKE: RefCell<Option<Waker>> = const { RefCell::new(None) };
}
/// One capture retained through every native alias of the authored interaction.
struct Capture;
impl Drop for Capture {
    fn drop(&mut self) {
        DROPS.set(DROPS.get() + 1);
    }
}

/// Runs login or acknowledges its actual pending prompt without reacquiring a context.
pub fn run<'a>(
    api: &'a ExtensionAPI,
    context: &'a ExtensionContext,
    mode: &'a str,
) -> maestro_extensions_wasm::ExtensionFuture<'a, Value> {
    Box::pin(async move {
        match mode {
            "counts" => return Ok(json!(DROPS.get())),
            "resume" => {
                RELEASED.set(true);
                if let Some(waker) = WAKE.with(|slot| slot.borrow_mut().take()) {
                    waker.wake();
                }
                return Ok(json!("resumed"));
            }
            _ => {}
        }
        let providers = context
            .model_registry()?
            .auth_storage()?
            .get_oauth_providers()?;
        let provider = providers
            .into_iter()
            .find(|provider| provider.id() == "component-login")
            .ok_or("missing controlled login")?;
        let events = Events::default();
        let callbacks = callbacks(api, context, events.clone(), mode)?;
        let login = provider.login(Rc::new(callbacks));
        drop(provider);
        Ok(match login.await {
            Ok(credentials) => json!({"credentials":credentials,"events":*events.borrow()}),
            Err(error) => json!({"error":error,"events":*events.borrow()}),
        })
    })
}

/// Supplies each callback in its original synchronous, awaited or optional form.
pub(super) fn callbacks(
    api: &ExtensionAPI,
    context: &ExtensionContext,
    events: Events,
    mode: &str,
) -> ExtensionResult<OAuthLoginCallbacks> {
    let (progress, manual, select) = (events.clone(), events.clone(), events.clone());
    let (progress_error, manual_error, select_error) = (
        mode == "progress-error",
        mode == "manual-error",
        mode == "select-error",
    );
    Ok(OAuthLoginCallbacks {
        on_auth: auth(
            context.model_registry()?,
            events.clone(),
            mode == "auth-error",
        ),
        on_prompt: prompt(api, events, mode),
        on_progress: (mode != "absent").then(|| {
            Rc::new(move |message: &str| {
                progress.borrow_mut().push(json!(["progress", message]));
                answer(progress_error, "progress", ())
            }) as _
        }),
        on_manual_code_input: (mode != "absent").then(|| {
            Rc::new(move || {
                manual.borrow_mut().push(json!(["manual"]));
                Box::pin(std::future::ready(answer(
                    manual_error,
                    "manual",
                    "manual-code".into(),
                ))) as maestro_extensions_wasm::ExtensionFuture<'static, String>
            }) as _
        }),
        on_select: (mode != "absent").then(|| {
            Rc::new(move |prompt| {
                select.borrow_mut().push(json!(["select", prompt]));
                Box::pin(std::future::ready(answer(select_error, "select", None)))
                    as maestro_extensions_wasm::ExtensionFuture<'static, Option<String>>
            }) as _
        }),
        signal: match mode {
            "signal" => context.signal()?,
            "pre-aborted" => Some(AbortSignal::new(Rc::new(
                super::super::exec::AlreadyAborted,
            ))),
            _ => None,
        },
    })
}

/// A synchronous callback can reenter the native catalog before returning.
fn auth(registry: ModelRegistry, events: Events, fail: bool) -> OAuthAuthCallback {
    let capture = Capture;
    Rc::new(move |info| {
        let _capture = &capture;
        registry.get_all()?;
        events.borrow_mut().push(json!(["auth", info]));
        answer(fail, "auth", ())
    })
}

/// Awaited input announces its first pending poll and resumes only on explicit acknowledgment.
fn prompt(api: &ExtensionAPI, events: Events, mode: &str) -> OAuthPromptCallback {
    let (pending, fail, bus) = (mode == "pending", mode == "prompt-error", api.events());
    let text = if mode == "builtin" {
        ""
    } else {
        " Literal answer "
    };
    Rc::new(move |prompt| {
        events.borrow_mut().push(json!(["prompt", prompt]));
        let (bus, result) = (bus.clone(), answer(fail, "prompt", text.into()));
        Box::pin(async move {
            if pending {
                wait(&bus).await?;
            }
            result
        })
    })
}

/// Preserves authored failures without changing successful values.
fn answer<T>(fail: bool, phase: &str, value: T) -> ExtensionResult<T> {
    if fail {
        Err(format!("literal {phase} failure"))
    } else {
        Ok(value)
    }
}

/// Waits for acknowledgment after notifying the native observer, without a timer.
async fn wait(bus: &maestro_extensions_wasm::EventBus) -> ExtensionResult<()> {
    bus.emit("oauth-prompt-ready", Value::Null).await?;
    std::future::poll_fn(|context| {
        if RELEASED.get() {
            Poll::Ready(())
        } else {
            WAKE.with(|slot| *slot.borrow_mut() = Some(context.waker().clone()));
            Poll::Pending
        }
    })
    .await;
    Ok(())
}

/// Uses the credential owner for persistence rather than accepting a provider result locally.
pub async fn storage(
    api: &ExtensionAPI,
    context: &ExtensionContext,
    mode: &str,
) -> ExtensionResult<Value> {
    let events = Events::default();
    let callbacks = Rc::new(callbacks(api, context, events.clone(), mode)?);
    let storage = super::super::credentials::login(api, mode, callbacks).await?;
    Ok(json!({"storage":storage,"events":*events.borrow()}))
}
