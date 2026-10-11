//! Fire-and-return notification callbacks authored through the existing contexts.
use maestro_extensions_wasm::{
    CallbackEmitter, CompactOptions, CompactionComplete, CompactionError, CompactionResult,
    ExtensionAPI, ExtensionCommandContext, ExtensionContext, ExtensionResult,
    NewSessionCommandData, NewSessionCommandOptions, Presence,
};
use serde_json::{Value, json};
use std::{cell::RefCell, rc::Rc};
/// Reports a captured value's destruction, optionally registering a new compaction.
struct Capture {
    /// Reporting facade.
    api: ExtensionAPI,
    /// Distinguishable capture label.
    name: String,
    /// Context used to prove remove-before-drop.
    reenter: Option<ExtensionContext>,
}
impl Drop for Capture {
    fn drop(&mut self) {
        let _ = self
            .api
            .append_entry("compact-drop", Some(json!(self.name)));
        if let Some(ctx) = self.reenter.take() {
            let api = self.api.clone();
            let _ = ctx.compact(Some(CompactOptions {
                on_complete: Some(Rc::new(move |result, _| {
                    api.append_entry("reentrant-complete", Some(json!(result.summary)))
                })),
                ..Default::default()
            }));
        }
    }
}
/// Independently retained authored callback aliases.
#[derive(Clone, Default)]
pub struct Aliases(Rc<RefCell<Option<CompactionComplete>>>);
/// Complete result observation with opaque details decoded once for source comparison.
fn result_value(result: CompactionResult) -> ExtensionResult<Value> {
    let details = result.details;
    let mut value = json!({"summary":result.summary,"firstKeptEntryId":result.first_kept_entry_id,"tokensBefore":result.tokens_before});
    match details {
        Presence::Missing => {}
        Presence::Null => {
            value["details"] = Value::Null;
        }
        Presence::Present(text) => {
            value["details"] = serde_json::from_str(&text).map_err(|e| e.to_string())?;
        }
    }
    Ok(value)
}
/// An optional callback-scoped emission, whose rejection can be caught by author code.
fn emit(api: &ExtensionAPI, emitter: &CallbackEmitter<'_>, input: &Value) -> ExtensionResult<()> {
    if let Some(data) = input.get("emit") {
        let result = emitter.emit(data["channel"].as_str().unwrap(), data["payload"].clone());
        if input["catchEmit"] == true {
            api.append_entry("emit-error", Some(json!(result.err())))?;
        } else {
            result?;
        }
    }
    Ok(())
}
/// Build independently optional notifications without assigning application policy.
fn options(
    api: &ExtensionAPI,
    ctx: &ExtensionContext,
    input: &Value,
    aliases: &Aliases,
) -> Option<CompactOptions> {
    if input["absent"] == true {
        return None;
    }
    let complete = if input["alias"] == true {
        aliases.0.borrow().clone()
    } else if input["complete"] == true {
        let guard = Capture {
            api: api.clone(),
            name: format!("complete {}", input["name"].as_str().unwrap_or("")),
            reenter: (input["reenter"] == true).then(|| ctx.clone()),
        };
        let retain_alias = input["retainAlias"] == true;
        let api = api.clone();
        let input = input.clone();
        let callback: CompactionComplete = Rc::new(move |result, emitter| {
            let _ = &guard;
            if input["wire"] == true {
                api.append_entry(
                    "compact-wire",
                    Some(serde_json::to_value(&result).map_err(|e| e.to_string())?),
                )?;
            }
            let negative = result.tokens_before.is_sign_negative();
            api.append_entry("compact-complete",Some(json!({"name":input["name"],"value":result_value(result)?,"negativeZero":negative})))?;
            emit(&api, &emitter, &input)?;
            if let Some(error) = input["completeError"].as_str() {
                return Err(error.into());
            }
            api.append_entry("compact-callback-return", Some(json!("complete")))
        });
        if retain_alias {
            aliases.0.replace(Some(callback.clone()));
        }
        Some(callback)
    } else {
        None
    };
    let error = error_callback(api, input);
    Some(CompactOptions {
        custom_instructions: input["instructions"]["present"]
            .as_bool()
            .unwrap_or(false)
            .then(|| input["instructions"]["value"].as_str().unwrap().to_owned()),
        on_complete: complete,
        on_error: error,
    })
}
/// Admit one compaction synchronously and report the returned host result.
fn admit(
    api: &ExtensionAPI,
    ctx: &ExtensionContext,
    input: &Value,
    aliases: &Aliases,
) -> ExtensionResult<()> {
    let result = ctx.compact(options(api, ctx, input, aliases));
    api.append_entry(
        "compact-return",
        Some(match result {
            Ok(()) => json!({"ok":null}),
            Err(error) => json!({"error":error}),
        }),
    )
}
/// Ordinary event view and retained aliases share the existing author registration.
pub fn register(api: &ExtensionAPI, aliases: Aliases) -> ExtensionResult<()> {
    let api_copy = api.clone();
    api.on(
        "action-compact",
        Rc::new(move |event, ctx| {
            let api = api_copy.clone();
            let aliases = aliases.clone();
            Box::pin(async move {
                let maestro_extensions_wasm::ExtensionEvent::Input(input) = event else {
                    return Err("wrong action event".into());
                };
                let request: Value =
                    serde_json::from_str(&input.text).map_err(|e| e.to_string())?;
                admit(&api, &ctx, &request, &aliases)?;
                Ok(None)
            })
        }),
    )
}
/// Dispatch command and replacement views plus explicit alias teardown.
pub async fn dispatch(
    api: &ExtensionAPI,
    ctx: &ExtensionCommandContext,
    input: &Value,
    aliases: &Aliases,
) -> ExtensionResult<()> {
    match input["view"].as_str() {
        Some("receiver") => receiver(api),
        Some("drop-alias") => {
            let removed = aliases.0.take();
            drop(removed);
            Ok(())
        }
        Some("register") => register(api, aliases.clone()),
        Some("replacement") => {
            let api = api.clone();
            let input = input.clone();
            let aliases = aliases.clone();
            ctx.new_session(Some(NewSessionCommandOptions {
                data: NewSessionCommandData {
                    parent_session: None,
                },
                with_session: Some(Box::new(move |fresh| {
                    Box::pin(async move { admit(&api, &fresh, &input, &aliases) })
                })),
            }))
            .await?;
            Ok(())
        }
        _ => admit(api, ctx, input, aliases),
    }
}

/// Optional error notification with its own captured owner.
fn error_callback(api: &ExtensionAPI, input: &Value) -> Option<CompactionError> {
    (input["error"] == true).then(|| {
        let guard = Capture {
            api: api.clone(),
            name: format!("error {}", input["name"].as_str().unwrap_or("")),
            reenter: None,
        };

        let api = api.clone();
        let input = input.clone();
        let callback: CompactionError = Rc::new(move |error, emitter| {
            let _ = &guard;
            api.append_entry("compact-error", Some(json!(error)))?;
            emit(&api, &emitter, &input)?;
            if let Some(error) = input["errorError"].as_str() {
                return Err(error.into());
            }
            api.append_entry("compact-callback-return", Some(json!("error")))
        });
        callback
    })
}

/// Register a receiver driven by the host after callback return.
fn receiver(api: &ExtensionAPI) -> ExtensionResult<()> {
    let api = api.clone();
    api.clone().on(
        "action-receiver",
        Rc::new(move |event, _| {
            let api = api.clone();
            Box::pin(async move {
                let maestro_extensions_wasm::ExtensionEvent::Input(input) = event else {
                    return Err("wrong receiver event".into());
                };
                api.append_entry(
                    "compact-received",
                    Some(serde_json::from_str(&input.text).map_err(|e| e.to_string())?),
                )?;
                Ok(None)
            })
        }),
    )
}
