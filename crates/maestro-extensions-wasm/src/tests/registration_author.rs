//! Authored registrations driven through the public extension facade.
use maestro_extensions_wasm::{
    ArgumentCompletions, AutocompleteItem, CommandOptions, ExtensionAPI, ShortcutOptions,
};
use serde::Deserialize;
use serde_json::json;
use std::rc::Rc;

/// One registration request from the driving scenario.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Request {
    /// Literal command name.
    name: String,
    /// Optional description, including an empty string.
    description: Option<String>,
    /// Whether completion is independently supplied.
    completion: bool,
    /// Literal shortcut key spelling.
    key: String,
}

/// Registers independent command, completion and ordinary-context callbacks.
pub(super) fn register(api: &ExtensionAPI, text: &str) -> Result<(), String> {
    let request: Request = serde_json::from_str(text).map_err(|e| e.to_string())?;
    let log = api.clone();
    let completions: Option<ArgumentCompletions> = request.completion.then(|| {
        Rc::new(
            |prefix| -> maestro_extensions_wasm::ExtensionFuture<
                'static,
                Option<Vec<AutocompleteItem>>,
            > {
                Box::pin(async move {
                    Ok(Some(vec![AutocompleteItem {
                        value: prefix,
                        label: "completion Ω".into(),
                        description: Some(String::new()),
                    }]))
                })
            },
        ) as ArgumentCompletions
    });
    api.register_command(
        &request.name,
        CommandOptions {
            description: request.description.clone(),
            get_argument_completions: completions,
            handler: Rc::new(move |args, ctx| {
                let log = log.clone();
                Box::pin(async move {
                    log.append_entry(
                        "registered-command",
                        Some(json!({"args":args, "cwd":ctx.cwd()?})),
                    )
                })
            }),
        },
    )?;
    let log = api.clone();
    api.register_shortcut(
        &request.key,
        ShortcutOptions {
            description: request.description,
            handler: Rc::new(move |ctx| {
                let log = log.clone();
                Box::pin(
                    async move { log.append_entry("registered-shortcut", Some(json!(ctx.cwd()?))) },
                )
            }),
        },
    )
}

/// Converts an independently typed authored flag payload.
fn flag_value(value: super::registration_fixture::Value) -> maestro_extensions_wasm::FlagValue {
    match value {
        super::registration_fixture::Value::Boolean(value) => {
            maestro_extensions_wasm::FlagValue::Boolean(value)
        }
        super::registration_fixture::Value::String(value) => {
            maestro_extensions_wasm::FlagValue::String(value)
        }
    }
}

/// Registers selected flag fields through the public author API.
pub(super) fn register_flag(api: &ExtensionAPI, text: &str) -> Result<(), String> {
    use super::registration_fixture::{Flag, Kind};
    let flag: Flag = serde_json::from_str(text).map_err(|e| e.to_string())?;
    api.register_flag(
        &flag.name,
        maestro_extensions_wasm::FlagOptions {
            description: flag.description,
            type_: match flag.kind {
                Kind::Boolean => maestro_extensions_wasm::FlagType::Boolean,
                Kind::String => maestro_extensions_wasm::FlagType::String,
            },
            default: flag.default.map(flag_value),
        },
    )
}

/// Reads the current host reply, observing absence separately from false and empty.
pub(super) fn read_flag(api: &ExtensionAPI, name: &str) -> Result<(), String> {
    let value = api.get_flag(name)?.map(|value| match value {
        maestro_extensions_wasm::FlagValue::Boolean(value) => json!(value),
        maestro_extensions_wasm::FlagValue::String(value) => json!(value),
    });
    api.append_entry("flag-read", Some(json!(value)))
}

/// Installs a completion callback with a selected answer and optional host barrier.
pub(super) fn completion(
    api: &ExtensionAPI,
    text: &str,
    ctx: maestro_extensions_wasm::ExtensionCommandContext,
) -> Result<(), String> {
    use super::registration_fixture::{Completion, Mode};
    let request: Completion = serde_json::from_str(text).map_err(|e| e.to_string())?;
    let answer = request.answer.map(|items| {
        items
            .into_iter()
            .map(|item| AutocompleteItem {
                value: item.value,
                label: item.label,
                description: item.description,
            })
            .collect::<Vec<_>>()
    });
    let log = api.clone();
    api.register_command(
        "complete",
        CommandOptions {
            description: None,
            handler: Rc::new(|_, _| Box::pin(async { Ok(()) })),
            get_argument_completions: Some(Rc::new(move |prefix| {
                let (log, ctx, answer) = (log.clone(), ctx.clone(), answer.clone());
                Box::pin(async move {
                    log.append_entry("completion-prefix", Some(json!(prefix)))?;
                    if matches!(request.mode, Mode::Pending) {
                        ctx.wait_for_idle().await?;
                    }
                    Ok(answer)
                })
            })),
        },
    )
}

/// Installs the documented environment-name completion example.
pub(super) fn example(api: &ExtensionAPI) -> Result<(), String> {
    api.register_command(
        "environment",
        CommandOptions {
            description: None,
            handler: Rc::new(|_, _| Box::pin(async { Ok(()) })),
            get_argument_completions: Some(Rc::new(|prefix| {
                Box::pin(async move {
                    let items = ["dev", "staging", "prod"]
                        .into_iter()
                        .filter(|name| name.starts_with(&prefix))
                        .map(|name| AutocompleteItem {
                            value: name.into(),
                            label: name.into(),
                            description: None,
                        })
                        .collect::<Vec<_>>();
                    Ok((!items.is_empty()).then_some(items))
                })
            })),
        },
    )
}

/// Independent callback ownership controls.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Ownership {
    /// Literal registration name.
    pub name: String,
    /// Optional completion callback.
    pub completion: bool,
    /// Keep an independently authored completion alias.
    pub alias: bool,
    /// Register from the completion capture's destructor.
    pub reenter: bool,
}

/// A completion capture whose destruction reenters the same registration API.
struct Reenter {
    /// The facade used only after the old entry leaves the table.
    api: ExtensionAPI,
}

impl Drop for Reenter {
    fn drop(&mut self) {
        let result = self.api.register_command(
            "fresh",
            CommandOptions {
                description: None,
                get_argument_completions: None,
                handler: Rc::new(|_, _| Box::pin(async { Ok(()) })),
            },
        );
        let _ = self.api.append_entry("reentered", Some(json!(result)));
    }
}

/// Registers drop-reporting callbacks with independently retained aliases.
pub(super) fn ownership(api: &ExtensionAPI, text: &str) -> Result<(), String> {
    let request: Ownership = serde_json::from_str(text).map_err(|e| e.to_string())?;
    let handler_guard = super::Released::new(api, "owned command");
    let log = api.clone();
    let handler = Rc::new(
        move |args: String, _: maestro_extensions_wasm::ExtensionCommandContext| {
            let _ = &handler_guard;
            let log = log.clone();
            Box::pin(async move { log.append_entry("owned-command", Some(json!(args))) })
                as maestro_extensions_wasm::ExtensionFuture<'static, ()>
        },
    );
    let completion: Option<ArgumentCompletions> = request.completion.then(|| {
        let guard = super::Released::new(api, "owned completion");
        let reenter = request.reenter.then(|| Reenter { api: api.clone() });
        Rc::new(move |prefix| {
            let _ = (&guard, &reenter);
            Box::pin(async move {
                Ok(Some(vec![AutocompleteItem {
                    value: prefix,
                    label: "owned".into(),
                    description: None,
                }]))
            })
                as maestro_extensions_wasm::ExtensionFuture<'static, Option<Vec<AutocompleteItem>>>
        }) as ArgumentCompletions
    });
    let alias = request.alias.then(|| completion.clone()).flatten();
    api.register_command(
        &request.name,
        CommandOptions {
            description: None,
            handler,
            get_argument_completions: completion,
        },
    )?;
    if let Some(alias) = alias {
        register_alias_control(api, alias)?;
    }
    Ok(())
}

/// Keeps an authored alias outside the released registration and removes it explicitly.
fn register_alias_control(api: &ExtensionAPI, alias: ArgumentCompletions) -> Result<(), String> {
    let slot = Rc::new(std::cell::RefCell::new(Some(alias)));
    let log = api.clone();
    api.register_command(
        "alias-control",
        CommandOptions {
            description: None,
            get_argument_completions: None,
            handler: Rc::new(move |args, _| {
                let (slot, log) = (Rc::clone(&slot), log.clone());
                Box::pin(async move {
                    let alias = if args == "drop" {
                        slot.take()
                    } else {
                        slot.borrow().clone()
                    };
                    if args == "drop" {
                        drop(alias);
                        return Ok(());
                    }
                    let answer = alias.ok_or("alias absent")?(args).await?;
                    log.append_entry(
                        "alias-answer",
                        Some(json!(answer.map(|items| {
                            items.into_iter().map(|item| item.value).collect::<Vec<_>>()
                        }))),
                    )
                })
            }),
        },
    )
}

/// Installs a drop-reporting shortcut for rejection and release checks.
pub(super) fn owned_shortcut(api: &ExtensionAPI) -> Result<(), String> {
    let guard = super::Released::new(api, "owned shortcut");
    api.register_shortcut(
        "owned-shortcut",
        ShortcutOptions {
            description: None,
            handler: Rc::new(move |_| {
                let _ = &guard;
                Box::pin(async { Ok(()) })
            }),
        },
    )
}

/// Records a prior effect and fails only the first invocation, retaining later recovery.
async fn fail_first(
    api: ExtensionAPI,
    request: super::registration_fixture::Failure,
    command: maestro_extensions_wasm::ExtensionCommandContext,
    first: bool,
) -> Result<(), String> {
    if !first {
        return api.append_entry("healthy", Some(json!(request.kind.name())));
    }
    api.append_entry(
        "callback-effect",
        Some(json!(format!("{} before failure", request.kind.name()))),
    )?;
    if matches!(request.mode, super::registration_fixture::Mode::Pending) {
        command.wait_for_idle().await?;
    }
    Err(request.error)
}

/// Registers each declared callback kind with a catchable first-call failure.
pub(super) fn failure(
    api: &ExtensionAPI,
    text: &str,
    command: maestro_extensions_wasm::ExtensionCommandContext,
) -> Result<(), String> {
    use super::registration_fixture::{CallbackKind, Failure};
    let request: Failure = serde_json::from_str(text).map_err(|e| e.to_string())?;
    let (log, calls) = (api.clone(), std::cell::Cell::new(0));
    match request.kind {
        CallbackKind::Command => api.register_command(
            "failure",
            CommandOptions {
                description: None,
                get_argument_completions: None,
                handler: Rc::new(move |_, _| {
                    let first = calls.replace(calls.get() + 1) == 0;
                    Box::pin(fail_first(
                        log.clone(),
                        request.clone(),
                        command.clone(),
                        first,
                    ))
                }),
            },
        ),
        CallbackKind::Completion => api.register_command(
            "failure",
            CommandOptions {
                description: None,
                handler: Rc::new(|_, _| Box::pin(async { Ok(()) })),
                get_argument_completions: Some(Rc::new(move |_| {
                    let first = calls.replace(calls.get() + 1) == 0;
                    let run = fail_first(log.clone(), request.clone(), command.clone(), first);
                    Box::pin(async move {
                        run.await?;
                        Ok(None)
                    })
                })),
            },
        ),
        CallbackKind::Shortcut => api.register_shortcut(
            "failure",
            ShortcutOptions {
                description: None,
                handler: Rc::new(move |_| {
                    let first = calls.replace(calls.get() + 1) == 0;
                    Box::pin(fail_first(
                        log.clone(),
                        request.clone(),
                        command.clone(),
                        first,
                    ))
                }),
            },
        ),
    }
}

/// Registers pending calls whose running futures own their captures independently.
pub(super) fn pending(
    api: &ExtensionAPI,
    command: maestro_extensions_wasm::ExtensionCommandContext,
) -> Result<(), String> {
    let guard = Rc::new(super::Released::new(api, "pending completion"));
    let log = api.clone();
    let barrier = command.clone();
    api.register_command(
        "pending",
        CommandOptions {
            description: None,
            handler: Rc::new(|_, _| Box::pin(async { Ok(()) })),
            get_argument_completions: Some(Rc::new(move |prefix| {
                let (guard, log, barrier) = (Rc::clone(&guard), log.clone(), barrier.clone());
                Box::pin(async move {
                    barrier.wait_for_idle().await?;
                    log.append_entry("pending-done", Some(json!(prefix)))?;
                    drop(guard);
                    Ok(Some(vec![AutocompleteItem {
                        value: prefix,
                        label: "captured".into(),
                        description: Some("origin".into()),
                    }]))
                })
            })),
        },
    )?;
    pending_shortcut(api, command)
}

/// Registers an ordinary-context shortcut and independent retained-context controls.
fn pending_shortcut(
    api: &ExtensionAPI,
    command: maestro_extensions_wasm::ExtensionCommandContext,
) -> Result<(), String> {
    let slot = Rc::new(std::cell::RefCell::new(Vec::<
        maestro_extensions_wasm::ExtensionContext,
    >::new()));
    let (log, captured) = (api.clone(), Rc::clone(&slot));
    let guard = Rc::new(super::Released::new(api, "pending shortcut"));
    api.register_shortcut(
        "pending",
        ShortcutOptions {
            description: None,
            handler: Rc::new(move |ctx| {
                let (log, captured, guard, command) = (
                    log.clone(),
                    Rc::clone(&captured),
                    Rc::clone(&guard),
                    command.clone(),
                );
                Box::pin(async move {
                    let aliases = if ctx.cwd()? == "/first" {
                        vec![ctx.clone(), ctx.clone()]
                    } else {
                        vec![]
                    };
                    if !aliases.is_empty() {
                        let old = captured.replace(aliases);
                        drop(old);
                    }
                    command.wait_for_idle().await?;
                    log.append_entry("pending-done", Some(json!(ctx.cwd()?)))?;
                    drop(guard);
                    Ok(())
                })
            }),
        },
    )?;
    context_control(api, slot)
}

/// Invokes an ordinary context without parsing the registered key.
pub(super) fn shortcut(
    api: &ExtensionAPI,
    text: &str,
    command: maestro_extensions_wasm::ExtensionCommandContext,
) -> Result<(), String> {
    use super::registration_fixture::{Mode, Shortcut};
    let request: Shortcut = serde_json::from_str(text).map_err(|e| e.to_string())?;
    let log = api.clone();
    api.register_shortcut(
        &request.key,
        ShortcutOptions {
            description: request.description,
            handler: Rc::new(move |ctx| {
                let (log, command) = (log.clone(), command.clone());
                Box::pin(async move {
                    if matches!(request.mode, Mode::Pending) {
                        command.wait_for_idle().await?;
                    }
                    log.append_entry("shortcut-origin", Some(json!(ctx.cwd()?)))
                })
            }),
        },
    )
}

/// Routes authored registration controls without growing the shared capture command.
pub(super) fn dispatch(
    api: &ExtensionAPI,
    args: &str,
    ctx: &maestro_extensions_wasm::ExtensionCommandContext,
) -> Option<Result<(), String>> {
    if let Some(request) = args.strip_prefix("shortcut ") {
        return Some(shortcut(api, request, ctx.clone()));
    }
    if args == "pending-registrations" {
        return Some(pending(api, ctx.clone()));
    }
    if let Some(request) = args.strip_prefix("callback-failure ") {
        return Some(failure(api, request, ctx.clone()));
    }
    if let Some(request) = args.strip_prefix("ownership ") {
        return Some(ownership(api, request));
    }
    if args == "owned-shortcut" {
        return Some(owned_shortcut(api));
    }
    if let Some(request) = args.strip_prefix("completion ") {
        return Some(completion(api, request, ctx.clone()));
    }
    if args == "completion-example" {
        return Some(example(api));
    }
    if let Some(request) = args.strip_prefix("flag-register ") {
        return Some(register_flag(api, request));
    }
    if let Some(name) = args.strip_prefix("flag-read ") {
        return Some(read_flag(api, name));
    }
    if let Some(request) = args.strip_prefix("registration ") {
        return Some(register(api, request));
    }
    None
}

/// Observes and drops the authored aliases without holding their slot across host calls.
fn context_control(
    api: &ExtensionAPI,
    slot: Rc<std::cell::RefCell<Vec<maestro_extensions_wasm::ExtensionContext>>>,
) -> Result<(), String> {
    let log = api.clone();
    api.register_command(
        "context-control",
        CommandOptions {
            description: None,
            get_argument_completions: None,
            handler: Rc::new(move |args, _| {
                let (slot, log) = (Rc::clone(&slot), log.clone());
                Box::pin(async move {
                    let mut owned = slot.take();
                    if args == "drop-one" {
                        drop(owned.pop());
                    }
                    if args == "drop-all" {
                        drop(owned);
                        return Ok(());
                    }
                    let value = owned
                        .first()
                        .map(maestro_extensions_wasm::ExtensionContext::cwd)
                        .transpose()?;
                    let old = slot.replace(owned);
                    drop(old);
                    log.append_entry("context-alias", Some(json!(value)))
                })
            }),
        },
    )
}

/// Registers a flag before factory completion without needing an invocation context.
pub(super) fn factory_flag(api: &ExtensionAPI) -> Result<(), String> {
    api.register_flag(
        "factory-flag",
        maestro_extensions_wasm::FlagOptions {
            description: Some(String::new()),
            type_: maestro_extensions_wasm::FlagType::String,
            default: Some(maestro_extensions_wasm::FlagValue::Boolean(false)),
        },
    )
}
