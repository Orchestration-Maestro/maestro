//! Authored callbacks exercised by the original native registration owners.
#![forbid(
    clippy::pedantic,
    clippy::too_many_arguments,
    clippy::excessive_nesting
)]
use maestro_extensions_wasm::{ExtensionAPI, ExtensionResult};
use serde_json::json;
use std::rc::Rc;

/// Registers callbacks with observable literal operands and native context reads.
pub(super) fn register(api: &ExtensionAPI) -> ExtensionResult<()> {
    api.register_command(
        "canonical-completion",
        maestro_extensions_wasm::CommandOptions {
            description: Some(" literal command ".into()),
            handler: Rc::new(|args, context| {
                Box::pin(async move { Err(format!("{args}|{}", context.cwd()?)) })
            }),
            get_argument_completions: Some(Rc::new(|prefix| {
                Box::pin(async move {
                    Ok(Some(vec![maestro_extensions_wasm::AutocompleteItem {
                        value: format!("{prefix} literal value "),
                        label: " literal label ".into(),
                        description: Some(" literal description ".into()),
                    }]))
                })
            })),
        },
    )?;
    api.register_shortcut(
        "ctrl+alt+k",
        maestro_extensions_wasm::ShortcutOptions {
            description: Some(" literal shortcut ".into()),
            handler: Rc::new(|context| {
                Box::pin(async move { Err(format!("shortcut:{}", context.cwd()?)) })
            }),
        },
    )?;
    tool(api)?;
    shell(api)
}

/// Uses the original update callback, cancellation and per-call context capabilities.
pub(super) fn tool(api: &ExtensionAPI) -> ExtensionResult<()> {
    api.register_tool(maestro_extensions_wasm::ToolDefinition {
        metadata: serde_json::from_value(json!({"name":"canonical-tool", "label":" Literal tool ", "description":" Literal tool description ", "parameters":{"type":"object"}})).map_err(|error| error.to_string())?,
        prepare_arguments: Some(Rc::new(|args, emitter| {
            emitter.emit("canonical-prepare", args.clone())?;
            Ok(json!({"prepared":args}))
        })),
        execute: Rc::new(|id, params, signal, update, context| Box::pin(async move {
            let details = json!({"id":id,"params":params,"cwd":context.cwd()?,"aborted":signal.ok_or("missing tool signal")?.aborted()});
            (update.ok_or("missing tool update")?)(serde_json::from_value(json!({"content":[],"details":details})).map_err(|error| error.to_string())?)?;
            serde_json::from_value(json!({"content":[{"type":"text","text":" literal result "}],"details":details,"terminate":false})).map_err(|error| error.to_string())
        })),
    })
}

/// Checks literal execution operands before streaming uninterpreted bytes.
fn shell_exec(
    command: String,
    cwd: String,
    options: maestro_extensions_wasm::BashExecOptions,
) -> maestro_extensions_wasm::ExtensionFuture<'static, maestro_extensions_wasm::BashExecResult> {
    Box::pin(async move {
        if command != " literal shell "
            || cwd != "./literal/../cwd"
            || options.timeout != Some(2.5)
            || !options.signal.ok_or("missing shell signal")?.aborted()
        {
            return Err("shell operands changed".into());
        }
        (options.on_data)(&[0, 255, 10])?;
        (options.on_data)(&[])?;
        Ok(maestro_extensions_wasm::BashExecResult {
            exit_code: Some(7.5),
        })
    })
}

/// Returns an authored executor without launching a process or adding timeout policy.
fn shell(api: &ExtensionAPI) -> ExtensionResult<()> {
    api.on(
        "user_bash",
        Rc::new(|event, _| {
            Box::pin(async move {
                let maestro_extensions_wasm::ExtensionEvent::UserBash(event) = event else {
                    return Err("expected shell event".into());
                };
                event.command.push_str(":edited");
                Ok(Some(
                    maestro_extensions_wasm::ExtensionEventResult::UserBash(
                        maestro_extensions_wasm::UserBashEventResult {
                            result: maestro_extensions_wasm::Presence::Missing,
                            operations: Some(maestro_extensions_wasm::BashOperations {
                                exec: Rc::new(shell_exec),
                            }),
                        },
                    ),
                ))
            })
        }),
    )
}
