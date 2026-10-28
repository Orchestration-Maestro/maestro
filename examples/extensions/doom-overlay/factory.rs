//! Command registration and the retained game instance.
#![forbid(
    clippy::pedantic,
    clippy::too_many_arguments,
    clippy::excessive_nesting
)]
use super::{doom_component::DoomOverlayComponent, doom_engine::DoomEngine, doom_keys::DoomKeys};
use maestro_extensions_wasm::{
    Background, CommandOptions, Component, CustomFactoryOutcome, CustomUIOptions, ExtensionAPI,
    ExtensionCommandContext, ExtensionComponent, ExtensionFuture, ExtensionResult,
    ExtensionUIContext, OverlayAnchor, OverlayMargin, OverlayMarginValue, OverlayOptions,
    OverlayOptionsHandle, OverlayOptionsSource, SizeValue, tools::path_utils,
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
/// Retained game selection.
type Active = Rc<RefCell<Option<ActiveGame>>>;

/// Resolves supplied WAD bytes only when a new engine is needed.
async fn play(
    api: ExtensionAPI,
    background: Background,
    active: Active,
    args: String,
    ctx: ExtensionCommandContext,
) -> ExtensionResult<()> {
    let wad = path_utils::resolve_to_cwd(&api, args.trim(), &ctx.cwd()?)?;
    let existing = active
        .borrow()
        .as_ref()
        .map(|game| (game.wad.clone(), game.engine.clone()));
    let (engine, resume) = if let Some((old, engine)) = existing {
        if old != wad {
            return Err("Reload the extension before changing WAD files".into());
        }
        (engine, true)
    } else {
        let game = initialize(&api, wad)?;
        let engine = game.engine.clone();
        active.replace(Some(game));
        (engine, false)
    };
    show(api, background, engine, ctx.ui()?, resume).await
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
    let Some(game) = active.borrow_mut().take() else {
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
        let sender = api.clone();
        api.register_command(
            "doom-overlay",
            CommandOptions {
                description: Some("Play DOOM in an overlay. Q pauses and closes it.".into()),
                get_argument_completions: None,
                handler: Rc::new(move |args, context| {
                    Box::pin(play(
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
