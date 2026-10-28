//! Selector factories over the existing chat components and native frame writer.
use maestro_app::presentation_data::input_selectors::{
    ExtensionSelectorFactory, PresentedSelector,
};
use maestro_chat::components::{
    config_selector::{ConfigSelectorComponent, ConfigSelectorOptions},
    extension_selector::{ExtensionSelectorComponent, ExtensionSelectorOptions},
    session_selector::{SessionSelectorComponent, SessionSelectorOptions},
};
use maestro_cli::{
    input::{
        config_selector::ConfigPresentation, selector::SelectorHost,
        session_picker::SessionPresentation,
    },
    startup::main::presentation::MissingSessionCwdPresentation,
};
use maestro_theme::{Collator, NativeThemeWatchOperations, ThemeState, supports_color};
use maestro_tui::TUI;
#[cfg(unix)]
use maestro_tui::TerminalImage;
use std::{io, rc::Rc};
use tokio::task::LocalSet;

/// The frame-writer factory shared by both selector hosts.
fn writer(local: Rc<LocalSet>) -> Rc<dyn Fn(Option<bool>) -> io::Result<TUI>> {
    #[cfg(unix)]
    {
        Rc::new(move |cursor| {
            let (terminal, runtime) =
                maestro_cli::input::selector::native_terminal(Rc::clone(&local));
            Ok(TUI::new(
                terminal,
                runtime,
                TerminalImage::default(),
                cursor,
            ))
        })
    }
    #[cfg(not(unix))]
    {
        drop(local);
        Rc::new(|_| Err(crate::modes::unsupported_terminal()))
    }
}

/// The native writer and explicit process-exit operation of a selector.
fn selector(local: Rc<LocalSet>) -> SelectorHost {
    let create = writer(local);
    SelectorHost {
        create_ui: Rc::new(move || create(None)),
        exit: Rc::new(|code| std::process::exit(code)),
    }
}

/// The package configuration component over supplied resolved resources and settings.
pub fn config(local: Rc<LocalSet>, theme: Rc<ThemeState>) -> ConfigPresentation {
    let palette = theme.theme();
    ConfigPresentation {
        host: selector(Rc::clone(&local)),
        theme,
        watcher: Rc::new(NativeThemeWatchOperations::new(local)),
        presenter: Rc::new(move |request, callbacks| {
            let component = Rc::new(ConfigSelectorComponent::new(
                request.resolved_paths,
                request.settings_manager,
                ConfigSelectorOptions {
                    cwd: request.cwd,
                    agent_dir: request.agent_dir,
                    theme: palette.clone(),
                    collator: Rc::new(Collator::new(&|key| std::env::var(key).ok())?),
                    styles_enabled: Rc::new(supports_color),
                },
                callbacks,
            ));
            Ok(PresentedSelector {
                focus: component.get_resource_list().clone(),
                component,
            })
        }),
    }
}

/// The session selector over the application's supplied scope loaders.
pub fn sessions(local: Rc<LocalSet>, theme: Rc<ThemeState>) -> SessionPresentation {
    let runtime = Rc::new(maestro_tui_crossterm::ProcessTuiRuntime::new(Rc::clone(
        &local,
    )));
    SessionPresentation {
        host: selector(local),
        keybindings: None,
        presenter: Rc::new(move |request, callbacks| {
            let component = SessionSelectorComponent::new(
                request.current_sessions_loader,
                request.all_sessions_loader,
                callbacks,
                SessionSelectorOptions {
                    keybindings: Some(request.keybindings),
                    show_rename_hint: Some(request.show_rename_hint),
                    rename_session: None,
                    file_operations: None,
                    current_session_file_path: None,
                    theme: theme.theme(),
                    runtime: runtime.clone(),
                    request_render: request.request_render,
                    now_ms: Rc::new(|| {
                        std::time::SystemTime::now()
                            .duration_since(std::time::UNIX_EPOCH)
                            .unwrap_or_default()
                            .as_secs_f64()
                            * 1000.0
                    }),
                    home: Rc::new(|| maestro_app::home_directory().unwrap_or_default()),
                    styles_enabled: Rc::new(supports_color),
                },
            )?;
            Ok(PresentedSelector {
                component: component.clone(),
                focus: component,
            })
        }),
    }
}

/// Continue/cancel presentation using the existing extension selection component.
pub fn missing_cwd(local: Rc<LocalSet>, theme: Rc<ThemeState>) -> MissingSessionCwdPresentation {
    let runtime = Rc::new(maestro_tui_crossterm::ProcessTuiRuntime::new(Rc::clone(
        &local,
    )));
    let palette = theme.theme();
    let presenter: ExtensionSelectorFactory = Rc::new(move |request| {
        let component = Rc::new(
            ExtensionSelectorComponent::new(
                request.title,
                request.options,
                request.on_select,
                request.on_cancel,
                ExtensionSelectorOptions {
                    tui: Some(request.tui),
                    timeout: None,
                    runtime: runtime.clone(),
                    theme: palette.clone(),
                },
            )
            .map_err(io::Error::other)?,
        );
        Ok(PresentedSelector {
            component: component.clone(),
            focus: component,
        })
    });
    let create = writer(local);
    MissingSessionCwdPresentation {
        create_ui: Rc::new(move |cursor| create(Some(cursor))),
        theme,
        presenter,
    }
}

#[cfg(test)]
mod tests;
