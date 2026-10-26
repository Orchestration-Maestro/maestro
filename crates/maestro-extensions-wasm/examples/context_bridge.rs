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

#[cfg(target_arch = "wasm32")]
#[path = "context_bridge/api.rs"]
mod api;
#[cfg(target_arch = "wasm32")]
#[path = "context_bridge/commands.rs"]
mod commands;
#[cfg(target_arch = "wasm32")]
#[path = "context_bridge/credentials.rs"]
mod credentials;
#[cfg(target_arch = "wasm32")]
#[path = "context_bridge/exec.rs"]
mod exec;
#[cfg(target_arch = "wasm32")]
#[path = "context_bridge/json_boundary.rs"]
mod json_boundary;
#[cfg(target_arch = "wasm32")]
#[path = "context_bridge/prefixes.rs"]
mod prefixes;

#[cfg(target_arch = "wasm32")]
#[path = "context_bridge/provider_controls.rs"]
mod provider_controls;
#[cfg(target_arch = "wasm32")]
#[path = "context_bridge/providers.rs"]
mod providers;
#[cfg(target_arch = "wasm32")]
#[path = "context_bridge/reads.rs"]
mod reads;
#[cfg(target_arch = "wasm32")]
#[path = "context_bridge/registrations.rs"]
mod registrations;
#[cfg(target_arch = "wasm32")]
#[path = "context_bridge/retained.rs"]
mod retained;

#[cfg(target_arch = "wasm32")]
#[path = "context_bridge/signals.rs"]
mod signals;

#[cfg(target_arch = "wasm32")]
#[path = "../src/tests/editor_factory_author.rs"]
mod editor_factory_author;

#[cfg(target_arch = "wasm32")]
#[path = "../src/tests/stacked_provider_author.rs"]
mod stacked_provider_author;

#[cfg(target_arch = "wasm32")]
#[path = "../src/tests/factory_component_author.rs"]
mod factory_component_author;

#[cfg(target_arch = "wasm32")]
#[path = "context_bridge/ui.rs"]
mod ui;

#[cfg(target_arch = "wasm32")]
#[path = "context_bridge/theme.rs"]
mod theme;

#[cfg(target_arch = "wasm32")]
#[path = "../src/tests/focus_author.rs"]
mod focus_author;
#[cfg(target_arch = "wasm32")]
#[path = "../src/tests/tui_author.rs"]
mod tui_author;
#[cfg(target_arch = "wasm32")]
#[path = "../src/tests/widget_author.rs"]
mod widget_author;

#[cfg(target_arch = "wasm32")]
#[path = "context_bridge/schema.rs"]
mod schema;

#[cfg(target_arch = "wasm32")]
#[path = "context_bridge/streams.rs"]
mod streams;

#[cfg(target_arch = "wasm32")]
#[path = "context_bridge/stream_lifetimes.rs"]
mod stream_lifetimes;

#[cfg(target_arch = "wasm32")]
#[path = "context_bridge/model_streams.rs"]
mod model_streams;

#[cfg(target_arch = "wasm32")]
#[path = "context_bridge/model_completions.rs"]
mod model_completions;

#[cfg(target_arch = "wasm32")]
#[path = "context_bridge/model_hooks.rs"]
mod model_hooks;

#[cfg(target_arch = "wasm32")]
#[path = "context_bridge/model_hook_gates.rs"]
mod model_hook_gates;

#[cfg(target_arch = "wasm32")]
#[path = "context_bridge/model_finite.rs"]
mod model_finite;

/// Registers callbacks whose errors expose the actual supplied context operands.
#[cfg(target_arch = "wasm32")]
fn factory(api: ExtensionAPI) -> ExtensionFuture<'static, ()> {
    Box::pin(async move {
        check_start(&api)?;
        providers::factory(&api)?;
        register_listeners(&api)?;
        let renderer_api = api.clone();
        api.register_message_renderer(
            "bridge-view",
            Rc::new(move |message, options, theme, emitter| {
                if let Some(method) = message.custom_type.strip_prefix("theme ") {
                    return Ok(Some(theme::view(theme, method.into())));
                }
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
                if message.custom_type == "send-message" {
                    send_inner(&renderer_api)?;
                    emitter.emit("trace", serde_json::json!("send-returned"))?;
                }
                Ok(Some(maestro_extensions_wasm::Component::new(View {
                    api: renderer_api.clone(),
                    name: message.custom_type,
                })))
            }),
        )?;
        let release = ReleaseTrap(std::cell::Cell::new(false));
        let command_api = api.clone();
        api.register_command(
            "bridge-observe",
            CommandOptions {
                description: Some("Observe the supplied context".into()),
                get_argument_completions: None,
                handler: Rc::new(move |args, context| {
                    if args == "arm-release-trap" {
                        release.arm();
                    }
                    command(args, context, command_api.clone())
                }),
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
                    return Ok(Some(listener_tail(events.clone(), label)));
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

/// Creates a separately owned continuation for one listener prefix.
#[cfg(target_arch = "wasm32")]
fn listener_tail(
    events: maestro_extensions_wasm::EventBus,
    label: &'static str,
) -> ExtensionFuture<'static, ()> {
    Box::pin(async move {
        events
            .emit("trace", serde_json::json!(format!("{label}:tail")))
            .await
    })
}

/// Uses independently supplied native command contexts and controllable pending operations.
#[cfg(target_arch = "wasm32")]
fn command(
    args: String,
    context: maestro_extensions_wasm::ExtensionCommandContext,
    api: ExtensionAPI,
) -> ExtensionFuture<'static, ()> {
    if let Some(request) = args.strip_prefix("model-selected-signal ") {
        let request = request.to_owned();
        return Box::pin(async move { Err(model_hook_gates::signals(&request, context).await?) });
    }
    if let Some(request) = args.strip_prefix("model-hook-gates ") {
        let request = request.to_owned();
        return Box::pin(async move { Err(model_hook_gates::command(&request, context).await?) });
    }
    if args == "model-finite" {
        return Box::pin(async { Err(model_finite::command()?) });
    }
    if args == "model-signals" {
        return Box::pin(async { Err(model_hooks::signals().await?) });
    }
    if args == "model-hooks" {
        return Box::pin(async { Err(model_hooks::command().await?) });
    }
    if let Some(action) = args.strip_prefix("model-completions ") {
        let action = action.to_owned();
        return Box::pin(async move { Err(model_completions::command(&action).await?) });
    }
    if args == "model-errors" {
        return Box::pin(async { Err(model_streams::errors().await?) });
    }
    if args == "model-streams" {
        return Box::pin(async { Err(model_streams::command().await?) });
    }
    if let Some(action) = args.strip_prefix("stream-lifetime ") {
        let action = action.to_owned();
        return Box::pin(
            async move { Err(stream_lifetimes::command(&api, &context, &action).await?) },
        );
    }
    if let Some(request) = args.strip_prefix("stream ") {
        let request = request.to_owned();
        return Box::pin(async move { Err(streams::command(&request).await?) });
    }
    context_command(args, context, api)
}

/// Dispatch context and registration observations unrelated to stream ownership.
#[cfg(target_arch = "wasm32")]
fn context_command(
    args: String,
    context: maestro_extensions_wasm::ExtensionCommandContext,
    api: ExtensionAPI,
) -> ExtensionFuture<'static, ()> {
    Box::pin(async move {
        if args.starts_with("credential ") || args.starts_with("provider ") {
            return Err(credential_command(&api, &context, &args).await?.to_string());
        }
        if args == "clock wait" {
            std::thread::sleep(std::time::Duration::from_millis(1));
            return Err("clock waited".into());
        }
        if args == "retain-suspended" {
            return retained::observe(api, context).await;
        }
        if args == "healthy-admitted" {
            return retained::healthy(context).await;
        }
        if args == "exec shell" {
            return Err(exec::shell(&api)?);
        }
        if let Some(request) = args.strip_prefix("exec imports:") {
            return Err(exec::observe(&api, &context, request).await?);
        }
        if args == "register-tool" || args == "register-painter" {
            return registrations::register_tool(&api, &args);
        }
        if args == "register-prefixes" {
            return prefixes::register(&api, context);
        }
        if args == "compact" {
            return compact(&context);
        }
        if let Some(root) = args.strip_prefix("api imports:") {
            return Err(api::observe(&api, &context, root).await?.to_string());
        }
        if let Some(path) = args.strip_prefix("command imports:") {
            return Err(commands::observe(&context, path.into()).await?.to_string());
        }
        if let Some(request) = args.strip_prefix("signal ") {
            return Err(signals::command(&api, &context, request)?);
        }
        if let Some(answer) = schema::command(&api, &args) {
            return Err(answer?.to_string());
        }
        if let Some(request) = args.strip_prefix("dialog ") {
            return Err(ui::dialog(&context, request).await?);
        }
        if args.starts_with("ui ") || args.starts_with("editor ") {
            return Err(ui::observe(&context, &api, &args)?);
        }
        if args == "imports" {
            return Err(reads::observe(&context)?.to_string());
        }
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

/// Sends a displayed custom message whose rendering the host may run before returning.
#[cfg(target_arch = "wasm32")]
fn send_inner(api: &ExtensionAPI) -> Result<(), String> {
    let message = serde_json::from_value(
        serde_json::json!({"customType": "inner", "content": "sent", "display": true}),
    )
    .map_err(|error| error.to_string())?;
    api.send_message(message, None)
}

/// Starts compaction with notifications the host may deliver before compaction returns.
#[cfg(target_arch = "wasm32")]
fn compact(context: &maestro_extensions_wasm::ExtensionCommandContext) -> Result<(), String> {
    context.compact(Some(maestro_extensions_wasm::CompactOptions {
        custom_instructions: None,
        on_complete: Some(Rc::new(|_, emitter| {
            emitter.emit("trace", serde_json::json!("compact-complete"))
        })),
        on_error: Some(Rc::new(|error, emitter| {
            emitter.emit("trace", serde_json::json!(format!("compact-error:{error}")))
        })),
    }))?;
    Err("compact returned".into())
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
        if context.get_system_prompt()? == "canonical imports" {
            return Err(reads::observe(&context)?.to_string());
        }
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

/// Makes a callback-release trap observable through the real lifecycle.
#[cfg(target_arch = "wasm32")]
struct ReleaseTrap(std::cell::Cell<bool>);
#[cfg(target_arch = "wasm32")]
impl ReleaseTrap {
    /// Arms only the deliberately selected destructor witness.
    fn arm(&self) {
        self.0.set(true);
    }
}
#[cfg(target_arch = "wasm32")]
impl Drop for ReleaseTrap {
    fn drop(&mut self) {
        if self.0.get() {
            std::arch::wasm32::unreachable();
        }
    }
}

/// Selects credential and provider observations without changing the ordinary command routes.
#[cfg(target_arch = "wasm32")]
async fn credential_command(
    api: &ExtensionAPI,
    context: &maestro_extensions_wasm::ExtensionContext,
    request: &str,
) -> Result<serde_json::Value, String> {
    if let Some(text) = request.strip_prefix("provider control ") {
        return provider_controls::control(api, context, text).await;
    }
    if let Some(request) = request.strip_prefix("credential lifetime ") {
        return credentials::lifetime(api, context, request);
    }
    if let Some(request) = request.strip_prefix("credential pending ") {
        return credentials::pending(api, request).await;
    }
    match request {
        "credential fallback" => credentials::fallback(api).await,
        "credential backend" => credentials::backend(api),
        "credential state" => credentials::state(api).await,
        _ => providers::command(api, context, request).await,
    }
}

/// Preserves the caller-controlled startup failure before any callback registration.
#[cfg(target_arch = "wasm32")]
fn check_start(api: &ExtensionAPI) -> Result<(), String> {
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
    Ok(())
}
