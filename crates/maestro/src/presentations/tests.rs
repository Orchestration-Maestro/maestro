//! Keyboard behavior through the executable's configuration presentation.
use super::config;
use maestro_app::presentation_data::input_selectors::{
    ConfigSelectorCallbacks, ConfigSelectorOptions,
};
use maestro_app::{
    PathMetadata, ResolvedPaths, ResolvedResource, Settings, SettingsManager, SourceOrigin,
    SourceScope,
};
use maestro_test_directory::Scratch;
use maestro_theme::{ThemeDirectories, ThemeState};
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};
use tokio::task::LocalSet;

/// One project prompt whose toggle must not write user settings.
fn options(cwd: &str, settings_manager: Rc<RefCell<SettingsManager>>) -> ConfigSelectorOptions {
    ConfigSelectorOptions {
        resolved_paths: ResolvedPaths {
            prompts: vec![ResolvedResource {
                path: format!("{cwd}/.maestro/prompts/review.md"),
                enabled: true,
                metadata: PathMetadata {
                    source: "local".into(),
                    scope: SourceScope::Project,
                    origin: SourceOrigin::TopLevel,
                    base_dir: None,
                },
            }],
            ..ResolvedPaths::default()
        },
        settings_manager,
        cwd: cwd.into(),
        agent_dir: cwd.into(),
    }
}

#[test]
fn config_presentation_space_toggles_and_escape_closes() {
    let scratch = Scratch::new().unwrap();
    let cwd = scratch.path().to_string_lossy().into_owned();
    let theme = Rc::new(ThemeState::new(ThemeDirectories {
        themes_dir: concat!(env!("CARGO_MANIFEST_DIR"), "/../maestro-theme/assets/theme").into(),
        custom_themes_dir: cwd.clone(),
    }));
    theme.init_theme(Some("dark"), None).unwrap();
    let presentation = config(Rc::new(LocalSet::new()), theme);
    let settings = Rc::new(RefCell::new(
        SettingsManager::in_memory(Settings::default()),
    ));
    let closed = Rc::new(Cell::new(false));
    let on_close = closed.clone();
    let view = (presentation.presenter)(
        options(&cwd, settings.clone()),
        ConfigSelectorCallbacks {
            on_close: Rc::new(move || on_close.set(true)),
            on_exit: Rc::new(|| panic!("Escape must close, not exit")),
            request_render: Rc::new(|| {}),
        },
    )
    .unwrap();
    assert!(
        view.component
            .try_render(80)
            .unwrap()
            .iter()
            .any(|row| row.contains("review.md"))
    );
    for key in [" ", "\x1b"] {
        if let Some(handler) = view.focus.input_handler() {
            handler.try_handle_input(key).unwrap();
        }
    }
    assert_eq!(
        (
            serde_json::Value::Object(settings.borrow().get_project_settings().0),
            closed.get()
        ),
        (serde_json::json!({"prompts": ["-prompts/review.md"]}), true),
        "Space must edit project settings and Escape must close the presentation",
    );
    assert_eq!(settings.borrow().get_global_settings(), Settings::default());
}
