//! The configuration selector presented over supplied resolved paths.

use super::selector::{Running, SelectorHost, exit_notification, settlement};
pub use maestro_app::presentation_data::input_selectors::ConfigSelectorOptions;
use maestro_app::presentation_data::input_selectors::{
    ConfigSelectorCallbacks, ConfigSelectorFactory,
};
use maestro_theme::{ThemeState, ThemeWatchOperations};
use std::io;
use std::rc::Rc;

/// The prepared presentation of the configuration selector.
pub struct ConfigPresentation {
    /// The frame writer and the exit effect.
    pub host: SelectorHost,
    /// The state whose selected theme is published for the presentation.
    pub theme: Rc<ThemeState>,
    /// Watch effects the selected theme starts.
    pub watcher: Rc<dyn ThemeWatchOperations>,
    /// Builds the presented component.
    pub presenter: ConfigSelectorFactory,
}

/// Show the configuration selector until it closes.
///
/// The selected theme from the supplied settings is published before the frame
/// writer starts and its watch stops with the presentation.
///
/// # Errors
/// Returns theme selection, frame-writer creation, presentation and start failures.
pub async fn select_config(
    options: ConfigSelectorOptions,
    presentation: ConfigPresentation,
) -> io::Result<()> {
    let theme = options.settings_manager.borrow().get_theme();
    presentation
        .theme
        .init_theme(theme.as_deref(), Some(Rc::clone(&presentation.watcher)))
        .map_err(io::Error::other)?;
    let (running, settled) = Running::new(Some(Rc::clone(&presentation.theme)));
    let ui = (presentation.host.create_ui)()?;
    running.attach(ui.clone());
    let callbacks = ConfigSelectorCallbacks {
        on_close: settlement(&running, ()),
        on_exit: exit_notification(&running, Rc::clone(&presentation.host.exit)),
        request_render: ui.weak_render_request(),
    };
    let presented = (presentation.presenter)(options, callbacks)?;
    ui.add_child(presented.component);
    ui.set_focus(Some(presented.focus));
    ui.start()?;
    let _ = settled.await;
    Ok(())
}
