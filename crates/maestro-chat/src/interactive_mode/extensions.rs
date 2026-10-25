//! The current session's extension binding, and extension shortcut dispatch through the
//! editor's first input interceptor.

use super::InteractiveMode;
use indexmap::IndexMap;
use maestro_app::{ExtensionRunner, ExtensionShortcut, Local};
use maestro_tui::{KeyId, matches_key};
use std::{
    rc::Rc,
    sync::{PoisonError, RwLock},
};

/// The shortcuts one selection keeps, by normalized key in selection order.
type Selected = IndexMap<KeyId, Rc<RwLock<ExtensionShortcut<Local>>>>;

impl InteractiveMode {
    /// Installs the editor's first input interceptor over the shortcuts `runner` selects
    /// from the configured keys, clearing a previous interceptor when it selects none.
    pub(super) fn setup_extension_shortcuts(self: &Rc<Self>, runner: &ExtensionRunner<Local>) {
        let selected = runner.get_shortcuts(&self.keybindings.get_effective_config());
        if selected.is_empty() {
            self.editor.set_on_extension_shortcut(None);
        } else {
            self.editor
                .set_on_extension_shortcut(Some(shortcut_interceptor(self, selected)));
        }
    }
    /// Installs this mode as the current session's extension user interface, then waits
    /// for the session start and the resources its extensions discover.
    ///
    /// The session and its runner only borrow the mode, which already owns them; a UI
    /// context an extension obtains keeps the mode alive.
    ///
    /// # Errors
    /// Returns the session start or resource discovery failure.
    pub(super) async fn bind_current_session_extensions(
        self: &Rc<Self>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let host = maestro_app::ExtensionUIContext::new(self.clone());
        self.runtime_host
            .session()
            .bind_extensions(maestro_app::ExtensionBindings {
                ui_context: Some(maestro_app::ExtensionUIContextBinding::borrowed(&host)),
                ..maestro_app::ExtensionBindings::default()
            })
            .await?;
        Ok(())
    }
}

/// The interceptor that starts the first selected handler matching the typed input.
///
/// The mode is reached weakly, so an installed or retired interceptor keeps it alive only
/// through work already started, and each dispatch builds its context from the current
/// runner rather than from a context captured when the interceptor was installed.
fn shortcut_interceptor(
    mode: &Rc<InteractiveMode>,
    selected: Selected,
) -> Rc<dyn Fn(&str) -> bool> {
    let stored = Rc::downgrade(mode);
    Rc::new(move |data: &str| {
        let Some(mode) = stored.upgrade() else {
            return false;
        };
        let Some((_, shortcut)) = selected.iter().find(|(key, _)| matches_key(data, key)) else {
            return false;
        };
        let handler = Rc::clone(
            &shortcut
                .read()
                .unwrap_or_else(PoisonError::into_inner)
                .handler,
        );
        let runner = mode.runtime_host.session().extension_runner();
        let runtime = Rc::clone(&mode.tool_environment.runtime);
        let work = handler(runner.create_context());
        runtime.spawn_local(Box::pin(async move {
            if let Err(error) = work.await {
                mode.show_error(&format!("Shortcut handler error: {error}"))?;
            }
            Ok(())
        }));
        true
    })
}
