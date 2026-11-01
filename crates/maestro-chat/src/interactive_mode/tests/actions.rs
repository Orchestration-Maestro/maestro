//! Editor actions through the mode's installed callbacks, its rendered editor and the
//! native editor, clipboard helper and temporary files of an isolated child.
use super::{
    World, border_row, dark, isolated, light, native, painted, rendered_border, select_level,
    started,
};
use crate::components::session_selector::list::tests::session_picker::decode_cases;
use maestro_app::utils::clipboard_image::ClipboardImage;
use maestro_app::{DiagnosticErrorInfo, Model, OAuthError, ScopedModel, ThinkingLevel};
use maestro_tui::{Component, tui::InputHandler};
use serde::{Deserialize, de::DeserializeOwned};
use std::{
    cell::RefCell,
    io,
    path::{Path, PathBuf},
    rc::Rc,
    sync::{Arc, RwLock},
};

/// Every recorded editor-action case.
const FIXTURES: &str = include_str!("../../../tests/fixtures/interactive_actions.json");

/// The width every editor frame is rendered at.
const WIDTH: usize = 80;

/// The recorded cases of `test`, typed by that witness.
fn cases<I: DeserializeOwned, E: DeserializeOwned>(test: &str) -> Vec<(I, E)> {
    decode_cases(FIXTURES, test)
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
        async |directory| {
            let world = started(directory).await;
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

#[test]
fn border_theme_change_refreshes_the_first_returned_frame() {
    isolated(
        "border_theme_change_refreshes_the_first_returned_frame",
        async |directory| {
            let world = started(directory).await;
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
pub(super) fn install(directory: &Path, name: &str, script: &str) -> PathBuf {
    let bin = directory.join("bin");
    let source = bin.join(format!("{name}.source"));
    std::fs::write(&source, script).unwrap();
    let target = bin.join(name);
    native::install_executable(&source, &target);
    std::fs::remove_file(source).unwrap();
    target
}

/// Runs the editor action the mode registered.
pub(super) fn run_action(world: &World, action: &str) {
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
        async |directory| {
            let (input, expected) = cases::<MissingEditor, Lines>(
                "external_edit_without_configuration_warns_without_native_work",
            )
            .swap_remove(0);
            assert_eq!(input.op, "external-missing");
            for configured in [None, Some("")] {
                let world = started(directory).await;
                if let Some(value) = configured {
                    world.runtime.set_environment("VISUAL", value);
                    world.runtime.set_environment("EDITOR", value);
                }
                world.mode.editor.set_text("kept");
                world.terminal.clear_writes();
                run_action(&world, "app.editor.external");
                let (rows, expected_rows) = notices(&world, &expected.lines);
                assert_eq!(rows, expected_rows);
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
        async |directory| {
            use std::os::unix::fs::PermissionsExt;
            let editor = install(directory, "editor", EDITOR);
            let bin = editor.parent().unwrap();
            for (input, expected) in cases::<ExternalEdit, Edited>(
                "external_edit_uses_expanded_text_and_preserves_the_exit_contract",
            ) {
                assert_eq!(input.op, "external");
                let world = started(directory).await;
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
                let restarted = restart_texts(&world);
                run_action(&world, "app.editor.external");
                assert_eq!(restarted.borrow().first(), Some(&expected.text));
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

/// The editor text at each terminal restart from now on, seen when the restart hides the
/// cursor, before any frame is written.
fn restart_texts(world: &World) -> Rc<RefCell<Vec<String>>> {
    let restarted = Rc::new(RefCell::new(Vec::new()));
    let (seen, editor) = (Rc::clone(&restarted), Rc::clone(&world.mode.editor));
    world.terminal.set_tap(move |data| {
        if data == "\x1b[?25l" {
            seen.borrow_mut().push(editor.get_text());
        }
    });
    restarted
}

/// A Wayland paste helper that lists the planned types and prints the planned image.
#[cfg(target_os = "linux")]
const WL_PASTE: &str = "#!/bin/sh\nif [ \"$1\" = --list-types ]; then /bin/cat \"${0%/*}/types\"; else /bin/cat \"${0%/*}/image\"; fi\n";

/// Runs the installed image paste and the host work it hands over.
#[cfg(target_os = "linux")]
fn paste(world: &World) {
    let on_paste = world.mode.editor.on_paste_image().unwrap();
    on_paste();
    assert!(world.runtime.drain_futures().is_empty());
}

/// One clipboard image type.
#[cfg(target_os = "linux")]
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
#[cfg(target_os = "linux")]
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
        async |directory| {
            use std::os::unix::fs::PermissionsExt;
            let helper = install(directory, "wl-paste", WL_PASTE);
            let bin = helper.parent().unwrap();
            for (input, expected) in cases::<Paste, Pasted>(
                "clipboard_paste_keeps_image_bytes_and_inserts_at_the_cursor",
            ) {
                assert_eq!(input.op, "paste");
                let world = started(directory).await;
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
#[cfg(target_os = "linux")]
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct NoImage {
    /// Always `paste-none`.
    op: String,
}

/// The editor text.
#[cfg(target_os = "linux")]
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
        async |directory| {
            let (input, expected) =
                cases::<NoImage, Text>("clipboard_absence_is_silent_and_does_not_edit")
                    .swap_remove(0);
            assert_eq!(input.op, "paste-none");
            let helper = install(directory, "wl-paste", WL_PASTE);
            std::fs::write(helper.with_file_name("types"), "text/plain\n").unwrap();
            let world = started(directory).await;
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
    isolated("clipboard_read_error_is_silent", async |directory| {
        let world = started(directory).await;
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
    isolated("clipboard_write_failure_is_silent", async |directory| {
        let world = started(directory).await;
        world.mode.editor.set_text("before");
        world.runtime.settle().unwrap();
        std::fs::remove_dir(std::env::temp_dir()).unwrap();
        world.mode.paste_clipboard_result(Ok(Some(image())));
        assert!(!std::env::temp_dir().exists());
        assert_eq!(world.mode.editor.get_text(), "before");
        assert_eq!(world.runtime.pending(), 0);
        #[cfg(target_os = "linux")]
        {
            std::fs::create_dir(std::env::temp_dir()).unwrap();
            let previous = file_size_limit(None);
            // One byte fits, so the file is created and the rest of the image fails to write.
            file_size_limit(Some("1"));
            let control = std::env::temp_dir().join("control");
            let error = std::fs::write(&control, "ab").unwrap_err();
            assert_eq!(error.kind(), io::ErrorKind::FileTooLarge);
            std::fs::remove_file(control).unwrap();
            world.mode.paste_clipboard_result(Ok(Some(image())));
            file_size_limit(Some(&previous));
            assert_eq!(temporary_names(), Vec::<String>::new());
            assert_eq!(world.mode.editor.get_text(), "before");
            assert_eq!(world.runtime.pending(), 0);
        }
    });
}

/// Reads this process's soft file-size limit, after setting it to `soft` when supplied.
#[cfg(target_os = "linux")]
fn file_size_limit(soft: Option<&str>) -> String {
    let mut command = std::process::Command::new("prlimit");
    command.arg(format!("--pid={}", std::process::id()));
    if let Some(soft) = soft {
        command.arg(format!("--fsize={soft}:"));
    } else {
        command.args(["--fsize", "--raw", "--noheadings", "--output=SOFT"]);
    }
    let output = command.output().unwrap();
    assert!(output.status.success(), "{output:?}");
    String::from_utf8(output.stdout).unwrap().trim().to_owned()
}

#[test]
fn temporary_files_preserve_existing_entries_and_private_bytes() {
    isolated(
        "temporary_files_preserve_existing_entries_and_private_bytes",
        async |directory| {
            use std::os::unix::fs::{PermissionsExt, symlink};
            let editor = install(directory, "editor", EDITOR);
            let bin = editor.parent().unwrap();
            std::fs::write(bin.join("content"), "edited").unwrap();
            std::fs::write(bin.join("status"), "0").unwrap();
            let world = started(directory).await;
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

/// Transcript rows rendered at the width the recorded notice rows were padded to.
fn notices(world: &World, expected: &[String]) -> (Vec<String>, Vec<String>) {
    let theme = world.themes.theme().get().unwrap();
    let expected = painted(&theme, expected);
    let width = expected
        .iter()
        .map(|row| maestro_tui::visible_width(row))
        .max()
        .unwrap_or_default();
    (world.mode.chat_container.render(width), expected)
}

/// Lets the session's model reason at every level, the highest included.
fn reasoning_everywhere(world: &World) {
    let model = world.host.session().model();
    let mut model = model.write().unwrap();
    model.reasoning = true;
    model.thinking_level_map = Some(serde_json::from_str(r#"{"xhigh": "xhigh"}"#).unwrap());
}

/// One thinking cycle, from the level before `level` in the model's order.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ThinkingCycle {
    /// Always `thinking`.
    op: String,
    /// The selected level, absent when the model cannot reason.
    level: Option<ThinkingLevel>,
}

#[test]
fn thinking_cycle_refreshes_only_after_a_supported_selection() {
    isolated(
        "thinking_cycle_refreshes_only_after_a_supported_selection",
        async |directory| {
            for (input, expected) in cases::<ThinkingCycle, Lines>(
                "thinking_cycle_refreshes_only_after_a_supported_selection",
            ) {
                assert_eq!(input.op, "thinking");
                let world = started(directory).await;
                if let Some(level) = &input.level {
                    reasoning_everywhere(&world);
                    let levels = world.host.session().get_available_thinking_levels();
                    let index = levels.iter().position(|known| known == level).unwrap();
                    select_level(
                        &world,
                        levels[(index + levels.len() - 1) % levels.len()].clone(),
                    );
                }
                world.mode.editor.set_border_color(Rc::new(str::to_owned));
                let footer = world.mode.footer.render(WIDTH);
                run_action(&world, "app.thinking.cycle");
                let (rows, expected_rows) = notices(&world, &expected.lines);
                assert_eq!(rows, expected_rows);
                let session = world.host.session();
                if let Some(level) = input.level {
                    let role = format!("@{}@━\x1b[39m", thinking_role(&level));
                    assert_eq!(session.thinking_level(), level);
                    assert_eq!(rendered_border(&world), border_row(&world, &role));
                } else {
                    assert_eq!(session.thinking_level(), ThinkingLevel::Off);
                    assert_eq!(rendered_border(&world), "─".repeat(WIDTH));
                    assert_eq!(world.mode.footer.render(WIDTH), footer);
                }
            }
        },
    );
}

/// The theme role of a thinking level's border.
fn thinking_role(level: &ThinkingLevel) -> String {
    let name = serde_json::to_value(level).unwrap();
    let name = name.as_str().unwrap();
    format!("thinking{}{}", name[..1].to_uppercase(), &name[1..])
}

/// A model cycle with no other candidate.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct NoCandidate {
    /// Always `model-none`.
    op: String,
    /// Whether the session has scoped models.
    scoped: bool,
    /// The cycle direction.
    direction: Direction,
}

/// A recorded cycle direction.
#[derive(Clone, Copy, Deserialize)]
#[serde(rename_all = "lowercase")]
enum Direction {
    /// The next model.
    Forward,
    /// The previous model.
    Backward,
}

impl Direction {
    /// The action bound to cycling this way.
    fn action(self) -> &'static str {
        match self {
            Self::Forward => "app.model.cycleForward",
            Self::Backward => "app.model.cycleBackward",
        }
    }
}

/// Runs the model cycle bound for `direction` and the host work it hands over.
fn cycle(world: &World, direction: Direction) {
    run_action(world, direction.action());
    assert!(world.runtime.drain_futures().is_empty());
}

/// The provider the cycled models belong to, authenticated by a runtime key.
const PROVIDER: &str = "anthropic";

/// A model of the authenticated provider, copied from the session's own descriptor.
fn model(world: &World, id: &str, name: &str, reasoning: bool) -> Arc<RwLock<Model>> {
    let mut model = world.host.session().model().read().unwrap().clone();
    model.provider = PROVIDER.into();
    model.id = id.into();
    model.name = name.into();
    model.reasoning = reasoning;
    Arc::new(RwLock::new(model))
}

/// Makes `models` the session's scoped models, authenticated, with the first selected.
fn scope(world: &World, models: Vec<ScopedModel>) {
    let session = world.host.session();
    world
        .host
        .services()
        .auth_storage
        .set_runtime_api_key(PROVIDER, "fixture");
    write_model(world, Arc::clone(&models[0].model));
    session.set_scoped_models(models);
}

/// Selects `model` as the session's agent model.
fn write_model(world: &World, model: Arc<RwLock<Model>>) {
    world.host.session().agent().state().write().unwrap().model = model;
}

#[test]
fn isolated_children_carry_no_ambient_credential_inputs() {
    // The home directory stands for every inherited input: credential files live below it.
    isolated(
        "isolated_children_carry_no_ambient_credential_inputs",
        async |_| {
            assert_eq!(std::env::var_os("HOME"), None);
        },
    );
}

#[test]
fn model_cycle_ignores_an_inherited_credential() {
    // A launcher's provider key would make built-in models available to the unscoped cycle.
    let test = "model_cycle_ignores_an_inherited_credential";
    let sibling = maestro_test_process::TestProcess::current(test)
        .unwrap()
        .name()
        .replace(
            test,
            "model_cycle_distinguishes_scope_and_available_exhaustion",
        );
    let output = maestro_test_process::TestProcess::named(sibling)
        .unwrap()
        .env("ANTHROPIC_API_KEY", "maestro-dummy-key")
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn model_cycle_distinguishes_scope_and_available_exhaustion() {
    isolated(
        "model_cycle_distinguishes_scope_and_available_exhaustion",
        async |directory| {
            for (input, expected) in cases::<NoCandidate, Lines>(
                "model_cycle_distinguishes_scope_and_available_exhaustion",
            ) {
                assert_eq!(input.op, "model-none");
                let world = started(directory).await;
                if input.scoped {
                    let only = model(&world, "only", "Only", false);
                    scope(
                        &world,
                        vec![ScopedModel {
                            model: only,
                            thinking_level: None,
                        }],
                    );
                }
                let selected = world.host.session().model();
                cycle(&world, input.direction);
                let (rows, expected_rows) = notices(&world, &expected.lines);
                assert_eq!(rows, expected_rows);
                assert!(Arc::ptr_eq(&world.host.session().model(), &selected));
            }
        },
    );
}

/// A model cycle onto a model with a name, reasoning and explicit level.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Selection {
    /// Always `model-selected`.
    op: String,
    /// The cycle direction.
    direction: Direction,
    /// Whether the selected model reasons.
    reasoning: bool,
    /// The level scoped with the selected model.
    level: ThinkingLevel,
    /// The selected model's display name.
    name: String,
}

#[test]
fn model_cycle_uses_the_selected_name_thinking_and_direction() {
    isolated(
        "model_cycle_uses_the_selected_name_thinking_and_direction",
        async |directory| {
            for (input, expected) in cases::<Selection, Lines>(
                "model_cycle_uses_the_selected_name_thinking_and_direction",
            ) {
                assert_eq!(input.op, "model-selected");
                let world = started(directory).await;
                let current = model(&world, "current-id", "Current", false);
                let [next, previous] =
                    [(); 2].map(|()| model(&world, "model-id", &input.name, input.reasoning));
                let scoped = |model: &Arc<RwLock<Model>>| ScopedModel {
                    model: Arc::clone(model),
                    thinking_level: Some(input.level.clone()),
                };
                scope(
                    &world,
                    vec![scoped(&current), scoped(&next), scoped(&previous)],
                );
                world.mode.editor.set_border_color(Rc::new(str::to_owned));
                cycle(&world, input.direction);
                let (rows, expected_rows) = notices(&world, &expected.lines);
                assert_eq!(rows, expected_rows);
                let session = world.host.session();
                let selected = match input.direction {
                    Direction::Forward => &next,
                    Direction::Backward => &previous,
                };
                assert!(Arc::ptr_eq(&session.model(), selected));
                let level = session.thinking_level();
                let role = format!("@{}@━\x1b[39m", thinking_role(&level));
                assert_eq!(rendered_border(&world), border_row(&world, &role));
                assert!(
                    world
                        .mode
                        .footer
                        .render(WIDTH)
                        .concat()
                        .contains("model-id")
                );
            }
        },
    );
}

/// A refused model cycle.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Refusal {
    /// Always `model-error`.
    op: String,
}

#[test]
fn model_cycle_renders_failures_without_success_effects() {
    isolated(
        "model_cycle_renders_failures_without_success_effects",
        async |directory| {
            let (input, expected) =
                cases::<Refusal, Lines>("model_cycle_renders_failures_without_success_effects")
                    .swap_remove(0);
            assert_eq!(input.op, "model-error");
            let world = started(directory).await;
            world.mode.editor.set_border_color(Rc::new(str::to_owned));
            let selected = world.host.session().model();
            let footer = world.mode.footer.render(WIDTH);
            world
                .host
                .services()
                .auth_storage
                .set_fallback_resolver(|_| {
                    Err(OAuthError::from(DiagnosticErrorInfo::error(
                        "cycle refused",
                    )))
                });
            cycle(&world, Direction::Forward);
            let (rows, expected_rows) = notices(&world, &expected.lines);
            assert_eq!(rows, expected_rows);
            assert!(Arc::ptr_eq(&world.host.session().model(), &selected));
            assert_eq!(rendered_border(&world), "─".repeat(WIDTH));
            assert_eq!(world.mode.footer.render(WIDTH), footer);
        },
    );
}

/// The status a thinking cycle shows when the model cannot reason.
const NO_THINKING: &str = "Current model does not support thinking";

/// The warning an external edit shows without a configured editor.
const NO_EDITOR: &str = "No editor configured";

/// The default keys of forward and backward model cycling and image paste, whose work
/// runs on the terminal runtime.
const DEFAULT_ASYNCHRONOUS: [&str; 3] = ["\x10", "\x1b[112;6u", "\x16"];

/// Whether the transcript shows `text`.
pub(super) fn shows(chat: &maestro_tui::Container, text: &str) -> bool {
    chat.render(WIDTH).concat().contains(text)
}

/// Presses each action's default key on a fresh mode and observes the action run.
async fn default_keys_run_every_action(directory: &Path) {
    let world = started(directory).await;
    world.mode.editor.handle_input("\x1b[Z");
    assert!(shows(&world.mode.chat_container, NO_THINKING));
    world.mode.editor.handle_input("\x07");
    assert!(shows(&world.mode.chat_container, NO_EDITOR));
    for (count, default) in (1..).zip(DEFAULT_ASYNCHRONOUS) {
        world.mode.editor.handle_input(default);
        assert_eq!(world.runtime.futures(), count);
    }
    assert!(world.runtime.drain_futures().is_empty());
}

/// Presses each action's default key on a mode whose actions are all remapped.
fn default_keys_do_nothing_once_remapped(world: &World) {
    for obsolete in ["\x1b[Z", "\x07"].iter().chain(&DEFAULT_ASYNCHRONOUS) {
        world.mode.editor.handle_input(obsolete);
    }
    assert_eq!(world.runtime.futures(), 0);
    assert!(!shows(&world.mode.chat_container, NO_THINKING));
    assert!(!shows(&world.mode.chat_container, NO_EDITOR));
}

#[test]
fn editor_actions_respect_remapped_keys_and_release_mode() {
    isolated(
        "editor_actions_respect_remapped_keys_and_release_mode",
        async |directory| {
            default_keys_run_every_action(directory).await;
            std::fs::write(
                directory.join("keybindings.json"),
                r#"{"app.thinking.cycle": "ctrl+y", "app.editor.external": "ctrl+w",
                    "app.model.cycleForward": "ctrl+u", "app.model.cycleBackward": "ctrl+k",
                    "app.clipboard.pasteImage": "ctrl+]"}"#,
            )
            .unwrap();
            let world = started(directory).await;
            default_keys_do_nothing_once_remapped(&world);
            let World { mode, runtime, .. } = world;
            let editor = Rc::clone(&mode.editor);
            let chat = Rc::clone(&mode.chat_container);
            editor.handle_input("\x19");
            assert!(shows(&chat, NO_THINKING));
            editor.handle_input("\x17");
            assert!(shows(&chat, NO_EDITOR));
            for (count, remapped) in (1..).zip(["\x15", "\x0b", "\x1d"]) {
                editor.handle_input(remapped);
                assert_eq!(runtime.futures(), count);
            }
            assert!(!shows(&chat, "Only one model available"));
            let weak = Rc::downgrade(&mode);
            drop(mode);
            assert!(weak.upgrade().is_some());
            assert!(runtime.drain_futures().is_empty());
            assert!(shows(&chat, "Only one model available"));
            assert!(weak.upgrade().is_none());
            for remapped in ["\x19", "\x17", "\x15", "\x0b", "\x1d"] {
                editor.handle_input(remapped);
            }
            assert_eq!(runtime.futures(), 0);
        },
    );
}
