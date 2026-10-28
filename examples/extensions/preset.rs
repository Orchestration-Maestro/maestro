//! Named model, thinking, tool and prompt configurations selected for one session.
#![forbid(
    clippy::pedantic,
    clippy::too_many_arguments,
    clippy::excessive_nesting
)]

#[path = "preset/config.rs"]
mod config;
#[path = "preset/selector.rs"]
mod selector;
use config::{Preset, Presets};
use maestro_extensions_wasm::{
    BeforeAgentStartEventResult, CommandOptions, ExtensionAPI, ExtensionContext, ExtensionEvent,
    ExtensionEventResult, ExtensionFuture, ExtensionResult, FlagOptions, FlagType, FlagValue, Key,
    Model, NotificationType, Presence, SessionEntry, ShortcutOptions, ThinkingLevel, utils,
};
use std::{cell::RefCell, rc::Rc};

/// Host settings captured before the first preset is applied.
#[derive(Clone)]
struct Original {
    /// Selected descriptor, when the host has one.
    model: Option<Model>,
    /// Requested reasoning effort.
    thinking: ThinkingLevel,
    /// Active tool names in host order.
    tools: Vec<String>,
}

/// Configuration and the currently selected preset, shared by this factory's callbacks.
#[derive(Default)]
struct State {
    /// Layered configurations in file order.
    presets: Presets,
    /// Active name and instructions, independent of later file reads.
    active: Option<(String, Preset)>,
    /// Settings to restore on clearing a preset.
    original: Option<Original>,
    /// A clearing choice still awaiting its next persisted turn.
    cleared: bool,
}

/// Updates the footer through the current UI theme.
fn status(ctx: &ExtensionContext, state: &State) -> ExtensionResult<()> {
    let text = state
        .active
        .as_ref()
        .map(|(name, _)| format!("preset:{name}"));
    let ui = ctx.ui()?;
    let styled = text
        .map(|text| ui.theme()?.fg("accent", &text))
        .transpose()?;
    ui.set_status("preset", styled.as_deref())
}

/// Restores only the newest saved name in the active branch, without changing host settings.
fn restore(ctx: &ExtensionContext, state: &mut State) -> ExtensionResult<()> {
    state.active = None;
    state.original = None;
    state.cleared = false;
    for entry in ctx.session_manager()?.get_branch(None)? {
        if let SessionEntry::Custom(entry) = entry
            && entry.custom_type == "preset-state"
            && let Some(data) = entry.data.present()
        {
            let data: serde_json::Value =
                serde_json::from_str(data).map_err(|error| error.to_string())?;
            state.active = data
                .get("name")
                .and_then(serde_json::Value::as_str)
                .and_then(|name| {
                    state
                        .presets
                        .get(name)
                        .map(|preset| (name.into(), preset.clone()))
                });
        }
    }
    Ok(())
}

/// Selects the exact configured model, keeping warnings nonfatal as other settings apply.
async fn model(
    api: &ExtensionAPI,
    ctx: &ExtensionContext,
    name: &str,
    preset: &Preset,
) -> ExtensionResult<()> {
    if let (Some(provider), Some(id)) = (&preset.provider, &preset.model)
        && !provider.is_empty()
        && !id.is_empty()
    {
        let warning = if let Some(model) = ctx.model_registry()?.find(provider, id)? {
            if api.set_model(model).await? {
                None
            } else {
                Some(format!("No API key for {provider}/{id}"))
            }
        } else {
            Some(format!("Model {provider}/{id} not found"))
        };
        if let Some(warning) = warning {
            ctx.ui()?.notify(
                &format!("Preset \"{name}\": {warning}"),
                Some(NotificationType::Warning),
            )?;
        }
    }
    Ok(())
}

/// Validates tool names against the host inventory without replacing tools with an empty set.
fn tools(
    api: &ExtensionAPI,
    ctx: &ExtensionContext,
    name: &str,
    names: &[String],
) -> ExtensionResult<()> {
    if names.is_empty() {
        return Ok(());
    }
    let inventory = api.get_all_tools()?;
    let (valid, invalid): (Vec<_>, Vec<_>) = names
        .iter()
        .cloned()
        .partition(|name| inventory.iter().any(|tool| tool.name == *name));
    if !invalid.is_empty() {
        ctx.ui()?.notify(
            &format!("Preset \"{name}\": Unknown tools: {}", invalid.join(", ")),
            Some(NotificationType::Warning),
        )?;
    }
    if !valid.is_empty() {
        api.set_active_tools(&valid)?;
    }
    Ok(())
}

/// Applies a preset, capturing defaults only when transitioning from no preset.
async fn apply(
    api: &ExtensionAPI,
    state: &RefCell<State>,
    selection: (&str, Preset),
    ctx: &ExtensionContext,
) -> ExtensionResult<()> {
    let (name, preset) = selection;
    if state.borrow().active.is_none() {
        let original = Original {
            model: ctx.model()?,
            thinking: api.get_thinking_level()?,
            tools: api.get_active_tools()?,
        };
        state.borrow_mut().original = Some(original);
    }
    model(api, ctx, name, &preset).await?;
    if let Some(thinking) = &preset.thinking_level {
        api.set_thinking_level(thinking.clone())?;
    }
    if let Some(names) = &preset.tools {
        tools(api, ctx, name, names)?;
    }
    let mut state = state.borrow_mut();
    state.active = Some((name.into(), preset));
    state.cleared = false;
    Ok(())
}

/// Clears instructions and restores the saved host settings, or the authored fallback tools.
async fn clear(api: &ExtensionAPI, state: &RefCell<State>) -> ExtensionResult<()> {
    state.borrow_mut().active = None;
    state.borrow_mut().cleared = true;
    let original = state.borrow().original.clone();
    if let Some(original) = original {
        if let Some(model) = original.model {
            api.set_model(model).await?;
        }
        api.set_thinking_level(original.thinking)?;
        api.set_active_tools(&original.tools)?;
    } else {
        api.set_active_tools(&["read".into(), "bash".into(), "edit".into(), "write".into()])?;
    }
    Ok(())
}

/// Applies a known selection and reports the exact activation or clearing message.
async fn choose(
    api: &ExtensionAPI,
    state: &RefCell<State>,
    name: &str,
    ctx: &ExtensionContext,
) -> ExtensionResult<()> {
    let message = if name == "(none)" {
        clear(api, state).await?;
        "Preset cleared, defaults restored".into()
    } else {
        let preset = state.borrow().presets.get(name).cloned();
        let Some(preset) = preset else { return Ok(()) };
        apply(api, state, (name, preset), ctx).await?;
        format!("Preset \"{name}\" activated")
    };
    ctx.ui()?.notify(&message, Some(NotificationType::Info))?;
    status(ctx, &state.borrow())
}

/// Reports an unknown name with the available names in configuration order.
fn unknown(
    ctx: &ExtensionContext,
    state: &State,
    name: &str,
    kind: NotificationType,
) -> ExtensionResult<()> {
    let available = state.presets.keys().cloned().collect::<Vec<_>>().join(", ");
    let available = if available.is_empty() {
        "(none defined)"
    } else {
        &available
    };
    ctx.ui()?.notify(
        &format!("Unknown preset \"{name}\". Available: {available}"),
        Some(kind),
    )
}

/// Warns rather than opening an empty selector or cycling an empty inventory.
fn has_presets(ctx: &ExtensionContext, state: &State) -> ExtensionResult<bool> {
    if !state.presets.is_empty() {
        return Ok(true);
    }
    ctx.ui()?.notify(
        "No presets defined. Add presets to ~/.maestro/agent/presets.json or .maestro/presets.json",
        Some(NotificationType::Warning),
    )?;
    Ok(false)
}

/// Trims only characters the host's shared whitespace helper recognizes.
fn trimmed<'a>(api: &ExtensionAPI, text: &'a str) -> ExtensionResult<&'a str> {
    let mut start = 0;
    let mut end = text.len();
    for (offset, c) in text.char_indices() {
        if !utils::is_whitespace_char(api, &c.to_string())? {
            start = offset;
            break;
        }
        start = offset + c.len_utf8();
    }
    for (offset, c) in text[start..].char_indices().rev() {
        if !utils::is_whitespace_char(api, &c.to_string())? {
            end = start + offset + c.len_utf8();
            break;
        }
        end = start + offset;
    }
    Ok(&text[start..end])
}

/// Handles direct names or opens the custom selector with the current active marker.
async fn command(
    api: ExtensionAPI,
    state: Rc<RefCell<State>>,
    args: String,
    ctx: ExtensionContext,
) -> ExtensionResult<()> {
    let name = trimmed(&api, &args)?;
    if !name.is_empty() {
        if !state.borrow().presets.contains_key(name) {
            return unknown(&ctx, &state.borrow(), name, NotificationType::Error);
        }
        return choose(&api, &state, name, &ctx).await;
    }
    if !has_presets(&ctx, &state.borrow())? {
        return Ok(());
    }
    let (presets, active) = {
        let state = state.borrow();
        (
            state.presets.clone(),
            state.active.as_ref().map(|(name, _)| name.clone()),
        )
    };
    if let Some(name) = selector::select(&ctx, &presets, active.as_deref()).await? {
        choose(&api, &state, &name, &ctx).await?;
    }
    Ok(())
}

/// Cycles sorted preset names, with the clearing choice before the first name.
async fn cycle(
    api: ExtensionAPI,
    state: Rc<RefCell<State>>,
    ctx: ExtensionContext,
) -> ExtensionResult<()> {
    if !has_presets(&ctx, &state.borrow())? {
        return Ok(());
    }
    let mut names = state.borrow().presets.keys().cloned().collect::<Vec<_>>();
    names.sort_by(|a, b| a.encode_utf16().cmp(b.encode_utf16()));
    names.insert(0, "(none)".into());
    let active = state
        .borrow()
        .active
        .as_ref()
        .map_or("(none)".into(), |(name, _)| name.clone());
    let next = names
        .iter()
        .position(|name| *name == active)
        .map_or(0, |index| (index + 1) % names.len());
    choose(&api, &state, &names[next], &ctx).await
}

/// Registers the command, shortcut and string flag on the existing host facade.
fn register_controls(api: &ExtensionAPI, state: &Rc<RefCell<State>>) -> ExtensionResult<()> {
    api.register_flag(
        "preset",
        FlagOptions {
            description: Some("Preset configuration to use".into()),
            type_: FlagType::String,
            default: None,
        },
    )?;
    let (registered, chosen) = (api.clone(), state.clone());
    api.register_command(
        "preset",
        CommandOptions {
            description: Some("Switch preset configuration".into()),
            get_argument_completions: None,
            handler: Rc::new(move |args, ctx| {
                Box::pin(command(
                    registered.clone(),
                    chosen.clone(),
                    args,
                    (*ctx).clone(),
                ))
            }),
        },
    )?;
    let (registered, chosen) = (api.clone(), state.clone());
    api.register_shortcut(
        &Key::ctrl_shift(api, "u")?,
        ShortcutOptions {
            description: Some("Cycle presets".into()),
            handler: Rc::new(move |ctx| Box::pin(cycle(registered.clone(), chosen.clone(), ctx))),
        },
    )
}

/// Loads configurations and applies the flag before session startup completes.
async fn start(
    api: ExtensionAPI,
    state: Rc<RefCell<State>>,
    ctx: ExtensionContext,
) -> ExtensionResult<()> {
    state.borrow_mut().presets = config::load(&api, &ctx.cwd()?)?;
    restore(&ctx, &mut state.borrow_mut())?;
    if let Some(FlagValue::String(name)) = api.get_flag("preset")?
        && !name.is_empty()
    {
        state.borrow_mut().active = None;
        if state.borrow().presets.contains_key(&name) {
            return choose(&api, &state, &name, &ctx).await;
        }
        unknown(&ctx, &state.borrow(), &name, NotificationType::Warning)?;
    }
    status(&ctx, &state.borrow())
}

/// Registers instruction injection and persisted choices on the current session branch.
fn register_state(api: &ExtensionAPI, state: &Rc<RefCell<State>>) -> ExtensionResult<()> {
    let injecting = state.clone();
    api.on(
        "before_agent_start",
        Rc::new(move |event, _| {
            let answer = if let ExtensionEvent::BeforeAgentStart(event) = event {
                injecting
                    .borrow()
                    .active
                    .as_ref()
                    .and_then(|(_, preset)| preset.instructions.as_ref())
                    .filter(|text| !text.is_empty())
                    .map(|instructions| {
                        ExtensionEventResult::BeforeAgentStart(BeforeAgentStartEventResult {
                            message: Presence::Missing,
                            system_prompt: Presence::Present(format!(
                                "{}\n\n{instructions}",
                                event.system_prompt
                            )),
                        })
                    })
            } else {
                None
            };
            Box::pin(std::future::ready(Ok(answer)))
        }),
    )?;
    let (saving, entries) = (state.clone(), api.clone());
    api.on(
        "turn_start",
        Rc::new(move |_, _| {
            let name = saving
                .borrow()
                .active
                .as_ref()
                .map(|(name, _)| name.clone());
            let result = if name.is_some() || saving.borrow().cleared {
                entries
                    .append_entry("preset-state", Some(serde_json::json!({"name":name})))
                    .map(|()| saving.borrow_mut().cleared = false)
            } else {
                Ok(())
            };
            Box::pin(std::future::ready(result.map(|()| None)))
        }),
    )?;
    let restoring = state.clone();
    api.on(
        "session_tree",
        Rc::new(move |_, ctx| {
            let result = restore(&ctx, &mut restoring.borrow_mut());
            let result = result.and_then(|()| status(&ctx, &restoring.borrow()));
            Box::pin(std::future::ready(result.map(|()| None)))
        }),
    )
}

/// Registers the preset commands and session hooks.
///
/// # Errors
/// Returns a host rejection unchanged.
#[must_use]
pub fn factory(api: ExtensionAPI) -> ExtensionFuture<'static, ()> {
    Box::pin(async move {
        let state = Rc::new(RefCell::new(State::default()));
        register_controls(&api, &state)?;
        register_state(&api, &state)?;
        let (starting, loading) = (state, api.clone());
        api.on(
            "session_start",
            Rc::new(move |_, ctx| {
                let (state, api) = (starting.clone(), loading.clone());
                Box::pin(async move {
                    start(api, state, ctx).await?;
                    Ok(None)
                })
            }),
        )
    })
}

#[cfg(target_arch = "wasm32")]
maestro_extensions_wasm::export_extension!(factory);
