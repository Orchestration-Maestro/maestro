//! The session selector presented over supplied discovery loaders.

use super::selector::{Running, SelectorHost, exit_notification, settlement};
use maestro_app::KeybindingsManager as AppKeybindingsManager;
use maestro_app::presentation_data::input_selectors::{
    SessionSelectorCallbacks, SessionSelectorFactory, SessionSelectorRequest, SessionsLoader,
};
use std::io;
use std::ops::Deref;
use std::rc::Rc;

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
/// Selection, cancellation and exit settle once; a later notification does nothing.
///
/// # Errors
/// Returns frame-writer creation, presentation and start failures.
pub async fn select_session(
    current_sessions_loader: SessionsLoader,
    all_sessions_loader: SessionsLoader,
    presentation: SessionPresentation,
) -> io::Result<Option<String>> {
    let ui = (presentation.host.create_ui)()?;
    let (running, settled) = Running::new(ui.clone(), None);
    let selecting = Rc::downgrade(&running);
    let request = SessionSelectorRequest {
        current_sessions_loader,
        all_sessions_loader,
        keybindings: presentation.keybindings.unwrap_or_default().deref().clone(),
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
