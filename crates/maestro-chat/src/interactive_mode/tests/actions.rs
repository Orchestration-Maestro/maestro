//! Editor actions through the mode's installed callbacks, its rendered editor and the
//! native editor, clipboard helper and temporary files of an isolated child.
use super::{World, dark, isolated, light, native, painted, roles, started};
use crate::components::session_selector::list::tests::session_picker::decode_cases;
use maestro_app::ThinkingLevel;
use maestro_app::utils::clipboard_image::ClipboardImage;
use maestro_tui::{Component, tui::InputHandler};
use serde::{Deserialize, de::DeserializeOwned};
use std::{
    io,
    path::{Path, PathBuf},
    rc::Rc,
};

/// Every recorded editor-action case.
const FIXTURES: &str = include_str!("../../../tests/fixtures/interactive_actions.json");

/// The width every editor frame is rendered at.
const WIDTH: usize = 80;

/// The width the transcript notices were recorded at.
const NOTICE_WIDTH: usize = 100;

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

/// The edit file every external edit in these witnesses writes, named by the host clock.
const EDIT_FILE: &str = "maestro-editor-1767323045006.maestro.md";

/// Installs `script` as the executable `name` in the command directory of `directory`.
fn install(directory: &Path, name: &str, script: &str) -> PathBuf {
    let bin = directory.join("bin");
    let source = bin.join(format!("{name}.source"));
    std::fs::write(&source, script).unwrap();
    let target = bin.join(name);
    native::install_executable(&source, &target);
    std::fs::remove_file(source).unwrap();
    target
}

/// Runs the editor action the mode registered.
fn run_action(world: &World, action: &str) {
    let handler = world.mode.editor.action_handlers().borrow()[action].clone();
    handler();
}

/// The names in the child's temporary directory.
fn temporary_names() -> Vec<String> {
    let mut names: Vec<_> = std::fs::read_dir(std::env::temp_dir())
        .unwrap()
        .map(|entry| entry.unwrap().file_name().into_string().unwrap())
        .collect();
    names.sort();
    names
}

/// The external-edit operation.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct MissingEditor {
    /// Always `external-missing`.
    op: String,
}

/// Transcript rows.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Lines {
    /// The transcript rows.
    lines: Vec<String>,
}

#[test]
fn external_edit_without_configuration_warns_without_native_work() {
    isolated(
        "external_edit_without_configuration_warns_without_native_work",
        |directory| {
            let (input, expected) = cases::<MissingEditor, Lines>(
                "external_edit_without_configuration_warns_without_native_work",
            )
            .swap_remove(0);
            assert_eq!(input.op, "external-missing");
            let theme = |world: &World| world.themes.theme().get().unwrap();
            for configured in [None, Some("")] {
                let world = started(directory);
                if let Some(value) = configured {
                    world.runtime.set_environment("VISUAL", value);
                    world.runtime.set_environment("EDITOR", value);
                }
                world.mode.editor.set_text("kept");
                world.terminal.clear_writes();
                run_action(&world, "app.editor.external");
                assert_eq!(
                    world.mode.chat_container.render(NOTICE_WIDTH),
                    painted(&theme(&world), &expected.lines)
                );
                assert_eq!(world.mode.editor.get_text(), "kept");
                assert_eq!(world.terminal.events(), ["start"]);
                assert_eq!(temporary_names(), Vec::<String>::new());
            }
        },
    );
}

/// One external edit of a large paste.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ExternalEdit {
    /// Always `external`.
    op: String,
    /// The text the editor writes back.
    content: String,
    /// The editor's exit status.
    status: i32,
    /// The editor text holding the paste marker.
    stored: String,
    /// The pasted text the marker stands for.
    expanded: String,
}

/// What the edit leaves and what the editor received.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Edited {
    /// The editor text afterwards.
    text: String,
    /// What the editor process received.
    handoff: Handoff,
}

/// The editor process's view of the edit.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Handoff {
    /// Arguments before the file.
    args: Vec<String>,
    /// The file's text when the editor started.
    text: String,
    /// The file's permission bits.
    mode: u32,
}

/// An editor that records its arguments, the file and its mode, then writes the planned
/// content and exits with the planned status.
const EDITOR: &str = "#!/bin/sh\nfor file; do :; done\nprintf '%s\\n' \"$@\" > \"${0%/*}/argv\"\n/bin/cp -p \"$file\" \"${0%/*}/before\"\n/bin/cat \"${0%/*}/content\" > \"$file\"\nexit \"$(/bin/cat \"${0%/*}/status\")\"\n";

#[test]
fn external_edit_uses_expanded_text_and_preserves_the_exit_contract() {
    isolated(
        "external_edit_uses_expanded_text_and_preserves_the_exit_contract",
        |directory| {
            use std::os::unix::fs::PermissionsExt;
            let editor = install(directory, "editor", EDITOR);
            let bin = editor.parent().unwrap();
            for (input, expected) in cases::<ExternalEdit, Edited>(
                "external_edit_uses_expanded_text_and_preserves_the_exit_contract",
            ) {
                assert_eq!(input.op, "external");
                let world = started(directory);
                world
                    .runtime
                    .set_environment("VISUAL", &format!("{} --wait", editor.display()));
                std::fs::write(bin.join("content"), &input.content).unwrap();
                std::fs::write(bin.join("status"), input.status.to_string()).unwrap();
                world
                    .mode
                    .editor
                    .handle_input(&format!("\x1b[200~{}\x1b[201~", input.expanded));
                assert_eq!(world.mode.editor.get_text(), input.stored);
                run_action(&world, "app.editor.external");
                assert_eq!(world.mode.editor.get_text(), expected.text);
                assert_eq!(world.terminal.events(), ["start", "stop", "start"]);
                world.terminal.clear_writes();
                world.runtime.settle().unwrap();
                let frame = world.terminal.writes().concat();
                assert!(frame.contains(expected.text.lines().next().unwrap_or_default()));
                let recorded = std::fs::read_to_string(bin.join("argv")).unwrap();
                let file = std::env::temp_dir().join(EDIT_FILE);
                let mut args: Vec<_> = recorded.lines().collect();
                assert_eq!(args.pop(), Some(file.to_str().unwrap()));
                assert_eq!(args, expected.handoff.args);
                let before = bin.join("before");
                assert_eq!(
                    std::fs::read_to_string(&before).unwrap(),
                    expected.handoff.text
                );
                let mode = std::fs::metadata(&before).unwrap().permissions().mode();
                assert_eq!(mode & 0o7777, expected.handoff.mode);
                assert_eq!(temporary_names(), Vec::<String>::new());
                std::fs::remove_file(before).unwrap();
            }
        },
    );
}

/// A Wayland paste helper that lists the planned types and prints the planned image.
const WL_PASTE: &str = "#!/bin/sh\nif [ \"$1\" = --list-types ]; then /bin/cat \"${0%/*}/types\"; else /bin/cat \"${0%/*}/image\"; fi\n";

/// Runs the installed image paste and the host work it hands over.
fn paste(world: &World) {
    let on_paste = world.mode.editor.on_paste_image().unwrap();
    on_paste();
    assert!(world.runtime.drain_futures().is_empty());
}

/// One clipboard image type.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Paste {
    /// Always `paste`.
    op: String,
    /// The type the clipboard offers.
    #[serde(rename = "type")]
    mime_type: String,
}

/// The pasted file and the editor text around its path.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Pasted {
    /// The file's bytes.
    bytes: Vec<u8>,
    /// The file name's suffix.
    extension: String,
    /// The editor text, with `<image-path>` standing for the inserted path.
    text: String,
    /// The file's permission bits.
    mode: u32,
}

#[cfg(target_os = "linux")]
#[test]
fn clipboard_paste_keeps_image_bytes_and_inserts_at_the_cursor() {
    isolated(
        "clipboard_paste_keeps_image_bytes_and_inserts_at_the_cursor",
        |directory| {
            use std::os::unix::fs::PermissionsExt;
            let helper = install(directory, "wl-paste", WL_PASTE);
            let bin = helper.parent().unwrap();
            for (input, expected) in cases::<Paste, Pasted>(
                "clipboard_paste_keeps_image_bytes_and_inserts_at_the_cursor",
            ) {
                assert_eq!(input.op, "paste");
                let world = started(directory);
                std::fs::write(bin.join("types"), format!("{}\n", input.mime_type)).unwrap();
                std::fs::write(bin.join("image"), &expected.bytes).unwrap();
                world.mode.editor.set_text("leftright");
                for _ in 0.."right".len() {
                    world.mode.editor.handle_input("\x1b[D");
                }
                world.runtime.settle().unwrap();
                paste(&world);
                let [name] = temporary_names().try_into().unwrap();
                let path = std::env::temp_dir().join(&name);
                let stem = name
                    .strip_prefix("maestro-clipboard-")
                    .and_then(|rest| rest.strip_suffix(&format!(".{}", expected.extension)))
                    .unwrap();
                assert_eq!(uuid::Uuid::parse_str(stem).unwrap().get_version_num(), 4);
                assert_eq!(std::fs::read(&path).unwrap(), expected.bytes);
                let mode = std::fs::metadata(&path).unwrap().permissions().mode();
                assert_eq!(mode & 0o7777, expected.mode);
                assert_eq!(
                    world.mode.editor.get_text(),
                    expected
                        .text
                        .replace("<image-path>", path.to_str().unwrap())
                );
                assert_eq!(world.runtime.pending(), 1);
                std::fs::remove_file(path).unwrap();
            }
        },
    );
}

/// An empty clipboard.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct NoImage {
    /// Always `paste-none`.
    op: String,
}

/// The editor text.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Text {
    /// The editor text.
    text: String,
}

#[cfg(target_os = "linux")]
#[test]
fn clipboard_absence_is_silent_and_does_not_edit() {
    isolated(
        "clipboard_absence_is_silent_and_does_not_edit",
        |directory| {
            let (input, expected) =
                cases::<NoImage, Text>("clipboard_absence_is_silent_and_does_not_edit")
                    .swap_remove(0);
            assert_eq!(input.op, "paste-none");
            let helper = install(directory, "wl-paste", WL_PASTE);
            std::fs::write(helper.with_file_name("types"), "text/plain\n").unwrap();
            let world = started(directory);
            world.mode.editor.set_text(&expected.text);
            world.runtime.settle().unwrap();
            paste(&world);
            assert_eq!(world.mode.editor.get_text(), expected.text);
            assert_eq!(temporary_names(), Vec::<String>::new());
            assert_eq!(world.runtime.pending(), 0);
        },
    );
}

#[test]
fn clipboard_read_error_is_silent() {
    isolated("clipboard_read_error_is_silent", |directory| {
        let world = started(directory);
        world.mode.editor.set_text("before");
        world.runtime.settle().unwrap();
        world
            .mode
            .paste_clipboard_result(Err(io::Error::other("clipboard refused")));
        assert_eq!(world.mode.editor.get_text(), "before");
        assert_eq!(temporary_names(), Vec::<String>::new());
        assert_eq!(world.runtime.pending(), 0);
    });
}

/// A read image the writer receives.
fn image() -> ClipboardImage {
    ClipboardImage {
        bytes: vec![0, 255, 128, 10],
        mime_type: "image/png".into(),
    }
}

#[test]
fn clipboard_write_failure_is_silent() {
    isolated("clipboard_write_failure_is_silent", |directory| {
        let world = started(directory);
        world.mode.editor.set_text("before");
        world.runtime.settle().unwrap();
        std::fs::remove_dir(std::env::temp_dir()).unwrap();
        world.mode.paste_clipboard_result(Ok(Some(image())));
        assert!(!std::env::temp_dir().exists());
        assert_eq!(world.mode.editor.get_text(), "before");
        assert_eq!(world.runtime.pending(), 0);
    });
}

#[test]
fn temporary_files_preserve_existing_entries_and_private_bytes() {
    isolated(
        "temporary_files_preserve_existing_entries_and_private_bytes",
        |directory| {
            use std::os::unix::fs::{PermissionsExt, symlink};
            let editor = install(directory, "editor", EDITOR);
            let bin = editor.parent().unwrap();
            std::fs::write(bin.join("content"), "edited").unwrap();
            std::fs::write(bin.join("status"), "0").unwrap();
            let world = started(directory);
            world
                .runtime
                .set_environment("VISUAL", editor.to_str().unwrap());
            world.mode.editor.set_text("draft");
            let file = std::env::temp_dir().join(EDIT_FILE);
            let target = directory.join("target");
            std::fs::write(&target, "unrelated").unwrap();
            for existing in ["file", "link"] {
                if existing == "file" {
                    std::fs::write(&file, "unrelated").unwrap();
                } else {
                    symlink(&target, &file).unwrap();
                }
                run_action(&world, "app.editor.external");
                let errors = world.runtime.drain_futures();
                let [error] = errors.as_slice() else {
                    panic!("one error")
                };
                let error = error.downcast_ref::<io::Error>().unwrap();
                assert_eq!(error.kind(), io::ErrorKind::AlreadyExists);
                assert!(!bin.join("argv").exists());
                assert_eq!(std::fs::read_to_string(&file).unwrap(), "unrelated");
                assert_eq!(std::fs::read_to_string(&target).unwrap(), "unrelated");
                assert_eq!(world.mode.editor.get_text(), "draft");
                std::fs::remove_file(&file).unwrap();
            }
            run_action(&world, "app.editor.external");
            let mode = std::fs::metadata(bin.join("before"))
                .unwrap()
                .permissions()
                .mode();
            assert_eq!(mode & 0o7777, 0o600);
            assert_eq!(world.mode.editor.get_text(), "edited");
            assert_eq!(temporary_names(), Vec::<String>::new());
            world.mode.paste_clipboard_result(Ok(Some(image())));
            let [name] = temporary_names().try_into().unwrap();
            let pasted = std::env::temp_dir().join(name);
            let mode = std::fs::metadata(&pasted).unwrap().permissions().mode();
            assert_eq!(mode & 0o7777, 0o600);
            assert_eq!(std::fs::read(pasted).unwrap(), image().bytes);
        },
    );
}
