//! The author extension shared by the example component and the tests.
//!
//! It is written only against the author facade, so the same code runs as a component and
//! under the controlled adapter. Closures with a [`Released`] guard report through the host
//! when that guard is dropped.
use std::cell::RefCell;
use std::rc::Rc;

use maestro_extensions_wasm::{
    AbortSignal, CommandOptions, CompactionResult, ExtensionAPI, ExtensionEvent,
    ExtensionEventResult, ExtensionFuture, ExtensionHandler, InputEventResult, InputTransform,
    NewSessionCommandData, NewSessionCommandOptions, SessionBeforeCompactResult, SessionEvent,
};
use serde_json::json;

/// Reports its label to the host when this guard is dropped.
struct Released {
    /// Extension handle used to report the release.
    api: ExtensionAPI,
    /// Name reported on release.
    label: &'static str,
}

impl Released {
    /// A guard that reports `label` through `api`.
    fn new(api: &ExtensionAPI, label: &'static str) -> Self {
        Self {
            api: api.clone(),
            label,
        }
    }
}

impl Drop for Released {
    fn drop(&mut self) {
        let _ = self.api.append_entry("released", Some(json!(self.label)));
    }
}

/// The extension factory both adapters run.
#[must_use]
pub fn factory(api: ExtensionAPI) -> ExtensionFuture<'static, ()> {
    Box::pin(async move {
        register_input(&api)?;
        register_compaction(&api)?;
        register_command(&api)?;
        register_rejected(&api)?;
        register_reentrant(&api)?;
        register_trim(&api)?;
        register_note(&api)
    })
}

/// Registers the input handler: it upper-cases every input.
fn register_input(api: &ExtensionAPI) -> Result<(), String> {
    let released = Released::new(api, "input-handler");
    let log = api.clone();
    api.on(
        "input",
        Rc::new(move |event, ctx| {
            let _ = &released;
            let log = log.clone();
            Box::pin(async move {
                let ExtensionEvent::Input(input) = event else {
                    return Ok(None);
                };
                log.append_entry(
                    "input",
                    Some(json!({ "text": input.text, "cwd": ctx.cwd()? })),
                )?;
                Ok(Some(ExtensionEventResult::Input(
                    InputEventResult::Transform(InputTransform {
                        text: input.text.to_uppercase(),
                        images: input.images.clone(),
                    }),
                )))
            })
        }),
    )
}

/// Registers the compaction handler: it cancels an aborted compaction and answers with the
/// state of the signal it retained from the previous one.
fn register_compaction(api: &ExtensionAPI) -> Result<(), String> {
    let released = Released::new(api, "compaction-handler");
    let remembered: Rc<RefCell<Option<AbortSignal>>> = Rc::default();
    api.on(
        "session_before_compact",
        Rc::new(move |event, _ctx| {
            let _ = &released;
            let remembered = Rc::clone(&remembered);
            Box::pin(async move {
                let ExtensionEvent::Session(SessionEvent::BeforeCompact(compact)) = event else {
                    return Ok(None);
                };
                let previous = remembered.borrow().as_ref().map(AbortSignal::aborted);
                remembered.replace(Some(compact.signal.clone()));
                Ok(Some(ExtensionEventResult::SessionBeforeCompact(
                    SessionBeforeCompactResult {
                        cancel: Some(compact.signal.aborted()),
                        compaction: Some(CompactionResult {
                            summary: format!("previous_aborted:{previous:?}"),
                            first_kept_entry_id: compact.preparation.first_kept_entry_id.clone(),
                            tokens_before: compact.preparation.tokens_before,
                            details: None,
                        }),
                    },
                )))
            })
        }),
    )
}

/// Registers the handler that trims an input in place and fails when nothing is left.
fn register_trim(api: &ExtensionAPI) -> Result<(), String> {
    api.on(
        "trim_input",
        Rc::new(|event, _ctx| {
            Box::pin(async move {
                let ExtensionEvent::Input(input) = event else {
                    return Ok(None);
                };
                input.text = input.text.trim().to_owned();
                if input.text.is_empty() {
                    return Err("nothing is left of the input after trimming".to_owned());
                }
                Ok(None)
            })
        }),
    )
}

/// Registers the handler that notes the tokens in the compaction preparation and fails when
/// the compaction was aborted.
fn register_note(api: &ExtensionAPI) -> Result<(), String> {
    api.on(
        "note_compaction",
        Rc::new(|event, _ctx| {
            Box::pin(async move {
                let ExtensionEvent::Session(SessionEvent::BeforeCompact(compact)) = event else {
                    return Ok(None);
                };
                let tokens = compact.preparation.tokens_before;
                compact.preparation.previous_summary = Some(format!("noted {tokens} tokens"));
                if compact.signal.aborted() {
                    return Err("compaction aborted after the note".to_owned());
                }
                Ok(None)
            })
        }),
    )
}

/// Registers a handler the host rejects, then reports the rejection.
fn register_rejected(api: &ExtensionAPI) -> Result<(), String> {
    let released = Released::new(api, "rejected-handler");
    let handler: ExtensionHandler = Rc::new(move |_event, _ctx| {
        let _ = &released;
        Box::pin(async { Ok(None) })
    });
    if let Err(message) = api.on("rejected", handler) {
        api.append_entry("rejection", Some(json!(message)))?;
    }
    Ok(())
}

/// Registers a handler whose captured guard registers another handler when it is dropped.
fn register_reentrant(api: &ExtensionAPI) -> Result<(), String> {
    let guard = Reentrant { api: api.clone() };
    api.on(
        "reentrant",
        Rc::new(move |_event, _ctx| {
            let _ = &guard;
            Box::pin(async { Ok(None) })
        }),
    )
}

/// A guard that registers a handler from its destructor, while its own entry is being released.
struct Reentrant {
    /// Extension handle used to report and register.
    api: ExtensionAPI,
}

impl Drop for Reentrant {
    fn drop(&mut self) {
        let _ = self
            .api
            .append_entry("released", Some(json!("reentrant-guard")));
        let _ = self
            .api
            .on("late", Rc::new(|_event, _ctx| Box::pin(async { Ok(None) })));
    }
}

/// Registers the command that starts a new session and waits for idle from its continuation.
fn register_command(api: &ExtensionAPI) -> Result<(), String> {
    let released = Released::new(api, "command-handler");
    let log = api.clone();
    api.register_command(
        "replace",
        CommandOptions {
            description: Some("Replaces the session".to_owned()),
            handler: Rc::new(move |args, ctx| {
                let _ = &released;
                let log = log.clone();
                Box::pin(async move {
                    let before = ctx.cwd()?;
                    log.append_entry("command", Some(json!({ "args": args, "cwd": before })))?;
                    let outcome = ctx.new_session(Some(replacement(&log))).await?;
                    if let Err(stale) = ctx.cwd() {
                        log.append_entry("stale", Some(json!(stale)))?;
                    }
                    log.append_entry(
                        "after",
                        Some(json!({ "before": before, "cancelled": outcome.cancelled })),
                    )
                })
            }),
        },
    )
}

/// Options whose continuation runs against the replacement session.
fn replacement(log: &ExtensionAPI) -> NewSessionCommandOptions {
    let released = Released::new(log, "continuation");
    let log = log.clone();
    NewSessionCommandOptions {
        data: NewSessionCommandData {
            parent_session: Some("parent".to_owned()),
        },
        with_session: Some(Box::new(move |replaced| {
            Box::pin(async move {
                let _ = &released;
                log.append_entry("continuation", Some(json!({ "cwd": replaced.cwd()? })))?;
                replaced.wait_for_idle().await
            })
        })),
    }
}
