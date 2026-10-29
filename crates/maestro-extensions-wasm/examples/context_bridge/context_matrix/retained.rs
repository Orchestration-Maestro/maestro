//! Ordinary hook contexts retained across later native bindings.
#![forbid(
    clippy::pedantic,
    clippy::too_many_arguments,
    clippy::excessive_nesting
)]
use maestro_extensions_wasm::{
    ExtensionAPI, ExtensionContext, ExtensionEvent, ExtensionUIContext, Presence, SessionEvent,
};
use serde_json::Value;
use std::{cell::RefCell, collections::HashMap, rc::Rc};

/// Installs the existing hook with per-instance retained context observations.
pub(crate) fn register(api: &ExtensionAPI) -> Result<(), String> {
    let contexts = Rc::new(RefCell::new(HashMap::<String, ExtensionContext>::new()));
    let ui = Rc::new(RefCell::new(None::<ExtensionUIContext>));
    api.on(
        "session_start",
        Rc::new(move |event, context| {
            let request = match event {
                ExtensionEvent::Session(SessionEvent::Start(start)) => {
                    match &start.previous_session_file {
                        Presence::Present(request) => {
                            request.strip_prefix("context retained ").map(str::to_owned)
                        }
                        _ => None,
                    }
                }
                _ => None,
            };
            let Some(request) = request else {
                return super::super::session_start(event, context);
            };
            let contexts = contexts.clone();
            let ui = ui.clone();
            Box::pin(async move {
                let result = observe(
                    &context,
                    &request,
                    &mut contexts.borrow_mut(),
                    &mut ui.borrow_mut(),
                );
                Err(serde_json::to_string(&result).map_err(|error| error.to_string())?)
            })
        }),
    )
}

/// Reads an earlier hook context without replacing it with the dispatcher's fresh one.
fn observe(
    context: &ExtensionContext,
    request: &str,
    contexts: &mut HashMap<String, ExtensionContext>,
    ui: &mut Option<ExtensionUIContext>,
) -> Result<Value, String> {
    let (slot, member) = request.split_once(' ').ok_or("missing retained member")?;
    if slot == "save" {
        contexts.insert(member.into(), context.clone());
        return Ok(Value::Null);
    }
    let context = contexts.get(slot).ok_or("missing retained context")?;
    match member {
        "saveUI" => {
            *ui = Some(context.ui()?);
            Ok(Value::Null)
        }
        "retainedUI" => {
            ui.as_ref()
                .ok_or("missing retained UI")?
                .notify("retained notification", None)?;
            Ok(Value::Null)
        }
        _ => super::base(context, member, "none"),
    }
}
