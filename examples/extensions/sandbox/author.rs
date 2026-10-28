//! Registers sandbox lifecycle, configuration display and the shared bash operations.
use super::{config, platform::command_output, runtime::Runtime};
use maestro_extensions_wasm::{
    Background, BashOperations, BashToolOptions, CommandOptions, ExtensionAPI,
    ExtensionCommandContext, ExtensionContext, ExtensionEventResult, ExtensionFuture,
    ExtensionResult, FlagOptions, FlagType, FlagValue, NotificationType, Presence,
    UserBashEventResult,
};
use std::{cell::RefCell, rc::Rc};

/// The currently initialized runtime, shared by both bash entry points.
type State = Rc<RefCell<Option<Rc<Runtime>>>>;

/// Registers the opt-in sandbox through the host's original bash factory.
///
/// # Errors
/// Returns the host's registration, directory or tool-construction failure.
#[must_use]
pub fn factory(api: ExtensionAPI) -> ExtensionFuture<'static, ()> {
    Box::pin(async move {
        api.register_flag(
            "no-sandbox",
            FlagOptions {
                description: Some("Disable OS-level sandboxing for bash commands".into()),
                type_: FlagType::Boolean,
                default: Some(FlagValue::Boolean(false)),
            },
        )?;
        let state = State::default();
        let operations = operations(&api, Rc::clone(&state))?;
        register_tool(&api, operations.clone())?;
        register_events(
            &api,
            Rc::clone(&state),
            operations,
            Background::new(api.events())?,
        )?;
        let owner = api.clone();
        api.register_command(
            "sandbox",
            CommandOptions {
                description: Some("Show sandbox configuration".into()),
                get_argument_completions: None,
                handler: Rc::new(move |_, ctx| {
                    Box::pin(std::future::ready(show(&owner, &state, &ctx)))
                }),
            },
        )
    })
}

/// Shows the current configuration without retaining a state borrow across host calls.
fn show(api: &ExtensionAPI, state: &State, ctx: &ExtensionCommandContext) -> ExtensionResult<()> {
    let disabled = state.borrow().is_none();
    let ui = ctx.ui()?;
    if disabled {
        return ui.notify("Sandbox is disabled", Some(NotificationType::Info));
    }
    let config = config::load(api, &ctx.cwd()?)?;
    ui.notify(&config::display(&config), Some(NotificationType::Info))
}

/// Wraps operands only; raw output, cancellation and timeout settlement stay native-owned.
fn operations(api: &ExtensionAPI, state: State) -> ExtensionResult<BashOperations> {
    let local = api.create_local_bash_operations(None)?;
    Ok(BashOperations {
        exec: Rc::new(move |command, cwd, options| {
            let selected = state.borrow().clone();
            let local = local.clone();
            Box::pin(async move {
                let command = match selected {
                    Some(runtime) => runtime.wrap(&command).await?,
                    None => command,
                };
                (local.exec)(command, cwd, options).await
            })
        }),
    })
}

/// Retains the original metadata and renderers while replacing bash execution once.
fn register_tool(api: &ExtensionAPI, operations: BashOperations) -> ExtensionResult<()> {
    let cwd = std::env::current_dir().map_err(|error| error.to_string())?;
    let mut tool = api.create_bash_tool_definition(
        cwd.to_string_lossy().into_owned(),
        BashToolOptions {
            operations: Some(operations),
            ..BashToolOptions::default()
        },
    )?;
    tool.metadata.label = "bash (sandboxed)".into();
    api.register_tool(tool)
}

/// Registers both bash routes and the lifecycle that owns their shared state.
fn register_events(
    api: &ExtensionAPI,
    state: State,
    operations: BashOperations,
    background: Background,
) -> ExtensionResult<()> {
    let enabled = Rc::clone(&state);
    api.on(
        "user_bash",
        Rc::new(move |_, _| {
            let result = enabled.borrow().as_ref().map(|_| {
                ExtensionEventResult::UserBash(UserBashEventResult {
                    operations: Some(operations.clone()),
                    result: Presence::Missing,
                })
            });
            Box::pin(std::future::ready(Ok(result)))
        }),
    )?;
    let (owner, started) = (api.clone(), Rc::clone(&state));
    api.on(
        "session_start",
        Rc::new(move |_, ctx| start(owner.clone(), Rc::clone(&started), background.clone(), ctx)),
    )?;
    api.on(
        "session_shutdown",
        Rc::new(move |_, _| {
            let retired = state.borrow_mut().take();
            Box::pin(async move {
                if let Some(runtime) = retired {
                    runtime.close().await;
                }
                Ok(None)
            })
        }),
    )
}

/// Initialization decisions retain disabled and unsupported states separately from errors.
enum Initialization {
    /// Configuration explicitly disables the wrapper.
    Disabled,
    /// The native operating system has no supported enforcement backend.
    Unsupported(String),
    /// A completely initialized runtime ready for publication.
    Ready(Rc<Runtime>),
}

/// Checks configuration before observing the operating system or creating native resources.
async fn initialize(
    api: ExtensionAPI,
    background: Background,
    ctx: &ExtensionContext,
) -> ExtensionResult<Initialization> {
    let cwd = ctx.cwd()?;
    let config = config::load(&api, &cwd)?;
    if !maestro_json::is_truthy(&config.enabled) {
        return Ok(Initialization::Disabled);
    }
    let platform = command_output(&api, "uname", &["-s".into()]).await?;
    let platform = platform.trim().to_owned();
    if !matches!(platform.as_str(), "Linux" | "Darwin") {
        return Ok(Initialization::Unsupported(platform));
    }
    Runtime::start(api, background, config, cwd, platform)
        .await
        .map(|runtime| Initialization::Ready(Rc::new(runtime)))
}

/// Initializes a replacement only after releasing the previous session's resources.
fn start(
    api: ExtensionAPI,
    state: State,
    background: Background,
    ctx: ExtensionContext,
) -> ExtensionFuture<'static, Option<ExtensionEventResult>> {
    Box::pin(async move {
        let previous = state.borrow_mut().take();
        if let Some(runtime) = previous {
            runtime.close().await;
        }
        let ui = ctx.ui()?;
        ui.set_status("sandbox", None)?;
        let disabled = match api.get_flag("no-sandbox")? {
            Some(FlagValue::Boolean(value)) => value,
            Some(FlagValue::String(value)) => !value.is_empty(),
            None => false,
        };
        if disabled {
            ui.notify(
                "Sandbox disabled via --no-sandbox",
                Some(NotificationType::Warning),
            )?;
            return Ok(None);
        }
        match initialize(api, background, &ctx).await {
            Ok(Initialization::Disabled) => {
                ui.notify("Sandbox disabled via config", Some(NotificationType::Info))?;
            }
            Ok(Initialization::Unsupported(platform)) => ui.notify(
                &format!("Sandbox not supported on {platform}"),
                Some(NotificationType::Warning),
            )?,
            Err(error) => ui.notify(
                &format!("Sandbox initialization failed: {error}"),
                Some(NotificationType::Error),
            )?,
            Ok(Initialization::Ready(runtime)) => publish(runtime, &state, &ctx)?,
        }
        Ok(None)
    })
}

/// Publishes a runtime only after its visible initialization succeeds.
fn publish(runtime: Rc<Runtime>, state: &State, ctx: &ExtensionContext) -> ExtensionResult<()> {
    let ui = ctx.ui()?;
    if ctx.has_ui()? {
        let text = format!(
            "Sandbox: {} domains, {} write paths",
            runtime.config.network.allowed_domains.len(),
            runtime.config.filesystem.allow_write.len()
        );
        let text = ui.theme()?.fg("accent", &text)?;
        ui.set_status("sandbox", Some(&text))?;
    }
    ui.notify("Sandbox initialized", Some(NotificationType::Info))?;
    state.replace(Some(runtime));
    Ok(())
}
