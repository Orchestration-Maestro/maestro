//! Live settings through the panel the mode shows in place of its editor.
use super::typed::submit;
use super::{Setup, World, isolated, start};
use crate::SettingsSelectorComponent;
use maestro_app::{MessageDeliveryMode, SettingsManager, Transport, TransportSetting};
use maestro_theme::{ThemeDirectories, ThemeState};
use maestro_tui::tui::InputHandler;
use maestro_tui::{Component, SettingsList, TerminalCapabilities};
use std::path::Path;
use std::rc::Rc;

/// The shipped themes every panel inventory reads.
const SHIPPED: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../maestro-theme/assets/theme");

/// A registry over the shipped themes and the custom directory of `directory`.
fn registry(directory: &Path) -> Rc<ThemeState> {
    let custom = directory.join("themes");
    if !custom.exists() {
        std::fs::create_dir(&custom).unwrap();
    }
    Rc::new(ThemeState::new(ThemeDirectories {
        themes_dir: SHIPPED.to_owned(),
        custom_themes_dir: custom.to_str().unwrap().to_owned(),
    }))
}

/// A started mode whose panel reads a real theme inventory.
fn settings_world(
    directory: &Path,
    configure: impl FnOnce(&mut SettingsManager) + 'static,
) -> World {
    let world = Setup {
        theme_state: Some(registry(directory)),
        configure: Box::new(configure),
        ..Setup::default()
    }
    .build(directory);
    start(&world);
    world
}

/// A default started mode whose panel reads a real theme inventory.
fn started_settings(directory: &Path) -> World {
    settings_world(directory, |_| {})
}

/// The settings panel the editor container shows, if it shows one.
fn panel(world: &World) -> Option<Rc<SettingsSelectorComponent>> {
    let children = world.mode.editor_container.children().borrow().clone();
    let [child] = children.as_slice() else {
        return None;
    };
    (Rc::clone(child) as Rc<dyn std::any::Any>)
        .downcast::<SettingsSelectorComponent>()
        .ok()
}

/// Opens settings through the mode's own operation and returns the shown list.
fn open(world: &World) -> Rc<SettingsList> {
    world.mode.show_settings_selector().unwrap();
    open_list(world)
}

/// The list of the panel already shown.
fn open_list(world: &World) -> Rc<SettingsList> {
    Rc::clone(
        panel(world)
            .expect("the panel is shown")
            .get_settings_list(),
    )
}

/// The saved form of `transport`.
fn saved(transport: &Transport) -> TransportSetting {
    match transport {
        Transport::Auto => TransportSetting::Auto,
        Transport::Sse => TransportSetting::Sse,
        Transport::Websocket => TransportSetting::Websocket,
        Transport::WebsocketCached => TransportSetting::WebsocketCached,
    }
}

/// Delivers `data` to the panel's installed list.
fn press(list: &SettingsList, data: &str) {
    list.handle_input(data);
}

/// Clears the list's search and types `query`, leaving only its matches.
fn search(list: &SettingsList, query: &str) {
    for _ in 0..16 {
        press(list, "\u{7f}");
    }
    for character in query.chars() {
        press(list, &character.to_string());
    }
}

/// Activates the `match_index`-th row the query leaves, in panel order.
fn choose(list: &SettingsList, query: &str, match_index: usize) {
    search(list, query);
    for _ in 0..match_index {
        press(list, "\x1b[B");
    }
    press(list, "\r");
}

/// The panel's rendered rows.
fn rows(world: &World) -> Vec<String> {
    panel(world).expect("the panel is shown").render(100)
}

/// Whether the panel's rendered rows hold a row starting with `label` and carrying `value`.
fn shows_value(world: &World, label: &str, value: &str) -> bool {
    rows(world).iter().any(|row| {
        let text = strip_ansi(row);
        let text = text.trim_start_matches(|character: char| !character.is_alphanumeric());
        text.starts_with(label) && text.split_whitespace().last() == Some(value)
    })
}

/// The printable text of `row`, without its style escapes.
fn strip_ansi(row: &str) -> String {
    let mut text = String::new();
    let mut chars = row.chars();
    while let Some(character) = chars.next() {
        if character != '\u{1b}' {
            text.push(character);
            continue;
        }
        for character in chars.by_ref() {
            if character.is_ascii_alphabetic() {
                break;
            }
        }
    }
    text
}

#[test]
fn settings_snapshot_reads_live_session_and_preferences() {
    isolated(
        "settings_snapshot_reads_live_session_and_preferences",
        |directory| {
            let world = settings_world(directory, |settings| {
                settings.set_show_images(false);
                settings.set_image_width_cells(80.0);
                settings.set_theme("light".to_owned());
            });
            let session = world.host.session();
            session.set_auto_compaction_enabled(true);
            session.set_steering_mode(maestro_app::QueueMode::All);
            session.set_follow_up_mode(maestro_app::QueueMode::OneAtATime);
            let levels = session.get_available_thinking_levels();
            open(&world);
            let config = world.mode.settings_config().unwrap();
            assert!(config.session.auto_compact);
            assert!(!config.session.quiet_startup);
            assert_eq!(config.steering_mode, MessageDeliveryMode::All);
            assert_eq!(config.follow_up_mode, MessageDeliveryMode::OneAtATime);
            assert_eq!(config.thinking_level, session.thinking_level());
            assert_eq!(
                config.available_thinking_levels, levels,
                "the panel offers the model's own levels"
            );
            assert!(!config.show_images);
            assert!(config.image_width_cells.total_cmp(&80.0).is_eq());
            assert_eq!(config.current_theme, "light");
            assert_eq!(
                config.available_themes,
                ["dark".to_owned(), "light".to_owned()],
                "the panel lists the real inventory"
            );
            assert_eq!(
                config.available_thinking_levels,
                session.get_available_thinking_levels()
            );
            let list = open_list(&world);
            search(&list, "Transport");
            assert!(shows_value(&world, "Transport", "auto"));
            search(&list, "Theme");
            assert!(shows_value(&world, "Theme", "light"));
            search(&list, "Auto-compact");
            assert!(shows_value(&world, "Auto-compact", "true"));
            drop(world.runtime.settle());
        },
    );
}

#[test]
fn settings_snapshot_covers_every_source_field() {
    isolated("settings_snapshot_covers_every_source_field", |directory| {
        // A terminal that reports image support admits the inline image rows; a plain
        // one leaves them out.
        let world = started_settings(directory);
        world
            .mode
            .tool_environment
            .terminal_image
            .set_capabilities(TerminalCapabilities::default());
        open(&world);
        let rows = rows(&world);
        assert!(rows.iter().any(|row| row.contains("resize")));
        assert!(!rows.iter().any(|row| row.contains("Show images")));
        drop(world.runtime.settle());
    });
}

#[test]
fn settings_cancel_requests_a_frame_after_restoring_editor() {
    isolated(
        "settings_cancel_requests_a_frame_after_restoring_editor",
        |directory| {
            let world = started_settings(directory);
            let list = open(&world);
            assert!(panel(&world).is_some());
            press(&list, "\x1b");
            assert!(panel(&world).is_none(), "the panel is removed");
            let children = world.mode.editor_container.children().borrow().clone();
            assert_eq!(children.len(), 1);
            assert!(std::ptr::addr_eq(
                Rc::as_ptr(&children[0]),
                Rc::as_ptr(&world.mode.editor).cast::<()>()
            ));
            world.mode.editor.set_text("");
            world.terminal.send_input("z");
            assert_eq!(world.mode.editor.get_text(), "z", "the editor is focused");
            assert!(world.runtime.pending() > 0, "a frame is requested");
            drop(world.runtime.settle());
        },
    );
}

#[test]
fn settings_command_uses_trimmed_exact_token_and_clears_editor() {
    isolated(
        "settings_command_uses_trimmed_exact_token_and_clears_editor",
        |directory| {
            let world = started_settings(directory);
            for text in ["/settings", " /settings\t", "\u{feff}/settings "] {
                world.mode.editor.set_text(text);
                submit(&world, text);
                assert!(panel(&world).is_some(), "{text:?} opens the panel");
                assert_eq!(world.mode.editor.get_text(), "", "text is cleared");
                press(&open_list(&world), "\x1b");
                assert!(panel(&world).is_none(), "the panel closes between cases");
            }
            for text in [
                "/settings all",
                "/settingsall",
                "/Settings",
                "\u{85}/settings",
                "\u{200b}/settings",
            ] {
                world.mode.editor.set_text(text);
                submit(&world, text);
                assert!(panel(&world).is_none(), "{text:?} does not open the panel");
                assert_eq!(world.mode.editor.get_text(), text, "text stays");
            }
            drop(world.runtime.settle());
        },
    );
}

#[test]
fn settings_changes_preserve_each_callback_effect() {
    isolated(
        "settings_changes_preserve_each_callback_effect",
        |directory| {
            let world = started_settings(directory);
            let list = open(&world);
            let session = world.host.session();
            assert!(footer_shows(&world, "(auto)"));
            choose(&list, "compact", 0);
            assert!(!session.auto_compaction_enabled(), "the session saved it");
            assert!(!footer_shows(&world, "(auto)"), "the footer learned it");
            choose(&list, "resize", 0);
            assert!(!session.settings_manager().borrow().get_image_auto_resize());
            choose(&list, "block", 0);
            assert!(session.settings_manager().borrow().get_block_images());
            choose(&list, "skill", 0);
            assert!(
                !session
                    .settings_manager()
                    .borrow()
                    .get_enable_skill_commands()
            );
            choose(&list, "quiet", 0);
            assert!(session.settings_manager().borrow().get_quiet_startup());
            choose(&list, "changelog", 0);
            assert!(session.settings_manager().borrow().get_collapse_changelog());
            choose(&list, "telemetry", 0);
            assert!(
                !session
                    .settings_manager()
                    .borrow()
                    .get_enable_install_telemetry()
            );
            drop(world.runtime.settle());
        },
    );
}

#[test]
fn settings_remaining_changes_preserve_each_callback_effect() {
    isolated(
        "settings_remaining_changes_preserve_each_callback_effect",
        |directory| {
            let world = started_settings(directory);
            let list = open(&world);
            let session = world.host.session();
            choose(&list, "Escape", 0);
            assert_eq!(
                session
                    .settings_manager()
                    .borrow()
                    .get_double_escape_action(),
                maestro_app::DoubleEscapeAction::Fork
            );
            choose(&list, "filter", 0);
            assert_eq!(
                session.settings_manager().borrow().get_tree_filter_mode(),
                maestro_app::TreeFilterMode::NoTools
            );
            choose(&list, "shrink", 0);
            assert!(session.settings_manager().borrow().get_clear_on_shrink());
            choose(&list, "progress", 0);
            assert!(
                session
                    .settings_manager()
                    .borrow()
                    .get_show_terminal_progress()
            );
            let cursor = session
                .settings_manager()
                .borrow()
                .get_show_hardware_cursor();
            choose(&list, "hardware", 0);
            let saved = session
                .settings_manager()
                .borrow()
                .get_show_hardware_cursor();
            assert_ne!(saved, cursor, "the cursor choice is saved");
            assert!(
                !session.settings_manager().borrow().get_block_images(),
                "unrelated settings are unchanged"
            );
            search(&list, "Terminal");
            assert!(shows_value(&world, "Terminal progress", "true"));
            drop(world.runtime.settle());
        },
    );
}

#[test]
fn settings_session_modes_use_distinct_live_policies() {
    isolated(
        "settings_session_modes_use_distinct_live_policies",
        |directory| {
            let world = settings_world(directory, |_| {});
            let session = world.host.session();
            session.set_steering_mode(maestro_app::QueueMode::All);
            session.set_follow_up_mode(maestro_app::QueueMode::OneAtATime);
            let list = open(&world);
            choose(&list, "Steering", 0);
            assert_eq!(session.steering_mode(), maestro_app::QueueMode::OneAtATime);
            assert_eq!(
                session.follow_up_mode(),
                maestro_app::QueueMode::OneAtATime,
                "the sibling policy is unchanged"
            );
            choose(&list, "follow", 0);
            assert_eq!(session.follow_up_mode(), maestro_app::QueueMode::All);
            assert_eq!(
                session.steering_mode(),
                maestro_app::QueueMode::OneAtATime,
                "the sibling policy is unchanged in the other direction"
            );
            drop(world.runtime.settle());
        },
    );
}

#[test]
fn settings_transport_change_reaches_the_live_agent() {
    isolated(
        "settings_transport_change_reaches_the_live_agent",
        |directory| {
            let world = started_settings(directory);
            let list = open(&world);
            let session = world.host.session();
            for transport in [
                Transport::Sse,
                Transport::Websocket,
                Transport::WebsocketCached,
                Transport::Auto,
            ] {
                choose(&list, "Transport", 0);
                assert_eq!(
                    session.settings_manager().borrow().get_transport(),
                    saved(&transport),
                    "the selection is saved"
                );
                let live = session
                    .agent()
                    .transport
                    .read()
                    .unwrap_or_else(std::sync::PoisonError::into_inner)
                    .clone();
                assert_eq!(live, transport);
            }
            drop(world.runtime.settle());
        },
    );
}

#[test]
fn settings_theme_preview_commit_and_failure_keep_distinct_effects() {
    isolated(
        "settings_theme_preview_commit_and_failure_keep_distinct_effects",
        |directory| {
            let world = started_settings(directory);
            let list = open(&world);
            let settings = Rc::clone(world.host.session().settings_manager());
            choose(&list, "Theme", 0);
            // The themes row opens a submenu over the inventory names, selecting the
            // committed one.
            let names = world.themes.get_available_themes().unwrap();
            assert_eq!(names, ["dark", "light"]);
            press(&list, "\x1b[B");
            press(&list, "\r");
            assert_eq!(
                settings.borrow().get_theme().as_deref(),
                Some("light"),
                "commit saves the requested name"
            );
            drop(world.runtime.settle());
        },
    );
}

/// Whether the footer paints `label` in its current state.
fn footer_shows(world: &World, label: &str) -> bool {
    world
        .mode
        .footer
        .render(100)
        .iter()
        .any(|row| strip_ansi(row).contains(label))
}
