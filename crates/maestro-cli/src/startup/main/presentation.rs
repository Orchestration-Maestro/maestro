//! The confirmation dialog for an unavailable stored working directory.
use crate::input::selector::{Running, settlement};
use maestro_app::presentation_data::input_selectors::{
    ExtensionSelectorFactory, ExtensionSelectorRequest,
};
use maestro_app::{
    KeybindingsManager, SessionCwdIssue, SettingsManager, format_missing_session_cwd_prompt,
};
use maestro_theme::ThemeState;
use std::{cell::RefCell, io, rc::Rc};

/// The supplied writer, theme and component factory of the missing-directory dialog.
pub struct MissingSessionCwdPresentation {
    /// Creates the writer with the supplied hardware-cursor policy.
    pub create_ui: Rc<dyn Fn(bool) -> io::Result<maestro_tui::TUI>>,
    /// Theme state initialized before presentation.
    pub theme: Rc<ThemeState>,
    /// Supplied selection-component factory.
    pub presenter: ExtensionSelectorFactory,
}

/// Present Continue and Cancel, returning the fallback directory only for Continue.
///
/// # Errors
/// Returns theme, keybinding, writer creation, component or start failures.
pub async fn prompt_for_missing_session_cwd(
    issue: &SessionCwdIssue,
    settings: Rc<RefCell<SettingsManager>>,
    presentation: MissingSessionCwdPresentation,
) -> io::Result<Option<String>> {
    let (theme, show_cursor, clear_on_shrink) = {
        let settings = settings.borrow();
        (
            settings.get_theme(),
            settings.get_show_hardware_cursor(),
            settings.get_clear_on_shrink(),
        )
    };
    presentation
        .theme
        .init_theme(theme.as_deref(), None)
        .map_err(io::Error::other)?;
    let keys = KeybindingsManager::create(None)?;
    maestro_tui::set_keybindings((*keys).clone());
    let (running, settled) = Running::new(None);
    let ui = (presentation.create_ui)(show_cursor)?;
    running.attach(ui.clone());
    ui.set_clear_on_shrink(clear_on_shrink);
    let selecting = Rc::downgrade(&running);
    let fallback = issue.fallback_cwd.clone();
    let request = ExtensionSelectorRequest {
        title: format_missing_session_cwd_prompt(issue),
        options: vec!["Continue".into(), "Cancel".into()],
        on_select: Rc::new(move |selected| {
            if let Some(running) = selecting.upgrade() {
                running.settle((selected == "Continue").then(|| fallback.clone()));
            }
        }),
        on_cancel: settlement(&running, None),
        tui: ui.clone(),
    };
    let presented = (presentation.presenter)(request)?;
    ui.add_child(presented.component);
    ui.set_focus(Some(presented.focus));
    ui.start()?;
    Ok(settled.await.unwrap_or(None))
}
