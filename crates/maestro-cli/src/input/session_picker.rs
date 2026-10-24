//! The session selector presented over supplied discovery loaders.

use super::selector::{Running, SelectorHost, exit_notification, settlement};
use maestro_app::KeybindingsManager as AppKeybindingsManager;
use maestro_app::presentation_data::input_selectors::{
    SessionSelectorCallbacks, SessionSelectorFactory, SessionSelectorRequest, SessionsLoader,
};
use maestro_tui::set_keybindings;
use std::io;
use std::ops::Deref;
use std::rc::Rc;

/// The configured bindings, or the supplied ones.
#[cfg(not(target_arch = "wasm32"))]
fn configured_bindings(
    supplied: Option<AppKeybindingsManager>,
) -> io::Result<AppKeybindingsManager> {
    supplied.map_or_else(|| AppKeybindingsManager::create(None), Ok)
}

/// The supplied bindings, or in-memory defaults where no configuration file is read.
#[cfg(target_arch = "wasm32")]
fn configured_bindings(
    supplied: Option<AppKeybindingsManager>,
) -> io::Result<AppKeybindingsManager> {
    Ok(supplied.unwrap_or_default())
}

/// The prepared presentation of the session selector.
pub struct SessionPresentation {
    /// The frame writer and the exit effect.
    pub host: SelectorHost,
    /// Retained configured bindings, or the application defaults.
    pub keybindings: Option<AppKeybindingsManager>,
    /// Builds the presented component.
    pub presenter: SessionSelectorFactory,
}

/// Show the session selector and return the selected path, or `None` when cancelled.
///
/// The configured bindings are installed before the presentation, so the search
/// field and the list dispatch the same configured controls.
///
/// Selection, cancellation and exit settle once; a later notification does nothing.
///
/// # Errors
/// Returns configuration loading, frame-writer creation, presentation and start failures.
pub async fn select_session(
    current_sessions_loader: SessionsLoader,
    all_sessions_loader: SessionsLoader,
    presentation: SessionPresentation,
) -> io::Result<Option<String>> {
    let (running, settled) = Running::new(None);
    let keybindings = configured_bindings(presentation.keybindings)?;
    set_keybindings(keybindings.deref().clone());
    let ui = (presentation.host.create_ui)()?;
    running.attach(ui.clone());
    let selecting = Rc::downgrade(&running);
    let request = SessionSelectorRequest {
        current_sessions_loader,
        all_sessions_loader,
        keybindings: keybindings.deref().clone(),
        show_rename_hint: false,
        request_render: ui.weak_render_request(),
    };
    let callbacks = SessionSelectorCallbacks {
        on_select: Rc::new(move |path: &str| {
            if let Some(running) = selecting.upgrade() {
                running.settle(Some(path.to_owned()));
            }
        }),
        on_cancel: settlement(&running, None),
        on_exit: exit_notification(&running, Rc::clone(&presentation.host.exit)),
    };
    let presented = (presentation.presenter)(request, callbacks)?;
    ui.add_child(presented.component);
    ui.set_focus(Some(presented.focus));
    ui.start()?;
    Ok(settled.await.unwrap_or(None))
}
