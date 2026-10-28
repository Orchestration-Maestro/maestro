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
#[path = "context_bridge/builtin.rs"]
mod builtin;
#[cfg(target_arch = "wasm32")]
#[path = "context_bridge/builtin_bash.rs"]
mod builtin_bash;
#[cfg(target_arch = "wasm32")]
#[path = "context_bridge/builtin_edit.rs"]
mod builtin_edit;
#[cfg(target_arch = "wasm32")]
#[path = "context_bridge/builtin_factories.rs"]
mod builtin_factories;
#[cfg(target_arch = "wasm32")]
#[path = "context_bridge/builtin_find.rs"]
mod builtin_find;
#[cfg(target_arch = "wasm32")]
#[path = "context_bridge/builtin_grep.rs"]
mod builtin_grep;
#[cfg(target_arch = "wasm32")]
#[path = "context_bridge/builtin_lifetimes.rs"]
mod builtin_lifetimes;
#[cfg(target_arch = "wasm32")]
#[path = "context_bridge/builtin_ls.rs"]
mod builtin_ls;
#[cfg(target_arch = "wasm32")]
#[path = "context_bridge/builtin_options.rs"]
mod builtin_options;
#[cfg(target_arch = "wasm32")]
#[path = "context_bridge/builtin_progress.rs"]
mod builtin_progress;
#[cfg(target_arch = "wasm32")]
#[path = "context_bridge/builtin_queue.rs"]
mod builtin_queue;
#[cfg(target_arch = "wasm32")]
#[path = "context_bridge/builtin_read.rs"]
mod builtin_read;
#[cfg(target_arch = "wasm32")]
#[path = "context_bridge/builtin_registered.rs"]
mod builtin_registered;
#[cfg(target_arch = "wasm32")]
#[path = "context_bridge/builtin_render.rs"]
mod builtin_render;
#[cfg(target_arch = "wasm32")]
#[path = "context_bridge/builtin_selection.rs"]
mod builtin_selection;
#[cfg(target_arch = "wasm32")]
#[path = "context_bridge/builtin_signals.rs"]
mod builtin_signals;
#[cfg(target_arch = "wasm32")]
#[path = "context_bridge/builtin_write.rs"]
mod builtin_write;
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
#[path = "context_bridge/load_time.rs"]
mod load_time;
#[cfg(target_arch = "wasm32")]
#[path = "context_bridge/oauth.rs"]
mod oauth;
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
#[path = "context_bridge/ui.rs"]
mod ui;

#[cfg(target_arch = "wasm32")]
#[path = "context_bridge/theme.rs"]
mod theme;

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
#[path = "context_bridge/model_registered.rs"]
mod model_registered;

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

#[cfg(target_arch = "wasm32")]
#[path = "context_bridge/model_lifetimes.rs"]
mod model_lifetimes;

#[cfg(target_arch = "wasm32")]
#[path = "context_bridge/model_protocol_routes.rs"]
mod model_protocol_routes;

/// Registers callbacks whose errors expose the actual supplied context operands.
#[cfg(target_arch = "wasm32")]
fn factory(api: ExtensionAPI) -> ExtensionFuture<'static, ()> {
    Box::pin(async move {
        check_start(&api)?;
        load_time::publish(&api)?;
        providers::factory(&api)?;
        register_callbacks(&api)?;
        let renderer_api = api.clone();
        api.register_message_renderer(
            "bridge-view",
            Rc::new(move |message, options, theme, emitter| {
                if let Some(method) = message.custom_type.strip_prefix("global-theme ") {
                    return Ok(Some(theme::view(
                        maestro_extensions_wasm::theme()?,
                        method.into(),
                    )));
                }
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
                Ok(Some(maestro_extensions_wasm::Component::new(ui::View {
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
    if args == "model-protocol-routes" {
        return Box::pin(async { Err(model_protocol_routes::command().await?) });
    }
    if let Some(request) = args.strip_prefix("model-retained ") {
        let result = model_lifetimes::start(&api, &context, request);
        return Box::pin(async move { Err(result?) });
    }
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
    if let Some(action) = args.strip_prefix("model-completions ") {
        let action = action.to_owned();
        return Box::pin(async move { Err(model_completions::command(&action).await?) });
    }
    if args == "model-errors" {
        return Box::pin(async { Err(model_streams::errors().await?) });
    }
    if let Some(request) = args.strip_prefix("model-registered ") {
        let request = request.to_owned();
        return Box::pin(
            async move { Err(model_registered::command(&api, &context, &request).await?) },
        );
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
    if args.starts_with("credential ") || args.starts_with("provider ") {
        return Box::pin(async move {
            Err(credential_command(&api, &context, &args).await?.to_string())
        });
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
        if let Some(result) = builtin::command(&api, &context, &args).await {
            return result;
        }
        if args.starts_with("credential ") || args.starts_with("provider ") {
            return Err(credential_command(&api, &context, &args).await?.to_string());
        }
        if let Some(answer) = ui::custom(&context, &args).await {
            return Err(answer?);
        }
        if args == "ui writer-drain" {
            return Err(ui::writer_author::drain().await?);
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
        if let Some(result) = registrations::command(&api, &context, &args) {
            return result;
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
        context_result(&args, &context).await
    })
}

/// Finish the controlled trap, wait and default command observations.
#[cfg(target_arch = "wasm32")]
async fn context_result(
    args: &str,
    context: &maestro_extensions_wasm::ExtensionCommandContext,
) -> Result<(), String> {
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

/// Registers independently retained listener and native tool callbacks.
#[cfg(target_arch = "wasm32")]
fn register_callbacks(api: &ExtensionAPI) -> maestro_extensions_wasm::ExtensionResult<()> {
    register_listeners(api)?;
    builtin::register(api)
}

/// Selects credential and provider observations without changing the ordinary command routes.
#[cfg(target_arch = "wasm32")]
async fn credential_command(
    api: &ExtensionAPI,
    context: &maestro_extensions_wasm::ExtensionContext,
    request: &str,
) -> Result<serde_json::Value, String> {
    if let Some(request) = request.strip_prefix("credential callbacks") {
        return credentials::composition(api, request.trim_start()).await;
    }
    if let Some(text) = request.strip_prefix("provider control ") {
        return provider_controls::control(api, context, text).await;
    }
    if let Some(request) = request.strip_prefix("credential lifetime ") {
        return credentials::lifetime(api, context, request);
    }
    if let Some(request) = request.strip_prefix("credential pending ") {
        return credentials::pending(api, request).await;
    }
    if let Some(id) = request.strip_prefix("credential oauth builtin ") {
        return oauth::builtin(api, context, id).await;
    }
    if let Some(operation) = request.strip_prefix("credential oauth reverse ") {
        return oauth::reverse(api, operation).await;
    }
    if let Some(mode) = request.strip_prefix("credential oauth storage ") {
        return oauth::storage(api, context, mode).await;
    }
    credentials::observation(api, context, request).await
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
