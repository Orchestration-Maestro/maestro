//! Theme directories selected from the installation and the environment.
#![cfg(unix)]
use crate::resource_fixtures::{Entry, Root, with_environment};
use crate::watching::Effects;
use maestro_app::configuration::config::get_theme_directories;
use maestro_theme::{ThemeExportColors, ThemeState, ThemeWatchOperations};
use serde_json::{Value, json};
use std::rc::Rc;

/// The agent and package directories every case selects through the environment.
const VARIABLES: [(&str, &str); 2] = [
    ("MAESTRO_CODING_AGENT_DIR", "$BASE/case/agent"),
    ("MAESTRO_PACKAGE_DIR", "$BASE/case/package"),
];

/// Writes the shipped themes and `custom` into the case root, then returns the theme
/// state over the directories selected for the running process, which runs from its
/// source and so takes the shipped themes from the package's distribution tree.
fn state(base: &str, custom: &Value) -> (Root, Rc<ThemeState>) {
    let root = Root::new(base, "case");
    let entries: Vec<Entry> = serde_json::from_value(json!([
        {"path": "package/dist/modes/interactive/theme/dark.json", "theme": {"name": "dark"}},
        {"path": "package/dist/modes/interactive/theme/light.json", "theme": {"name": "light"}},
        custom,
    ]))
    .unwrap();
    root.write_tree(&entries);
    let directories = get_theme_directories().unwrap();
    assert_eq!(
        (
            directories.themes_dir.as_str(),
            directories.custom_themes_dir.as_str()
        ),
        (
            root.fill("$ROOT/package/dist/modes/interactive/theme")
                .as_str(),
            root.fill("$ROOT/agent/themes").as_str()
        ),
    );
    (root, Rc::new(ThemeState::new(directories)))
}

#[test]
fn custom_theme_export_resolves_variable_aliases() {
    with_environment(
        "custom_theme_export_resolves_variable_aliases",
        &VARIABLES,
        |base| {
            let (_, state) = state(
                base,
                &json!({"path": "agent/themes/custom-export-vars.json", "theme": {
                    "name": "custom-export-vars",
                    "vars": {"pageBgVar": "#112233", "pageBgAlias": "pageBgVar", "infoBgVar": "#445566", "cardBgVar": "#223344"},
                    "export": {"pageBg": "pageBgAlias", "cardBg": "cardBgVar", "infoBg": "infoBgVar"},
                }}),
            );
            let expected = ThemeExportColors {
                page_bg: Some("#112233".into()),
                card_bg: Some("#223344".into()),
                info_bg: Some("#445566".into()),
            };
            assert_eq!(
                state.get_theme_export_colors(Some("custom-export-vars")),
                expected
            );
        },
    );
}

#[test]
fn custom_theme_export_converts_indexed_colors_and_omits_empty_values() {
    let test = "custom_theme_export_converts_indexed_colors_and_omits_empty_values";
    with_environment(test, &VARIABLES, |base| {
        let (_, state) = state(
            base,
            &json!({"path": "agent/themes/custom-export-recursive.json", "theme": {
                "name": "custom-export-recursive",
                "vars": {"deepPageBg": "#abcdef", "pageBgAlias": "deepPageBg", "cardBgAnsi": 24},
                "export": {"pageBg": "pageBgAlias", "cardBg": "cardBgAnsi", "infoBg": ""},
            }}),
        );
        let expected = ThemeExportColors {
            page_bg: Some("#abcdef".into()),
            card_bg: Some("#005f87".into()),
            info_bg: None,
        };
        assert_eq!(
            state.get_theme_export_colors(Some("custom-export-recursive")),
            expected
        );
    });
}

/// A watch error on the selected custom theme's directory closes the watch and keeps the
/// published theme.
#[test]
fn resolved_custom_theme_watch_errors_leave_child_alive() {
    with_environment(
        "resolved_custom_theme_watch_errors_leave_child_alive",
        &VARIABLES,
        |base| {
            let custom =
                json!({"path": "agent/themes/custom-test.json", "theme": {"name": "custom-test"}});
            let (root, state) = state(base, &custom);
            let effects = Rc::new(Effects::default());
            let operations: Rc<dyn ThemeWatchOperations> = effects.clone();
            state.set_theme("custom-test", Some(operations)).unwrap();
            assert_eq!(effects.active(), [root.fill("$ROOT/agent/themes")]);
            effects.error(0);
            assert!(effects.active().is_empty());
            let live = state.theme().get().unwrap();
            assert_eq!(live.name(), Some("custom-test"));
        },
    );
}
