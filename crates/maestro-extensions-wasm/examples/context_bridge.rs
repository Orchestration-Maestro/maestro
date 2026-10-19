//! Author component for native context forwarding through the extension loader.
#![forbid(
    clippy::pedantic,
    clippy::too_many_arguments,
    clippy::excessive_nesting
)]

#[cfg(target_arch = "wasm32")]
use maestro_extensions_wasm::{
    CommandOptions, ExtensionAPI, ExtensionEvent, ExtensionFuture, SessionEvent,
};
#[cfg(target_arch = "wasm32")]
use std::rc::Rc;

/// Registers callbacks whose errors expose the actual supplied context operands.
#[cfg(target_arch = "wasm32")]
fn factory(api: ExtensionAPI) -> ExtensionFuture<'static, ()> {
    Box::pin(async move {
        api.register_flag(
            "fail-start",
            maestro_extensions_wasm::FlagOptions {
                description: None,
                type_: maestro_extensions_wasm::FlagType::Boolean,
                default: None,
            },
        )?;
        if matches!(
            api.get_flag("fail-start")?,
            Some(maestro_extensions_wasm::FlagValue::Boolean(true))
        ) {
            return Err("guest startup failure".into());
        }
        register_listeners(&api)?;
        let renderer_api = api.clone();
        api.register_message_renderer(
            "bridge-view",
            Rc::new(move |message, options, _, emitter| {
                emitter.emit(
                    "renderer-created",
                    serde_json::json!([message.custom_type, options.expanded]),
                )?;
                if message.custom_type == "cross-instance" {
                    emitter.emit("trace", serde_json::json!("renderer-return:cross-instance"))?;
                }
                if message.custom_type == "creation-error" {
                    return Err("guest creation failure".into());
                }
                Ok(Some(maestro_extensions_wasm::Component::new(View {
                    api: renderer_api.clone(),
                    name: message.custom_type,
                })))
            }),
        )?;
        api.register_command(
            "bridge-observe",
            CommandOptions {
                description: Some("Observe the supplied context".into()),
                get_argument_completions: None,
                handler: Rc::new(command),
            },
        )?;
        api.on("session_start", Rc::new(session_start))
    })
}

/// Registers real same-instance listeners, including recursive synchronous emission.
#[cfg(target_arch = "wasm32")]
fn register_listeners(api: &ExtensionAPI) -> Result<(), String> {
    for label in ["g1", "g2"] {
        let events = api.events();
        api.events().on(
            "renderer-created",
            Rc::new(move |data, emitter| {
                let name = data[0].as_str().ok_or("missing name")?;
                emitter.emit("trace", serde_json::json!(format!("{label}:{name}")))?;
                if label == "g1" && name == "listener-error" {
                    return Err("guest listener failure".into());
                }
                if name == "listener-tail" {
                    let events = events.clone();
                    return Ok(Some(Box::pin(async move {
                        events
                            .emit("trace", serde_json::json!(format!("{label}:tail")))
                            .await
                    })));
                }
                if label == "g1" && name == "reentry" {
                    emitter.emit("renderer-created", serde_json::json!(["inner", false]))?;
                }
                Ok(None)
            }),
        )?;
    }
    Ok(())
}

/// Uses independently supplied native command contexts and controllable pending operations.
#[cfg(target_arch = "wasm32")]
fn command(
    args: String,
    context: maestro_extensions_wasm::ExtensionCommandContext,
) -> ExtensionFuture<'static, ()> {
    Box::pin(async move {
        if args == "trap" || args == "pending" {
            context.get_system_prompt()?;
            context.wait_for_idle().await?;
            if args == "trap" {
                std::arch::wasm32::unreachable();
            }
        }
        Err(format!(
            "command:{args}|{}|{}",
            context.cwd()?,
            context.get_system_prompt()?
        ))
    })
}

/// Exposes the actual context supplied by the typed native session-start caller.
#[cfg(target_arch = "wasm32")]
fn session_start(
    event: &mut ExtensionEvent,
    context: maestro_extensions_wasm::ExtensionContext,
) -> ExtensionFuture<'_, Option<maestro_extensions_wasm::ExtensionEventResult>> {
    Box::pin(async move {
        let ExtensionEvent::Session(SessionEvent::Start(_)) = event else {
            return Err("session-start callback received another event".into());
        };
        Err(format!(
            "hook:{}|{}",
            context.cwd()?,
            context.get_system_prompt()?
        ))
    })
}

#[cfg(target_arch = "wasm32")]
maestro_extensions_wasm::export_extension!(factory);

/// Retained authored view exposes exact method failures and final destruction.
#[cfg(target_arch = "wasm32")]
struct View {
    /// Original API, including during the guest resource destructor.
    api: ExtensionAPI,
    /// Literal custom-message identity supplied by the caller.
    name: String,
}
#[cfg(target_arch = "wasm32")]
impl maestro_extensions_wasm::ComponentView for View {
    fn render(
        &self,
        width: u32,
        emitter: maestro_extensions_wasm::CallbackEmitter<'_>,
    ) -> Result<Vec<String>, String> {
        emitter.emit("component-rendered", serde_json::json!([self.name, width]))?;
        if self.name == "render-error" {
            return Err("guest render failure".into());
        }
        Ok(vec![format!("{}:{width}", self.name)])
    }
    fn handle_input(
        &self,
        data: &str,
        _: maestro_extensions_wasm::CallbackEmitter<'_>,
    ) -> Result<(), String> {
        if data == "error" {
            Err("guest input failure".into())
        } else {
            Ok(())
        }
    }
    fn wants_key_release(
        &self,
        _: maestro_extensions_wasm::CallbackEmitter<'_>,
    ) -> Result<bool, String> {
        if self.name == "key-error" {
            Err("guest key failure".into())
        } else {
            Ok(true)
        }
    }
    fn invalidate(&self, _: maestro_extensions_wasm::CallbackEmitter<'_>) -> Result<(), String> {
        if self.name == "invalidate-error" {
            Err("guest invalidate failure".into())
        } else {
            Ok(())
        }
    }
}
#[cfg(target_arch = "wasm32")]
impl Drop for View {
    fn drop(&mut self) {
        let _ = self.api.register_flag(
            &format!("dropped-{}", self.name),
            maestro_extensions_wasm::FlagOptions {
                description: None,
                type_: maestro_extensions_wasm::FlagType::Boolean,
                default: Some(maestro_extensions_wasm::FlagValue::Boolean(true)),
            },
        );
    }
}
