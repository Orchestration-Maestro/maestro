//! Command registration and the retained game instance.
#![forbid(
    clippy::pedantic,
    clippy::too_many_arguments,
    clippy::excessive_nesting
)]
use super::{
    doom_component::DoomOverlayComponent, doom_engine::DoomEngine, doom_keys::DoomKeys, doom_state,
    wad_finder,
};
use maestro_extensions_wasm::{
    Background, CommandOptions, Component, CustomFactoryOutcome, CustomUIOptions, ExtensionAPI,
    ExtensionCommandContext, ExtensionComponent, ExtensionContext, ExtensionFuture,
    ExtensionResult, ExtensionUIContext, NotificationType, OverlayAnchor, OverlayMargin,
    OverlayMarginValue, OverlayOptions, OverlayOptionsHandle, OverlayOptionsSource, SizeValue,
    tools::path_utils,
};
use std::{cell::RefCell, rc::Rc};

/// The WAD choice and its live engine survive closing a view.
struct ActiveGame {
    /// Lexically resolved native WAD path.
    wad: String,
    /// Writable guest path belonging only to this engine instance.
    root: String,
    /// The one engine initialized in this component.
    engine: Rc<DoomEngine>,
}
/// Engine ownership and whether any failed operation requires a replacement.
#[derive(Default)]
struct Instance {
    /// The initialized engine and its private files.
    game: Option<ActiveGame>,
    /// Prevents a partially initialized or failed component from being reused.
    invalid: bool,
}
/// Retained engine state for one composed instance.
type Active = Rc<RefCell<Instance>>;

/// Keeps command errors visible without escaping the game UI boundary.
async fn command(
    api: ExtensionAPI,
    background: Background,
    active: Active,
    args: String,
    ctx: ExtensionCommandContext,
) -> ExtensionResult<()> {
    let ui = ctx.ui()?;
    if !ctx.has_ui()? {
        return ui.notify(
            "DOOM requires interactive mode",
            Some(NotificationType::Error),
        );
    }
    ui.notify("Loading DOOM...", Some(NotificationType::Info))?;
    let Some(wad) = selected_wad(&api, &args, &ctx).await? else {
        return Ok(());
    };
    let replace = {
        let instance = active.borrow();
        instance.invalid
            || instance
                .game
                .as_ref()
                .is_some_and(|game| game.wad != wad || game.engine.has_failed())
    };
    if replace {
        doom_state::request(&api, &wad)?;
        cleanup(&active)?;
        return ctx.reload().await;
    }
    report_open(api, background, active, wad, (*ctx).clone()).await
}

/// Resolves an explicit or remembered choice before considering a download.
async fn selected_wad(
    api: &ExtensionAPI,
    args: &str,
    ctx: &ExtensionContext,
) -> ExtensionResult<Option<String>> {
    let cwd = ctx.cwd()?;
    if !args.trim().is_empty() {
        return path_utils::resolve_to_cwd(api, args.trim(), &cwd).map(Some);
    }
    if let Some(selection) = doom_state::saved(ctx)? {
        return Ok(Some(selection.wad));
    }
    if let Ok(wad) = wad_finder::ensure_wad_file(api, &cwd).await {
        Ok(Some(wad))
    } else {
        ctx.ui()?.notify(
            "Failed to download DOOM WAD file. Check your internet connection.",
            Some(NotificationType::Error),
        )?;
        Ok(None)
    }
}

/// Reports a failed open while preventing later reuse of that instance.
async fn report_open(
    api: ExtensionAPI,
    background: Background,
    active: Active,
    wad: String,
    ctx: ExtensionContext,
) -> ExtensionResult<()> {
    let ui = ctx.ui()?;
    let failed = active.clone();
    if let Err(error) = open(api, background, active, wad, ctx).await {
        failed.borrow_mut().invalid = true;
        ui.notify(
            &format!("Failed to load DOOM: {error}"),
            Some(NotificationType::Error),
        )?;
    }
    Ok(())
}

/// Opens or resumes a healthy retained engine.
async fn open(
    api: ExtensionAPI,
    background: Background,
    active: Active,
    wad: String,
    ctx: ExtensionContext,
) -> ExtensionResult<()> {
    let existing = active
        .borrow()
        .game
        .as_ref()
        .map(|game| game.engine.clone());
    let (engine, resume) = if let Some(engine) = existing {
        ctx.ui()?
            .notify("Resuming DOOM...", Some(NotificationType::Info))?;
        (engine, true)
    } else {
        ctx.ui()?.notify(
            &format!("Loading DOOM from {wad}..."),
            Some(NotificationType::Info),
        )?;
        let game = initialize(&api, wad.clone())?;
        let engine = game.engine.clone();
        active.borrow_mut().game = Some(game);
        doom_state::remember(&api, &wad)?;
        (engine, false)
    };
    show(api, background, engine, ctx.ui()?, resume).await
}

/// Consumes a request before opening it, including when initialization fails.
async fn pending(
    api: ExtensionAPI,
    background: Background,
    active: Active,
    ctx: ExtensionContext,
) -> ExtensionResult<()> {
    let Some(selection) = doom_state::saved(&ctx)? else {
        return Ok(());
    };
    if !selection.pending {
        return Ok(());
    }
    doom_state::consume(&api, &selection)?;
    if !ctx.has_ui()? {
        return Ok(());
    }
    ctx.ui()?
        .notify("Loading DOOM...", Some(NotificationType::Info))?;
    report_open(api, background, active, selection.wad, ctx).await
}

/// Creates one private root and removes partial files if initialization fails.
fn initialize(api: &ExtensionAPI, wad: String) -> ExtensionResult<ActiveGame> {
    let bytes = std::fs::read(path_utils::guest_path(api, &wad)?).map_err(|e| e.to_string())?;
    let root = path_utils::guest_path(api, &path_utils::make_temp_dir(api, "maestro-doom")?)?;
    match DoomEngine::new(&bytes, &root) {
        Ok(engine) => Ok(ActiveGame {
            wad,
            root,
            engine: Rc::new(engine),
        }),
        Err(error) => {
            std::fs::remove_dir_all(root).map_err(|cleanup| {
                format!("{error}; removing private engine files failed: {cleanup}")
            })?;
            Err(error)
        }
    }
}

/// Deletes the retained engine's files on session shutdown.
fn cleanup(active: &Active) -> ExtensionResult<()> {
    let Some(game) = active.borrow_mut().game.take() else {
        return Ok(());
    };
    std::fs::remove_dir_all(game.root).map_err(|error| error.to_string())
}

/// Shows one centered overlay; its disposer owns the ticking task's end.
async fn show(
    api: ExtensionAPI,
    background: Background,
    engine: Rc<DoomEngine>,
    ui: ExtensionUIContext,
    resume: bool,
) -> ExtensionResult<()> {
    let placement = overlay_placement()?;
    let notifications = ui.clone();
    ui.custom(
        Rc::new(move |tui, _, _, done, _| {
            let (api, background, engine, ui) = (
                api.clone(),
                background.clone(),
                engine.clone(),
                notifications.clone(),
            );
            Ok(CustomFactoryOutcome::Async(Box::pin(async move {
                if resume {
                    engine.push_key(true, DoomKeys::KEY_PAUSE);
                    engine.push_key(false, DoomKeys::KEY_PAUSE);
                }
                let view = DoomOverlayComponent::new(api, tui, engine, done, ui);
                view.start(&background).await?;
                let disposal = view.clone();
                Ok(ExtensionComponent {
                    component: Component::new(view),
                    dispose: Some(Rc::new(move |_| disposal.close())),
                    set_expanded: None,
                    width: None,
                })
            })))
        }),
        Some(CustomUIOptions {
            overlay: Some(true),
            overlay_options: Some(OverlayOptionsSource::Static(placement)),
            on_handle: None,
        }),
    )
    .await?;
    Ok(())
}

/// Registers the opt-in command and the shared background owner.
/// # Errors
/// Returns a registration failure.
#[must_use]
pub fn factory(api: ExtensionAPI) -> ExtensionFuture<'static, ()> {
    Box::pin(async move {
        let background = Background::new(api.events())?;
        let active: Active = Rc::default();
        let shutdown = active.clone();
        api.on(
            "session_shutdown",
            Rc::new(move |_, _| {
                let result = cleanup(&shutdown);
                Box::pin(async move { result.map(|()| None) })
            }),
        )?;
        let (starting_api, starting_background, starting_active) =
            (api.clone(), background.clone(), active.clone());
        api.on(
            "session_start",
            Rc::new(move |_, ctx| {
                let (api, background, active) = (
                    starting_api.clone(),
                    starting_background.clone(),
                    starting_active.clone(),
                );
                Box::pin(async move {
                    pending(api, background, active, ctx).await?;
                    Ok(None)
                })
            }),
        )?;
        let sender = api.clone();
        api.register_command(
            "doom-overlay",
            CommandOptions {
                description: Some("Play DOOM in an overlay. Q pauses and closes it.".into()),
                get_argument_completions: None,
                handler: Rc::new(move |args, context| {
                    Box::pin(command(
                        sender.clone(),
                        background.clone(),
                        active.clone(),
                        args,
                        context,
                    ))
                }),
            },
        )
    })
}

/// Creates the shared placement retained by the host.
fn overlay_placement() -> ExtensionResult<OverlayOptionsHandle> {
    OverlayOptionsHandle::new(OverlayOptions {
        width: Some(SizeValue::Percentage("75%".into())),
        max_height: Some(SizeValue::Percentage("95%".into())),
        anchor: Some(OverlayAnchor::Center),
        margin: Some(OverlayMarginValue::Sides(OverlayMargin {
            top: Some(1),
            ..OverlayMargin::default()
        })),
        ..OverlayOptions::default()
    })
}
