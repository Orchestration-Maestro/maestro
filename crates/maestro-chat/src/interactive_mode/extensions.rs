//! The current session's extension binding, and extension shortcut dispatch through the
//! editor's first input interceptor.

use super::InteractiveMode;
use indexmap::IndexMap;
use maestro_app::{ExtensionRunner, ExtensionShortcut, Local, ReadonlySessionManager};
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
    /// Applies the current session's footer, thinking, terminal and editor settings, the
    /// editor settings to the default editor first and then to a distinct current editor.
    ///
    /// # Errors
    /// Returns the cursor-hiding failure or the current editor's original failure; the
    /// settings before it stay applied.
    pub(super) fn apply_runtime_settings(&self) -> Result<(), Box<dyn std::error::Error>> {
        let session = self.runtime_host.session();
        self.footer.set_session(Rc::clone(&session));
        self.footer
            .set_auto_compact_enabled(session.auto_compaction_enabled());
        let cwd = session.session_manager().borrow().get_cwd().to_owned();
        self.footer_data_provider.set_cwd(cwd);
        let settings = session.settings_manager().borrow();
        self.hide_thinking_block
            .set(settings.get_hide_thinking_block());
        self.ui
            .set_show_hardware_cursor(settings.get_show_hardware_cursor())?;
        self.apply_clear_on_shrink(settings.get_clear_on_shrink());
        let padding = settings.get_editor_padding_x();
        let max_visible = settings.get_autocomplete_max_visible();
        drop(settings);
        self.editor.set_padding_x(padding);
        self.editor.set_autocomplete_max_visible(max_visible);
        if self.custom_editor_current() {
            self.current_editor()
                .try_set_padding_x(padding)
                .map_err(erase)?;
        }
        if self.custom_editor_current() {
            self.current_editor()
                .try_set_autocomplete_max_visible(max_visible)
                .map_err(erase)?;
        }
        Ok(())
    }
    /// Installs this mode as the current session's extension user interface, with its
    /// shutdown and error handlers, and waits for the session start and the resources its
    /// extensions discover; then publishes the loaded themes, rebuilds completion, selects
    /// the shortcuts, lists the loaded resources and shows the startup notices.
    ///
    /// The session and its runner only borrow the mode, which already owns them; a UI
    /// context an extension obtains keeps the mode alive. A shutdown request while idle
    /// starts the graceful shutdown; while streaming, the agent's end starts it.
    ///
    /// # Errors
    /// Returns the session start, resource discovery, completion, listing or notice failure.
    pub(super) async fn bind_current_session_extensions(
        self: &Rc<Self>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let host = maestro_app::ExtensionUIContext::new(self.clone());
        let (requester, reporter, aborter) = (
            Rc::downgrade(self),
            Rc::downgrade(self),
            Rc::downgrade(self),
        );
        self.runtime_host
            .session()
            .bind_extensions(maestro_app::ExtensionBindings {
                ui_context: Some(maestro_app::ExtensionUIContextBinding::borrowed(&host)),
                command_context_actions: Some(self.command_context_actions()),
                abort_handler: Some(Rc::new(move || {
                    if let Some(mode) = aborter.upgrade() {
                        let restored = mode.restore_queued_messages_to_editor(
                            super::queues::RestoreQueuedMessagesOptions {
                                abort: true,
                                ..Default::default()
                            },
                        );
                        mode.report(restored.map(drop));
                    }
                })),
                shutdown_handler: Some(Rc::new(move || {
                    let Some(mode) = requester.upgrade() else {
                        return Ok(());
                    };
                    mode.shutdown_requested.set(true);
                    if !mode.runtime_host.session().is_streaming() {
                        mode.spawn_shutdown();
                    }
                    Ok(())
                })),
                on_error: Some(Rc::new(move |error| {
                    if let Some(mode) = reporter.upgrade() {
                        let stack = error.stack.as_deref();
                        mode.show_extension_error(&error.extension_path, &error.error, stack)?;
                    }
                    Ok(())
                })),
            })
            .await?;
        let session = self.runtime_host.session();
        self.themes
            .set_registered_themes(session.resource_loader().get_themes().themes.clone());
        self.setup_autocomplete_provider().map_err(erase)?;
        self.setup_extension_shortcuts(&session.extension_runner());
        self.show_loaded_resources(super::resources::ShowLoadedResourcesOptions {
            show_diagnostics_when_quiet: true,
            ..super::resources::ShowLoadedResourcesOptions::default()
        })?;
        self.show_startup_notices_if_needed()
    }
    /// Removes the session subscription, applies the settings, binds the session's
    /// extensions, subscribes again, then refreshes the provider count, the editor border
    /// and the terminal title.
    ///
    /// # Errors
    /// Returns the first failing phase's error; the phases before it stay applied.
    pub(super) async fn rebind_current_session(
        self: &Rc<Self>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        if let Some(unsubscribe) = self.unsubscribe.take() {
            unsubscribe();
        }
        self.apply_runtime_settings()?;
        self.bind_current_session_extensions().await?;
        self.subscribe_to_agent();
        self.update_available_provider_count()?;
        self.update_editor_border_color()?;
        self.update_terminal_title().map_err(erase)
    }
    /// Appends an extension failure to the transcript in the error colour, then its stack
    /// below without the first line, each frame trimmed, indented and dimmed.
    ///
    /// # Errors
    /// Returns the published theme's failure.
    fn show_extension_error(
        &self,
        extension_path: &str,
        error: &str,
        stack: Option<&str>,
    ) -> Result<(), maestro_theme::ThemeError> {
        let theme = self.themes.theme().get()?;
        let heading = format!("Extension \"{extension_path}\" error: {error}");
        let heading = theme.fg(&maestro_theme::ThemeColor::Error, &heading)?;
        self.chat_container
            .add_child(Rc::new(maestro_tui::Text::new(heading, 1, 0, None)));
        if let Some(stack) = stack.filter(|stack| !stack.is_empty()) {
            let frames = stack
                .split('\n')
                .skip(1)
                .map(|line| {
                    let line = line.trim_matches(maestro_app::is_whitespace_scalar);
                    theme.fg(&maestro_theme::ThemeColor::Dim, &format!("  {line}"))
                })
                .collect::<Result<Vec<_>, _>>()?
                .join("\n");
            if !frames.is_empty() {
                self.chat_container
                    .add_child(Rc::new(maestro_tui::Text::new(frames, 1, 0, None)));
            }
        }
        self.ui.request_render(false);
        Ok(())
    }
}

/// The same failure behind the frontend's error type.
fn erase(error: maestro_app::EventError) -> Box<dyn std::error::Error> {
    error
}

/// Installs the extension UI reset that runs before each session invalidation and the
/// replacement continuation on the runtime, both reaching `mode` weakly, so the runtime
/// the mode owns does not keep it alive.
pub(super) fn bind_runtime_hooks(mode: &Rc<InteractiveMode>) {
    let reset = Rc::downgrade(mode);
    mode.runtime_host
        .set_before_session_invalidate(Some(Rc::new(move || {
            reset
                .upgrade()
                .map_or(Ok(()), |mode| mode.reset_extension_ui())
        })));
    mode.runtime_host
        .set_rebind_session(Some(rebind_callback(Rc::downgrade(mode))));
}

/// The runtime's replacement continuation: rebinds the mode, while it is alive, before
/// the runtime continues with the replacement session.
fn rebind_callback(mode: std::rc::Weak<InteractiveMode>) -> maestro_app::RebindSession {
    Rc::new(move |_session| {
        let mode = mode.upgrade();
        Box::pin(async move {
            let Some(mode) = mode else {
                return Ok(());
            };
            mode.rebind_current_session().await.map_err(|error| {
                error
                    .downcast::<maestro_app::DiagnosticErrorInfo>()
                    .map_or_else(
                        |error| maestro_app::DiagnosticErrorInfo::error(error.to_string()),
                        |error| *error,
                    )
            })
        })
    })
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
