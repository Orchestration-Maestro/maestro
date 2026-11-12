//! Selector factories over the existing chat components and native frame writer.
use maestro_app::presentation_data::input_selectors::{
    ExtensionSelectorFactory, PresentedSelector,
};
use maestro_chat::components::{ComponentOptions, ConfigSelectorComponent};
use maestro_chat::{
    ExtensionSelectorComponent, ExtensionSelectorOptions, SessionSelectorComponent,
    SessionSelectorOptions,
};
use maestro_cli::{
    input::{
        configuration::ConfigPresentation, selection::SelectorHost,
        session_picker::SessionPresentation,
    },
    startup::entrypoint::feedback::MissingSessionCwdPresentation,
};
use maestro_theme::{Collator, NativeThemeWatchOperations, ThemeState, supports_color};
use maestro_tui::{TUI, TerminalImage};
use std::{io, rc::Rc};
use tokio::task::LocalSet;

/// The frame-writer factory shared by both selector hosts.
fn writer(local: Rc<LocalSet>) -> Rc<dyn Fn(Option<bool>) -> io::Result<TUI>> {
    Rc::new(move |cursor| {
        let (terminal, runtime) =
            maestro_cli::input::selection::native_terminal(Rc::clone(&local))?;
        Ok(TUI::new(
            terminal,
            runtime,
            TerminalImage::default(),
            cursor,
        ))
    })
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
                ComponentOptions {
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
    let host = selector(Rc::clone(&local));
    SessionPresentation {
        host,
        keybindings: None,
        presenter: Rc::new(move |request, callbacks| {
            let runtime = Rc::new(maestro_tui_crossterm::ProcessTuiRuntime::new(
                Rc::clone(&local),
                maestro_app::configuration::config::get_agent_dir()?,
            ));
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
                    runtime,
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
    let create = writer(Rc::clone(&local));
    let palette = theme.theme();
    let presenter: ExtensionSelectorFactory = Rc::new(move |request| {
        let runtime = Rc::new(maestro_tui_crossterm::ProcessTuiRuntime::new(
            Rc::clone(&local),
            maestro_app::configuration::config::get_agent_dir()?,
        ));
        let component = Rc::new(
            ExtensionSelectorComponent::new(
                request.title,
                request.options,
                request.on_select,
                request.on_cancel,
                ExtensionSelectorOptions {
                    tui: Some(request.tui),
                    timeout: None,
                    runtime,
                    theme: palette.clone(),
                    on_toggle_tools_expanded: None,
                },
            )
            .map_err(io::Error::other)?,
        );
        Ok(PresentedSelector {
            component: component.clone(),
            focus: component,
        })
    });
    MissingSessionCwdPresentation {
        create_ui: Rc::new(move |cursor| create(Some(cursor))),
        theme,
        presenter,
    }
}

#[cfg(test)]
mod tests;
