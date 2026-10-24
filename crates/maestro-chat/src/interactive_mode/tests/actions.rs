//! Editor actions through the mode's installed callbacks and its rendered editor.
use super::{World, dark, isolated, light, roles, started};
use crate::components::session_selector::list::tests::session_picker::decode_cases;
use maestro_app::ThinkingLevel;
use maestro_tui::Component;
use serde::{Deserialize, de::DeserializeOwned};
use std::rc::Rc;

/// Every recorded editor-action case.
const FIXTURES: &str = include_str!("../../../tests/fixtures/interactive_actions.json");

/// The width every editor frame is rendered at.
const WIDTH: usize = 80;

/// The recorded cases of `test`, typed by that witness.
fn cases<I: DeserializeOwned, E: DeserializeOwned>(test: &str) -> Vec<(I, E)> {
    decode_cases(FIXTURES, test)
}

/// The editor's top border row as the recorder's painted probe glyph describes it.
fn border_row(world: &World, recorded: &str) -> String {
    let theme = world.themes.theme().get().unwrap();
    roles(&theme, &recorded.replace('━', "─")).repeat(WIDTH)
}

/// The editor's rendered top border row.
fn rendered_border(world: &World) -> String {
    world.mode.editor.render(WIDTH).swap_remove(0)
}

/// Selects `level` on the session as its agent state, whatever the model supports.
fn select_level(world: &World, level: ThinkingLevel) {
    world
        .host
        .session()
        .agent()
        .state()
        .write()
        .unwrap()
        .thinking_level = level;
}

/// Editor text delivered to the change callback in one shell mode.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct TextChange {
    /// Always `change`.
    op: String,
    /// The shell mode before the change.
    initial: bool,
    /// The editor text.
    text: String,
}

/// The shell mode and border after a change.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ChangedBorder {
    /// The shell mode after the change.
    shell: bool,
    /// The probe glyph as painted; unstyled when the border was not repainted.
    border: String,
}

#[test]
fn border_text_change_uses_the_shared_whitespace_domain() {
    isolated(
        "border_text_change_uses_the_shared_whitespace_domain",
        |directory| {
            let world = started(directory);
            let on_change = world.mode.editor.on_change().unwrap();
            for (input, expected) in cases::<TextChange, ChangedBorder>(
                "border_text_change_uses_the_shared_whitespace_domain",
            ) {
                assert_eq!(input.op, "change");
                world.mode.is_bash_mode.set(input.initial);
                world.mode.editor.set_border_color(Rc::new(str::to_owned));
                on_change(&input.text);
                assert_eq!(
                    world.mode.is_bash_mode.get(),
                    expected.shell,
                    "{:?}",
                    input.text
                );
                assert_eq!(
                    rendered_border(&world),
                    border_row(&world, &expected.border)
                );
                let repainted = expected.shell != input.initial;
                assert_eq!(world.runtime.pending(), usize::from(repainted));
                world.runtime.settle().unwrap();
            }
        },
    );
}

/// A border refresh in one shell mode and thinking level.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Refresh {
    /// Always `border`.
    op: String,
    /// Whether the editor is in shell mode.
    shell: bool,
    /// The session's thinking level.
    level: ThinkingLevel,
}

/// The painted border after a refresh.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Painted {
    /// The probe glyph as painted.
    border: String,
}

#[test]
fn border_refresh_paints_every_level_and_shell_override() {
    isolated(
        "border_refresh_paints_every_level_and_shell_override",
        |directory| {
            let world = started(directory);
            for (input, expected) in
                cases::<Refresh, Painted>("border_refresh_paints_every_level_and_shell_override")
            {
                assert_eq!(input.op, "border");
                world.mode.is_bash_mode.set(input.shell);
                select_level(&world, input.level);
                world.mode.update_editor_border_color().unwrap();
                assert_eq!(
                    rendered_border(&world),
                    border_row(&world, &expected.border)
                );
            }
        },
    );
}

#[test]
fn border_theme_change_refreshes_the_first_returned_frame() {
    isolated(
        "border_theme_change_refreshes_the_first_returned_frame",
        |directory| {
            let world = started(directory);
            select_level(&world, ThinkingLevel::High);
            for (shell, theme, role) in [
                (false, light(), "@thinkingHigh@━\x1b[39m"),
                (true, dark(), "@bashMode@━\x1b[39m"),
            ] {
                world.mode.is_bash_mode.set(shell);
                let stale = border_row(&world, role);
                world.themes.set_theme_instance(theme).unwrap();
                world.terminal.clear_writes();
                world.runtime.settle().unwrap();
                let frame = world.terminal.writes().concat();
                assert!(frame.contains(&border_row(&world, role)));
                assert!(!frame.contains(&stale));
            }
        },
    );
}
