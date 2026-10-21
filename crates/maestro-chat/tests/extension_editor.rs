//! Multiline dialog behavior through public construction, input and child handles.
#![cfg(test)]
#![cfg(not(target_arch = "wasm32"))]
use crate::bindings;
use crate::input_support;
use crate::published;
use crate::recording_terminal;
use crate::runtime_spy;
use maestro_chat::{ExtensionEditorComponent, ExtensionEditorOptions};
use maestro_theme::{ColorMode, ColorValue, Theme, ThemeColor, ThemeOptions};
use maestro_tui::{Component, EditorOptions, Focusable, KeybindingsManager, TUI, TerminalImage};
use recording_terminal::RecordingTerminal;
use runtime_spy::Spy;
use serde::{Deserialize, de::DeserializeOwned};
use serde_json::Value;
use std::{cell::RefCell, io::Write, rc::Rc};

/// An attributed recording with strict membership.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Case {
    /// Owning public witness.
    test: String,
    /// Operands decoded by the witness.
    input: Value,
    /// Observations decoded by the witness.
    expected: Value,
}
/// Decodes each field through its owning schema.
fn cases<I: DeserializeOwned, E: DeserializeOwned>(name: &str) -> Vec<(I, E)> {
    let corpus: Vec<Case> =
        serde_json::from_str(include_str!("fixtures/extension_editor.json")).unwrap();
    assert_eq!(corpus.len(), 105);
    let mut identities = std::collections::HashSet::new();
    for row in &corpus {
        assert!(OWNERS.contains(&row.test.as_str()));
        assert!(identities.insert((&row.test, row.input.to_string())));
    }
    let result: Vec<_> = corpus
        .into_iter()
        .filter(|row| row.test == name)
        .map(|row| {
            (
                serde_json::from_value(row.input).unwrap(),
                serde_json::from_value(row.expected).unwrap(),
            )
        })
        .collect();
    assert!(!result.is_empty());
    result
}
/// Complete constructor operands for rendered recordings.
#[derive(Default, Deserialize)]
#[serde(deny_unknown_fields)]
struct RenderInput {
    /// Caller title.
    title: String,
    /// Viewport width.
    width: usize,
    /// Captured visual editor environment.
    #[serde(rename = "VISUAL")]
    visual: Option<String>,
    /// Captured fallback editor environment.
    #[serde(rename = "EDITOR")]
    editor: Option<String>,
}
/// Complete public constructor observation.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Snapshot {
    /// Every rendered row, including styles and padding.
    lines: Vec<String>,
    /// Retained editor text.
    text: String,
    /// Parent flag.
    focused: bool,
    /// Inner flag.
    child_focused: bool,
    /// Ordered callback payloads.
    events: Vec<Vec<String>>,
}
/// A complete dialog and its observable owners.
struct Host {
    /// Registered component.
    dialog: Rc<ExtensionEditorComponent>,
    /// Caller-owned writer.
    tui: Rc<TUI>,
    /// Controlled host and render transcript.
    spy: Spy,
    /// Controlled terminal.
    terminal: RecordingTerminal,
    /// Published style owner.
    theme: maestro_theme::ThemeState,
    /// Live supplied binding manager.
    manager: KeybindingsManager,
    /// Callback transcript.
    events: Rc<RefCell<Vec<Vec<String>>>>,
}
/// Exact custom foreground palette used by the recording.
fn colors(offset: u8) -> Rc<Theme> {
    Rc::new(
        Theme::new(
            [
                (ThemeColor::Border, ColorValue::Index(31 + offset)),
                (ThemeColor::BorderMuted, ColorValue::Index(32 + offset)),
                (ThemeColor::Accent, ColorValue::Index(33 + offset)),
                (ThemeColor::Dim, ColorValue::Index(35 + offset)),
                (ThemeColor::Muted, ColorValue::Index(36 + offset)),
            ],
            [],
            ColorMode::Truecolor,
            ThemeOptions::default(),
        )
        .unwrap(),
    )
}
/// Actual application definitions, not a replacement shortcut table.
fn manager() -> KeybindingsManager {
    KeybindingsManager::new(
        maestro_app::presentation_data::keybindings::KEYBINDINGS.clone(),
        vec![],
    )
}
/// Constructs against the controlled runtime and published custom theme.
fn dialog(input: &RenderInput, prefill: Option<String>, options: EditorOptions) -> Host {
    configured_dialog(input, prefill, options, manager())
}
/// Constructs with a distinct supplied application binding route.
fn configured_dialog(
    input: &RenderInput,
    prefill: Option<String>,
    options: EditorOptions,
    manager: KeybindingsManager,
) -> Host {
    let spy = Spy::default();
    for (key, value) in [("VISUAL", &input.visual), ("EDITOR", &input.editor)] {
        if let Some(value) = value {
            spy.clock.set_environment(key, value);
        }
    }
    let terminal = RecordingTerminal::new(80, 24);
    let tui = Rc::new(TUI::new(
        terminal.handle(),
        Rc::new(spy.clone()),
        TerminalImage::new(|_| None, || 1),
        Some(false),
    ));
    let (theme_state, theme) = published::published(&colors(0));
    let events = Rc::new(RefCell::new(Vec::new()));
    let submits = events.clone();
    let cancels = events.clone();
    let dialog = Rc::new(
        ExtensionEditorComponent::new(
            &tui,
            manager.clone(),
            input.title.clone(),
            ExtensionEditorOptions {
                prefill,
                on_submit: Rc::new(move |text| {
                    submits
                        .borrow_mut()
                        .push(vec!["submit".into(), text.into()]);
                }),
                on_cancel: Rc::new(move || cancels.borrow_mut().push(vec!["cancel".into()])),
                editor_options: options,
                theme,
                runtime: Rc::new(spy.clone()),
            },
        )
        .unwrap(),
    );
    Host {
        theme: theme_state,
        manager,
        dialog,
        tui,
        spy,
        terminal,
        events,
    }
}
/// Borrows the real editor through the inherited child array.
fn text(dialog: &ExtensionEditorComponent) -> String {
    let child = dialog.children().borrow()[4].clone();
    let editor = (&*child as &dyn std::any::Any)
        .downcast_ref::<maestro_tui::Editor>()
        .unwrap();
    editor.get_text()
}
#[test]
fn editor_dialog_renders_title_editor_and_optional_hint() {
    let _guard = input_support::globals();
    if isolated("editor_dialog_renders_title_editor_and_optional_hint") {
        return;
    }
    maestro_tui::set_keybindings(manager());
    for (input, expected) in
        cases::<RenderInput, Snapshot>("editor_dialog_renders_title_editor_and_optional_hint")
    {
        let host = dialog(&input, None, EditorOptions::default());
        let actual = visible_rows(host.dialog.render(input.width));
        assert_eq!(
            actual.len(),
            expected.lines.len(),
            "row count title={:?}",
            input.title
        );
        for (index, (actual, expected)) in actual.iter().zip(&expected.lines).enumerate() {
            assert_eq!(actual, expected, "row {index} title={:?}", input.title);
        }
        assert_eq!(text(&host.dialog), expected.text);
        assert_eq!(host.dialog.focus_flag().get(), expected.focused);
        assert_eq!(
            host.dialog.children().borrow()[4]
                .focusable()
                .unwrap()
                .focus_flag()
                .get(),
            expected.child_focused
        );
        assert_eq!(*host.events.borrow(), expected.events);
    }
}
/// Drops only invisible foreground styles on space-only rows, retaining inverse cells.
fn visible_rows(rows: Vec<String>) -> Vec<String> {
    rows.into_iter()
        .map(|row| {
            let plain = row.replace("\x1b[38;5;33m", "").replace("\x1b[39m", "");
            if plain.chars().all(|ch| ch == ' ') && !row.contains("\x1b[7m") {
                plain
            } else {
                row
            }
        })
        .collect()
}
#[test]
fn external_editor_preserves_existing_temp_file_and_symlink() {
    let _guard = input_support::globals();
    if isolated("external_editor_preserves_existing_temp_file_and_symlink") {
        return;
    }
    maestro_tui::set_keybindings(manager());
    let host = dialog(
        &RenderInput {
            title: "Edit text".into(),
            visual: Some("missing-editor-command".into()),
            ..RenderInput::default()
        },
        Some("original".into()),
        EditorOptions::default(),
    );
    let path = std::env::temp_dir().join("maestro-extension-editor-1767323045006.md");
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&path)
        .unwrap();
    file.write_all(b"unrelated saved text").unwrap();
    let result = host.dialog.handle_input("\x07");
    let preserved = std::fs::read(&path).unwrap();
    std::fs::remove_file(&path).unwrap();
    assert_eq!(
        result.unwrap_err().kind(),
        std::io::ErrorKind::AlreadyExists
    );
    assert_eq!(preserved, b"unrelated saved text");
    assert_eq!(host.terminal.events(), ["start"]);
    #[cfg(unix)]
    {
        use std::os::unix::fs::symlink;
        let target = std::env::temp_dir().join("unrelated-target");
        std::fs::write(&target, b"unrelated target bytes").unwrap();
        symlink(&target, &path).unwrap();
        let result = host.dialog.handle_input("\x07");
        assert_eq!(
            result.unwrap_err().kind(),
            std::io::ErrorKind::AlreadyExists
        );
        assert!(
            std::fs::symlink_metadata(&path)
                .unwrap()
                .file_type()
                .is_symlink()
        );
        assert_eq!(std::fs::read(&target).unwrap(), b"unrelated target bytes");
        std::fs::remove_file(&path).unwrap();
        install_script(
            "inspect-mode",
            "#!/bin/sh\nfor file; do :; done\n/usr/bin/stat -c '%a' \"$file\" > \"${0%/*}/mode\"\nexit 1\n",
        );
        host.spy.clock.set_environment("VISUAL", "inspect-mode");
        host.dialog.handle_input("\x07").unwrap();
        assert_eq!(
            std::fs::read_to_string(std::env::temp_dir().join("mode")).unwrap(),
            "600\n"
        );
        assert!(!path.exists());
    }
}
/// Runs native effects in a fresh process with a private temporary directory and PATH.
fn isolated(name: &str) -> bool {
    if maestro_test_process::selection().is_child()
        && maestro_test_process::selection().runs_case(name)
    {
        return false;
    }
    let scratch = maestro_test_directory::Scratch::new().unwrap();
    let directory = scratch.path();
    let output = maestro_test_process::TestProcess::current(name)
        .unwrap()
        .nocapture()
        .case(name)
        .env_remove("MAESTRO_EDITOR_FILE")
        .env("TMPDIR", directory)
        .env("TEMP", directory)
        .env("TMP", directory)
        .env("PATH", directory)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "isolated witness failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    true
}
/// Prefill-only constructor operands.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct PrefillInput {
    /// Optional caller buffer.
    prefill: Option<String>,
}
/// Standard recorded dialog dimensions and title.
fn ordinary_input() -> RenderInput {
    RenderInput {
        title: "Edit text".into(),
        width: 120,
        ..RenderInput::default()
    }
}
/// Checks every snapshot field against the actual retained editor and callback transcript.
fn snapshot(host: &Host, expected: &Snapshot) {
    let actual = visible_rows(host.dialog.render(120));
    assert_eq!(actual.len(), expected.lines.len());
    for (index, (actual, expected)) in actual.iter().zip(&expected.lines).enumerate() {
        assert_eq!(actual, expected, "row {index}");
    }
    assert_eq!(text(&host.dialog), expected.text);
    assert_eq!(host.dialog.focus_flag().get(), expected.focused);
    assert_eq!(
        host.dialog.children().borrow()[4]
            .focusable()
            .unwrap()
            .focus_flag()
            .get(),
        expected.child_focused
    );
    assert_eq!(*host.events.borrow(), expected.events);
}
#[test]
fn editor_dialog_initializes_only_nonempty_prefill() {
    let _guard = input_support::globals();
    maestro_tui::set_keybindings(manager());
    for (input, expected) in
        cases::<PrefillInput, Snapshot>("editor_dialog_initializes_only_nonempty_prefill")
    {
        let host = dialog(&ordinary_input(), input.prefill, EditorOptions::default());
        snapshot(&host, &expected);
    }
}
/// Named caller overrides.
type Bindings = std::collections::BTreeMap<String, bindings::Binding>;
/// An ordinary key event or an independently mutable owner.
#[derive(Deserialize)]
#[serde(untagged)]
enum Operation {
    /// Raw input bytes.
    Key(String),
    /// Live application manager mutation.
    App(AppOperation),
    /// Replaces the global registry rather than the retained manager.
    Global(GlobalOperation),
    /// Replaces the published style.
    Theme(ThemeOperation),
    /// Replaces the observed environment.
    Environment(EnvironmentOperation),
    /// Invalidates existing component caches.
    Invalidate(InvalidateOperation),
}
/// App manager replacement configuration.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct AppOperation {
    /// Live overrides.
    #[serde(rename = "appBindings")]
    values: Bindings,
}
/// Global registry replacement configuration.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct GlobalOperation {
    /// Replacement overrides.
    #[serde(rename = "replaceGlobal")]
    values: Bindings,
}
/// New style palette.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ThemeOperation {
    /// Palette offset.
    theme: u8,
}
/// Explicit invalidation request.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct InvalidateOperation {
    /// Recorded request.
    invalidate: bool,
}
/// Constructor and subsequent actions for direct editor recordings.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct OpsInput {
    /// Initial buffer.
    prefill: Option<String>,
    /// Captured external editor.
    #[serde(rename = "VISUAL")]
    visual: Option<String>,
    /// Global hint/input overrides.
    #[serde(default, rename = "globalBindings")]
    global: Bindings,
    /// Independent application overrides.
    #[serde(default, rename = "appBindings")]
    app: Bindings,
    /// Captured fallback editor.
    #[serde(rename = "EDITOR")]
    editor: Option<String>,
    /// Controlled child output, including an explicitly empty string.
    content: Option<String>,
    /// Controlled exit policy; absence differs from a recorded null status.
    #[serde(default)]
    status: RecordedStatus,
    /// Ordered operations.
    ops: Vec<Operation>,
}
/// Ordered complete observations, including the constructor frame.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Frames {
    /// State after each operation.
    frames: Vec<Snapshot>,
}
/// Applies one operation and records actual terminal lifecycle events in occurrence order.
fn operation(host: &Host, operation: &Operation) {
    let before = host.terminal.events().len();
    match operation {
        Operation::Key(data) => host.dialog.handle_input(data).unwrap(),
        Operation::Environment(value) => {
            for key in ["VISUAL", "EDITOR"] {
                host.spy
                    .clock
                    .set_environment(key, value.environment.get(key).map_or("", String::as_str));
            }
        }
        Operation::App(value) => host
            .manager
            .set_user_bindings(bindings::bindings(&value.values)),
        Operation::Global(value) => bindings::replace(&value.values),
        Operation::Theme(value) => {
            host.theme.set_theme_instance(colors(value.theme)).unwrap();
        }
        Operation::Invalidate(value) => {
            assert!(value.invalidate);
            host.dialog.invalidate();
        }
    }
    host.events.borrow_mut().extend(
        host.terminal.events()[before..]
            .iter()
            .map(|value| vec![(*value).into()]),
    );
}
/// Replays direct input while keeping constructor/global/application routes separate.
fn input_group(name: &str) {
    let _guard = input_support::globals();
    if isolated(name) {
        return;
    }
    #[cfg(unix)]
    install_editor("vi", b"edited\n", 0);
    for (input, expected) in cases::<OpsInput, Frames>(name) {
        bindings::replace(&input.global);
        let application = KeybindingsManager::new(
            maestro_app::presentation_data::keybindings::KEYBINDINGS.clone(),
            bindings::bindings(&input.app),
        );
        let host = configured_dialog(
            &RenderInput {
                visual: input.visual,
                editor: input.editor,
                ..ordinary_input()
            },
            input.prefill,
            EditorOptions::default(),
            application,
        );
        assert_eq!(expected.frames.len(), input.ops.len() + 1);
        assert!(input.content.is_none());
        assert!(matches!(input.status, RecordedStatus::Code(0)));
        snapshot(&host, &expected.frames[0]);
        for (op, expected) in input.ops.iter().zip(&expected.frames[1..]) {
            operation(&host, op);
            snapshot(&host, expected);
        }
    }
}

#[test]
fn editor_dialog_submits_through_editor_and_clears_buffer() {
    input_group("editor_dialog_submits_through_editor_and_clears_buffer");
}

#[test]
fn editor_dialog_delegates_newlines_paste_and_unicode() {
    input_group("editor_dialog_delegates_newlines_paste_and_unicode");
}

#[test]
fn editor_dialog_cancel_does_not_submit_or_edit() {
    input_group("editor_dialog_cancel_does_not_submit_or_edit");
}

#[test]
fn editor_dialog_keeps_independent_binding_routes() {
    input_group("editor_dialog_keeps_independent_binding_routes");
}

#[test]
fn editor_dialog_cancel_wins_external_binding_collision() {
    input_group("editor_dialog_cancel_wins_external_binding_collision");
}

#[test]
#[cfg(unix)]
fn editor_dialog_observes_live_bindings_but_captures_help() {
    input_group("editor_dialog_observes_live_bindings_but_captures_help");
}
/// Installs a controlled Unix editor through a child, never executing a freshly written inode.
#[cfg(unix)]
fn install_editor(name: &str, content: &[u8], status: i32) {
    install_script(name, &editor_script(content, status));
}
/// Builds the controlled edit effects and records actual argv plus original bytes.
#[cfg(unix)]
fn editor_script(content: &[u8], status: i32) -> String {
    let exit = if status < 0 {
        "kill -TERM $$".to_owned()
    } else {
        format!("exit {status}")
    };
    let mut escaped = String::new();
    for byte in content {
        std::fmt::Write::write_fmt(&mut escaped, format_args!("\\{byte:03o}")).unwrap();
    }

    format!(
        "#!/bin/sh\nfor file; do :; done\nprintf '%s\\0' \"$0\" \"$@\" > \"${{0%/*}}/argv\"\n/bin/cp \"$file\" \"${{0%/*}}/before\"\nprintf '{escaped}' > \"$file\"\n{exit}\n"
    )
}

#[test]
fn editor_dialog_refreshes_only_live_border_styles() {
    input_group("editor_dialog_refreshes_only_live_border_styles");
}
/// Typed editor constructor options, with no ignored fixture members.
#[derive(Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct RecordedOptions {
    /// Requested padding.
    padding_x: Option<f64>,
    /// Requested completion rows.
    autocomplete_max_visible: Option<f64>,
}
/// Options construction record.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct OptionsInput {
    /// Starting text.
    prefill: String,
    /// Options passed before the editor exists.
    options: RecordedOptions,
}
/// Normalized options and their rendered consumer.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct OptionsExpected {
    /// Complete rendered editor state.
    snapshot: Snapshot,
    /// Normalized horizontal padding.
    padding: f64,
    /// Clamped completion capacity.
    maximum: usize,
}
#[test]
fn editor_dialog_forwards_editor_options_at_construction() {
    let _guard = input_support::globals();
    maestro_tui::set_keybindings(manager());
    for (input, expected) in cases::<OptionsInput, OptionsExpected>(
        "editor_dialog_forwards_editor_options_at_construction",
    ) {
        let host = dialog(
            &ordinary_input(),
            Some(input.prefill),
            EditorOptions {
                padding_x: input.options.padding_x,
                autocomplete_max_visible: input.options.autocomplete_max_visible,
            },
        );
        let child = host.dialog.children().borrow()[4].clone();
        let editor = (&*child as &dyn std::any::Any)
            .downcast_ref::<maestro_tui::Editor>()
            .unwrap();
        assert_eq!(editor.get_padding_x().to_bits(), expected.padding.to_bits());
        assert_eq!(editor.get_autocomplete_max_visible(), expected.maximum);
        snapshot(&host, &expected.snapshot);
    }
}
/// Constructor record with no operands.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Empty {}
/// Independent parent/child flags and full child render.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct FocusFrame {
    /// Parent's owned state.
    parent: bool,
    /// Forwarding target state.
    child: bool,
    /// Exact child display.
    lines: Vec<String>,
}
/// Ordered focus setter observations.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct FocusExpected {
    /// Initial state and each setter result.
    frames: Vec<FocusFrame>,
}
#[test]
fn editor_dialog_focus_is_independent_and_forwarded_immediately() {
    let _guard = input_support::globals();
    for (_, expected) in cases::<Empty, FocusExpected>(
        "editor_dialog_focus_is_independent_and_forwarded_immediately",
    ) {
        let host = dialog(&ordinary_input(), None, EditorOptions::default());
        let child = host.dialog.children().borrow()[4].clone();
        assert_eq!(expected.frames.len(), 6);
        for (index, frame) in expected.frames.iter().enumerate() {
            match index {
                1 | 3 | 5 => host.dialog.focus_flag().set(true),
                2 => child.focusable().unwrap().focus_flag().set(false),
                4 => host.dialog.focus_flag().set(false),
                _ => {}
            }
            assert_eq!(host.dialog.focus_flag().get(), frame.parent);
            assert_eq!(child.focusable().unwrap().focus_flag().get(), frame.child);
            assert_eq!(child.render(80), frame.lines);
            if index == 4 {
                host.dialog.clear();
            }
        }
    }
}
/// A replacement visible collection has no editor but still receives callback effects.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Replacement {
    /// All visible output.
    lines: Vec<String>,
    /// Submitted retained-editor text.
    events: Vec<Vec<String>>,
}
/// Child-array replacement and restoration observations.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ChildrenExpected {
    /// Before removal.
    initial: Snapshot,
    /// Replacement collection while original editor still handles input.
    replaced: Replacement,
    /// Restored original collection, now showing the cleared editor.
    restored: Snapshot,
}
#[test]
fn editor_dialog_keeps_children_identity_and_retained_editor() {
    let _guard = input_support::globals();
    for (input, expected) in cases::<PrefillInput, ChildrenExpected>(
        "editor_dialog_keeps_children_identity_and_retained_editor",
    ) {
        let host = dialog(&ordinary_input(), input.prefill, EditorOptions::default());
        let saved = host.dialog.children();
        snapshot(&host, &expected.initial);
        host.dialog.clear();
        host.dialog.add_child(Rc::new(maestro_tui::Text::new(
            "replacement".into(),
            0,
            0,
            None,
        )));
        host.dialog.handle_input("after").unwrap();
        host.dialog.handle_input("\r").unwrap();
        assert_eq!(host.dialog.render(120), expected.replaced.lines);
        assert_eq!(*host.events.borrow(), expected.replaced.events);
        host.dialog.set_children(saved.clone());
        assert!(Rc::ptr_eq(&host.dialog.children(), &saved));
        snapshot(&host, &expected.restored);
    }
}
#[test]
fn editor_dialog_callbacks_allow_reentrant_input_and_render() {
    let _guard = input_support::globals();
    for (_, expected) in
        cases::<Empty, Snapshot>("editor_dialog_callbacks_allow_reentrant_input_and_render")
    {
        let host = dialog(&ordinary_input(), None, EditorOptions::default());
        let weak = Rc::downgrade(&host.dialog);
        let events = host.events.clone();
        let first = std::cell::Cell::new(true);
        let child = host.dialog.children().borrow()[4].clone();
        let editor = (&*child as &dyn std::any::Any)
            .downcast_ref::<maestro_tui::Editor>()
            .unwrap();
        editor.set_on_submit(Some(Rc::new(move |value| {
            events
                .borrow_mut()
                .push(vec!["submit".into(), value.into()]);
            if first.replace(false) {
                let dialog = weak.upgrade().unwrap();
                assert!(!dialog.render(120).is_empty());
                dialog.handle_input("again").unwrap();
                dialog.handle_input("\r").unwrap();
            }
        })));
        host.dialog.handle_input("first").unwrap();
        host.dialog.handle_input("\r").unwrap();
        snapshot(&host, &expected);
    }
}
/// Reached constructor failure without diagnostic text coupling.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Failed {
    /// Whether theme resolution failed.
    failed: bool,
}
#[test]
fn editor_dialog_constructor_returns_theme_failure() {
    let _guard = input_support::globals();
    for (_, expected) in cases::<Empty, Failed>("editor_dialog_constructor_returns_theme_failure") {
        let host = dialog(&ordinary_input(), None, EditorOptions::default());
        let result = ExtensionEditorComponent::new(
            &host.tui,
            manager(),
            "title".into(),
            ExtensionEditorOptions {
                prefill: None,
                on_submit: Rc::new(|_| panic!("constructor submitted")),
                on_cancel: Rc::new(|| panic!("constructor cancelled")),
                editor_options: EditorOptions::default(),
                theme: published::unpublished().theme(),
                runtime: Rc::new(host.spy.clone()),
            },
        );
        assert_eq!(result.is_err(), expected.failed);
        assert!(host.terminal.events().is_empty());
        assert!(host.events.borrow().is_empty());
    }
}
#[test]
fn editor_dialog_narrow_rows_fit_one_column() {
    let _guard = input_support::globals();
    let host = dialog(
        &ordinary_input(),
        Some("界 a".into()),
        EditorOptions {
            padding_x: Some(2.0),
            ..EditorOptions::default()
        },
    );
    let rows = host.dialog.render(1);
    assert!(!rows.is_empty());
    for row in rows {
        assert!(maestro_tui::visible_width(&row) <= 1);
    }
}
#[test]
fn editor_dialog_uses_published_theme_roles() {
    let _guard = input_support::globals();
    for name in ["dark", "light"] {
        let path = format!(
            "{}/../maestro-theme/assets/theme/{name}.json",
            env!("CARGO_MANIFEST_DIR")
        );
        let theme = Rc::new(
            maestro_theme::load_theme_from_path(&path, Some(ColorMode::Truecolor)).unwrap(),
        );
        let host = dialog(&ordinary_input(), None, EditorOptions::default());
        host.theme.set_theme_instance(theme.clone()).unwrap();
        for role in [
            ThemeColor::Accent,
            ThemeColor::Border,
            ThemeColor::BorderMuted,
            ThemeColor::Dim,
            ThemeColor::Muted,
        ] {
            assert!(!theme.fg(&role, "sample").unwrap().is_empty());
        }
        assert_eq!(
            host.dialog.render(80)[0],
            theme.fg(&ThemeColor::Border, &"─".repeat(80)).unwrap()
        );
    }
}
#[test]
fn editor_dialog_focus_routes_real_writer_cursor() {
    let _guard = input_support::globals();
    let host = dialog(
        &ordinary_input(),
        Some("cursor".into()),
        EditorOptions::default(),
    );
    host.tui.set_show_hardware_cursor(true).unwrap();
    host.tui.add_child(host.dialog.clone());
    host.tui.start().unwrap();
    host.tui.set_focus(Some(host.dialog.clone()));
    assert!(
        host.dialog
            .render(80)
            .iter()
            .any(|row| row.contains(maestro_tui::CURSOR_MARKER))
    );
    host.spy.clock.settle().unwrap();
    assert!(
        host.terminal
            .writes()
            .iter()
            .any(|row| row.contains("\x1b[?25h"))
    );
    assert!(
        host.terminal
            .writes()
            .iter()
            .all(|row| !row.contains(maestro_tui::CURSOR_MARKER))
    );
    host.tui.set_focus(None);
    assert!(
        host.dialog
            .render(80)
            .iter()
            .all(|row| !row.contains(maestro_tui::CURSOR_MARKER))
    );
    host.tui.stop().unwrap();
}
#[test]
fn editor_dialog_releases_writer_and_callback_captures() {
    let _guard = input_support::globals();
    let host = dialog(&ordinary_input(), None, EditorOptions::default());
    let capture = Rc::new(());
    let released = Rc::downgrade(&capture);
    let component = Rc::new(
        ExtensionEditorComponent::new(
            &host.tui,
            manager(),
            "title".into(),
            ExtensionEditorOptions {
                prefill: None,
                on_submit: Rc::new(move |_| {
                    std::hint::black_box(&capture);
                }),
                on_cancel: Rc::new(|| {}),
                editor_options: EditorOptions::default(),
                theme: host.theme.theme(),
                runtime: Rc::new(host.spy.clone()),
            },
        )
        .unwrap(),
    );
    let weak_component = Rc::downgrade(&component);
    let weak_writer = Rc::downgrade(&host.tui);
    host.tui.add_child(component.clone());
    drop(component);
    let survivor = host.dialog.clone();
    let runtime = host.spy.clone();
    drop(host);
    assert!(weak_writer.upgrade().is_none());
    assert!(weak_component.upgrade().is_none());
    assert!(released.upgrade().is_none());
    runtime.clock.set_environment("VISUAL", "must-not-spawn");
    let before = runtime.events.borrow().len();
    survivor.handle_input("\x07").unwrap();
    assert_eq!(runtime.events.borrow().len(), before);
    assert_eq!(runtime.clock.pending(), 0);
}
#[test]
fn editor_dialog_trait_reports_errors_through_runtime() {
    let _guard = input_support::globals();
    if isolated("editor_dialog_trait_reports_errors_through_runtime") {
        return;
    }
    maestro_tui::set_keybindings(manager());
    let host = dialog(
        &RenderInput {
            visual: Some("missing-editor".into()),
            ..ordinary_input()
        },
        None,
        EditorOptions::default(),
    );
    host.terminal
        .set_hide_error(Some(std::io::ErrorKind::PermissionDenied));
    assert_eq!(
        host.dialog.handle_input("\x07").unwrap_err().kind(),
        std::io::ErrorKind::PermissionDenied
    );
    assert_eq!(host.spy.clock.futures(), 0);
    host.dialog.input_handler().unwrap().handle_input("\x07");
    assert_eq!(host.spy.clock.futures(), 1);
    let errors = host.spy.clock.drain_futures();
    assert_eq!(errors.len(), 1);
    let error = errors.into_iter().next().unwrap();
    assert_eq!(
        error.downcast_ref::<std::io::Error>().unwrap().kind(),
        std::io::ErrorKind::PermissionDenied
    );
    assert_eq!(host.spy.clock.futures(), 0);
}
/// Host environment replacement with explicit empty/missing preservation.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct EnvironmentOperation {
    /// Complete new observed environment.
    environment: std::collections::BTreeMap<String, String>,
}
/// Exit status zero/nonzero or termination with no integer status.
#[derive(Deserialize)]
#[serde(untagged)]
enum RecordedStatus {
    /// Native exit code.
    Code(i32),
    /// Signal or absent native process status.
    Null(()),
}
impl Default for RecordedStatus {
    fn default() -> Self {
        Self::Code(0)
    }
}
/// Replays native edits below the component, using literal-named local executables.
#[cfg(unix)]
fn external_group(name: &str) {
    let _guard = input_support::globals();
    if isolated(name) {
        return;
    }
    for (index, (input, expected)) in cases::<OpsInput, Frames>(name).into_iter().enumerate() {
        let _ = std::fs::remove_file(std::env::temp_dir().join("argv"));
        let command = input.visual.clone();
        maestro_tui::set_keybindings(manager());
        assert!(input.global.is_empty() && input.app.is_empty());
        let content = input.content.as_deref().unwrap_or("edited\n").as_bytes();
        let status = match input.status {
            RecordedStatus::Code(code) => code,
            RecordedStatus::Null(()) => -15,
        };
        for executable in [
            "ed", "chosen", "fallback", "changed", "\"my", "ed\t-x", "missing",
        ] {
            install_editor(executable, content, status);
        }
        let host = dialog(
            &RenderInput {
                visual: input.visual,
                editor: input.editor,
                ..ordinary_input()
            },
            input.prefill,
            EditorOptions::default(),
        );
        assert_eq!(expected.frames.len(), input.ops.len() + 1);
        snapshot(&host, &expected.frames[0]);
        for (op, expected) in input.ops.iter().zip(&expected.frames[1..]) {
            operation(&host, op);
            snapshot(&host, expected);
        }
        if name == "external_editor_splits_only_ascii_spaces_and_sets_platform_shell" {
            assert_arguments(command.as_deref().unwrap());
        }
        if name == "external_editor_selects_visual_then_editor_without_trimming" {
            selected_executable(index);
        }
    }
}
#[test]
#[cfg(unix)]
fn external_editor_selects_visual_then_editor_without_trimming() {
    external_group("external_editor_selects_visual_then_editor_without_trimming");
}

#[test]
#[cfg(unix)]
fn external_editor_replaces_text_only_after_success_and_removes_one_lf() {
    external_group("external_editor_replaces_text_only_after_success_and_removes_one_lf");
}

#[test]
#[cfg(unix)]
fn external_editor_nonzero_signal_and_spawn_failure_preserve_text() {
    external_group("external_editor_nonzero_signal_and_spawn_failure_preserve_text");
}

#[test]
#[cfg(unix)]
fn external_editor_samples_environment_at_each_invocation() {
    external_group("external_editor_samples_environment_at_each_invocation");
}
/// Checks the actual executable and argv against the independently recorded split table.
#[cfg(unix)]
fn assert_arguments(command: &str) {
    let expected: Option<(&str, &[&str])> = match command {
        "ed" => Some(("ed", &[])),
        "ed -w file" => Some(("ed", &["-w", "file"])),
        "ed  --wait " => Some(("ed", &["", "--wait", ""])),
        " ed" => None,
        "\"my editor\" -x" => Some(("\"my", &["editor\"", "-x"])),
        "ed\t-x" => Some(("ed\t-x", &[])),
        _ => panic!("unexpected split-table row"),
    };
    let observed = std::fs::read(std::env::temp_dir().join("argv")).ok();
    assert_eq!(observed.is_some(), expected.is_some());
    if let Some((executable, arguments)) = expected {
        let observed = observed.unwrap();
        let fields: Vec<_> = observed
            .strip_suffix(&[0])
            .unwrap()
            .split(|byte| *byte == 0)
            .map(|field| std::str::from_utf8(field).unwrap())
            .collect();
        assert_eq!(
            std::path::Path::new(fields[0]).file_name().unwrap(),
            executable
        );
        assert_eq!(&fields[1..fields.len() - 1], arguments);
        assert_eq!(
            std::path::Path::new(fields.last().unwrap()),
            std::env::temp_dir().join("maestro-extension-editor-1767323045006.md")
        );
        assert!(!std::path::Path::new(fields.last().unwrap()).exists());
    }
}
#[test]
#[cfg(unix)]
fn external_editor_splits_only_ascii_spaces_and_sets_platform_shell() {
    external_group("external_editor_splits_only_ascii_spaces_and_sets_platform_shell");
}
/// Native error-path inputs below the component's own finally boundary.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
#[cfg(unix)]
struct FaultInput {
    /// Literal executable installed in the controlled PATH.
    #[serde(rename = "VISUAL")]
    visual: String,
    /// Original buffer.
    prefill: String,
    /// Reached effect failures.
    fault: String,
}
/// Failure precedence and complete dialog output.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
#[cfg(unix)]
struct FaultExpected {
    /// Whether the input returns an error.
    failed: bool,
    /// Winning source effect.
    failure_phase: Option<String>,
    /// Complete post-failure state.
    snapshot: Snapshot,
}
/// Installs exact shell bytes through the same child-owned executable creation boundary.
#[cfg(unix)]
fn install_script(name: &str, source: &str) {
    let root = std::env::temp_dir();
    let source_path = root.join("script-source");
    std::fs::write(&source_path, source).unwrap();
    assert!(
        std::process::Command::new("/usr/bin/install")
            .args(["-m", "0700"])
            .arg(&source_path)
            .arg(root.join(name))
            .status()
            .unwrap()
            .success()
    );
    std::fs::remove_file(source_path).unwrap();
}
#[test]
#[cfg(unix)]
fn external_editor_finally_cleans_and_restarts_after_io_failures() {
    use std::os::unix::fs::PermissionsExt;
    let _guard = input_support::globals();
    if isolated("external_editor_finally_cleans_and_restarts_after_io_failures") {
        return;
    }
    maestro_tui::set_keybindings(manager());
    let root = std::env::temp_dir();
    for (input, expected) in cases::<FaultInput, FaultExpected>(
        "external_editor_finally_cleans_and_restarts_after_io_failures",
    ) {
        let host = dialog(
            &RenderInput {
                visual: Some(input.visual.clone()),
                ..ordinary_input()
            },
            Some(input.prefill),
            EditorOptions::default(),
        );
        install_editor(&input.visual, b"edited\n", 0);
        if input.fault.contains("read") {
            install_script(
                &input.visual,
                "#!/bin/sh\nfor file; do :; done\n/bin/rm \"$file\"\n",
            );
        }
        if input.fault == "unlink" {
            install_script(
                &input.visual,
                "#!/bin/sh\nfor file; do :; done\nprintf 'edited\\n' > \"$file\"\n/bin/chmod 0500 \"${0%/*}\"\n",
            );
        }
        if input.fault.contains("write") {
            std::fs::set_permissions(&root, std::fs::Permissions::from_mode(0o500)).unwrap();
        }
        if input.fault == "stop" {
            host.terminal
                .set_stop_error(Some(std::io::ErrorKind::BrokenPipe));
        }
        if input.fault.contains("start") {
            host.terminal
                .set_hide_error(Some(std::io::ErrorKind::ConnectionReset));
        }
        let result = host.dialog.handle_input("\x07");
        std::fs::set_permissions(&root, std::fs::Permissions::from_mode(0o700)).unwrap();
        assert_eq!(result.is_err(), expected.failed);
        let phase = result.as_ref().err().map(|error| match error.kind() {
            std::io::ErrorKind::PermissionDenied => "write",
            std::io::ErrorKind::BrokenPipe => "stop",
            std::io::ErrorKind::NotFound => "read",
            std::io::ErrorKind::ConnectionReset => "start",
            _ => panic!("unexpected native failure kind"),
        });
        assert_eq!(phase, expected.failure_phase.as_deref());
        host.events.borrow_mut().extend(
            host.terminal
                .events()
                .into_iter()
                .map(|event| vec![event.into()]),
        );
        snapshot(&host, &expected.snapshot);
        let pending = host.spy.events.borrow().len();
        assert_eq!(pending > 0, !input.fault.contains("start"));
        let _ = std::fs::remove_file(root.join("maestro-extension-editor-1767323045006.md"));
    }
}
/// Controlled native process scenario, not a simulated bridge response.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
#[cfg(target_os = "linux")]
struct NativeInput {
    /// Recording protocol.
    mode: String,
    /// Explicit executable/helper placeholders substituted at the native boundary.
    #[serde(rename = "EDITOR")]
    editor: String,
    /// Exact initial bytes.
    prefill: String,
    /// Native outcome.
    scenario: String,
}
/// Independent child observations in the native corpus.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
#[cfg(target_os = "linux")]
struct Observation {
    /// Exact file bytes before edit.
    before: String,
    /// All arguments before the appended path.
    args: Vec<String>,
    /// Identity of inherited descriptors.
    descriptors_inherited: bool,
}
/// Native error, cleanup, child observation and full output.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
#[cfg(target_os = "linux")]
struct NativeExpected {
    /// Whether a filesystem error escaped the action.
    failed: bool,
    /// Whether cleanup left a directory rather than a file.
    temporary_survived: bool,
    /// Absent only when native spawning failed.
    observation: Option<Observation>,
    /// Complete final state.
    snapshot: Snapshot,
}
/// Compares the child's inherited descriptors with the calling process's actual descriptors.
#[cfg(target_os = "linux")]
fn inherited_descriptors(expected: bool) {
    use std::os::unix::fs::MetadataExt;
    let root = std::env::temp_dir();
    let observed = std::fs::read_to_string(root.join("descriptors")).unwrap();
    let actual: Vec<_> = (0..3)
        .map(|fd| {
            let meta = std::fs::metadata(format!("/proc/self/fd/{fd}")).unwrap();
            format!(
                "{} {} {:x} {}",
                meta.dev(),
                meta.ino(),
                meta.mode(),
                meta.rdev()
            )
        })
        .collect();
    assert!(expected);
    assert_eq!(observed.lines().collect::<Vec<_>>(), actual);
}
#[test]
#[cfg(target_os = "linux")]
fn external_editor_native_command_roundtrip_and_cleanup() {
    let _guard = input_support::globals();
    if isolated("external_editor_native_command_roundtrip_and_cleanup") {
        return;
    }
    maestro_tui::set_keybindings(manager());
    let binding = &maestro_app::presentation_data::keybindings::KEYBINDINGS
        .iter()
        .find(|(id, _)| id == "app.editor.external")
        .unwrap()
        .1;
    assert_eq!(
        binding.default_keys,
        maestro_tui::KeybindingKeys::Single("ctrl+g".into())
    );
    assert_eq!(binding.description.as_deref(), Some("Open external editor"));
    for (input, expected) in
        cases::<NativeInput, NativeExpected>("external_editor_native_command_roundtrip_and_cleanup")
    {
        native_case(input, expected);
    }
}
/// Executes one observed OS outcome, retaining all original scenario distinctions.
#[cfg(target_os = "linux")]
fn native_case(input: NativeInput, expected: NativeExpected) {
    assert_eq!(input.mode, "native");
    let root = std::env::temp_dir();
    let path = root.join("maestro-extension-editor-1767323045006.md");
    for file in ["argv", "before", "descriptors"] {
        let _ = std::fs::remove_file(root.join(file));
    }
    let script = native_script(&input.scenario);
    install_script("native-editor", &script);
    let command = input
        .editor
        .replace("<node> <controlled-editor>", "native-editor")
        .replace("<temporary-directory>", root.to_str().unwrap());
    let host = dialog(
        &RenderInput {
            editor: Some(command),
            ..ordinary_input()
        },
        Some(input.prefill),
        EditorOptions::default(),
    );
    let result = host.dialog.handle_input("\x07");
    assert_eq!(result.is_err(), expected.failed);
    assert_eq!(path.exists(), expected.temporary_survived);
    if let Some(observation) = expected.observation {
        assert_eq!(
            std::fs::read(root.join("before")).unwrap(),
            observation.before.as_bytes()
        );
        let argv = std::fs::read(root.join("argv")).unwrap();
        let fields: Vec<_> = argv
            .strip_suffix(&[0])
            .unwrap()
            .split(|byte| *byte == 0)
            .collect();
        assert_eq!(
            &fields[1..fields.len() - 1],
            observation
                .args
                .iter()
                .map(String::as_bytes)
                .collect::<Vec<_>>()
        );
        assert_eq!(fields.last().unwrap(), &path.as_os_str().as_encoded_bytes());
        inherited_descriptors(observation.descriptors_inherited);
    } else {
        assert_eq!(input.scenario, "missing");
        assert!(!root.join("argv").exists());
    }
    host.events.borrow_mut().extend(
        host.terminal
            .events()
            .into_iter()
            .map(|event| vec![event.into()]),
    );
    snapshot(&host, &expected.snapshot);
    if expected.temporary_survived {
        std::fs::remove_dir(path).unwrap();
    }
}
/// The component-level Windows policy row; shell argument rows have their private unit owner.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
#[cfg(unix)]
struct PlatformInput {
    /// Recorded platform policy.
    platform: String,
    /// Literal controlled command.
    #[serde(rename = "VISUAL")]
    visual: String,
}
#[test]
#[cfg(unix)]
fn external_editor_windows_uses_shell_while_unix_executes_directly() {
    let _guard = input_support::globals();
    if isolated("external_editor_windows_uses_shell_while_unix_executes_directly") {
        return;
    }
    maestro_tui::set_keybindings(manager());
    let rows: Vec<Case> =
        serde_json::from_str(include_str!("fixtures/extension_editor.json")).unwrap();
    let rows: Vec<_> = rows
        .into_iter()
        .filter(|row| {
            row.test == "external_editor_windows_uses_shell_while_unix_executes_directly"
                && row.input.get("VISUAL").is_some()
        })
        .collect();
    assert_eq!(rows.len(), 1);
    for row in rows {
        let input: PlatformInput = serde_json::from_value(row.input).unwrap();
        let expected: Snapshot = serde_json::from_value(row.expected).unwrap();
        assert_eq!(input.platform, "win32");
        install_editor("ed", b"edited\n", 0);
        let host = dialog(
            &RenderInput {
                visual: Some(input.visual),
                ..ordinary_input()
            },
            None,
            EditorOptions::default(),
        );
        host.dialog.handle_input("\x07").unwrap();
        host.events.borrow_mut().extend(
            host.terminal
                .events()
                .into_iter()
                .map(|event| vec![event.into()]),
        );
        snapshot(&host, &expected);
        let argv = std::fs::read(std::env::temp_dir().join("argv")).unwrap();
        let fields: Vec<_> = argv
            .strip_suffix(&[0])
            .unwrap()
            .split(|byte| *byte == 0)
            .collect();
        assert_eq!(fields.len(), 3);
        assert_eq!(fields[1], b"--wait");
    }
}

/// Checks the independently recorded winning environment route.
#[cfg(unix)]
fn selected_executable(index: usize) {
    let executable = [None, None, Some("fallback"), Some("chosen"), None][index];
    let observed = std::fs::read(std::env::temp_dir().join("argv")).ok();
    assert_eq!(observed.is_some(), executable.is_some());
    if let Some(executable) = executable {
        assert_eq!(
            std::path::Path::new(
                std::str::from_utf8(
                    observed
                        .as_ref()
                        .unwrap()
                        .split(|byte| *byte == 0)
                        .next()
                        .unwrap()
                )
                .unwrap()
            )
            .file_name()
            .unwrap(),
            executable
        );
    }
}

/// Builds distinct native child outcomes below the product launch boundary.
#[cfg(target_os = "linux")]
fn native_script(scenario: &str) -> String {
    let content: &[u8] = if scenario == "invalid_utf8" {
        &[0xef, 0xbb, 0xbf, b'a', 0xff, b'\n']
    } else {
        b"new\n\n"
    };
    let status = match scenario {
        "nonzero" => 7,
        "signal" => -15,
        _ => 0,
    };
    let mut script = editor_script(content, status);
    script = script.replacen("for file; do :; done", "for file; do :; done\nexec 3<&0 4>&1 5>&2\n/usr/bin/stat -L -c '%d %i %f %r' /proc/$$/fd/3 /proc/$$/fd/4 /proc/$$/fd/5 > \"${0%/*}/descriptors\"", 1);
    for (name, effect) in [
        ("missing_read", "/bin/rm \"$file\""),
        (
            "cleanup_directory",
            "/bin/rm \"$file\"; /bin/mkdir \"$file\"",
        ),
    ] {
        if scenario == name {
            script = script.replace("exit 0", effect);
        }
    }
    script
}

/// Each recording has one named public witness; shell-boundary rows also have their private owner.
const OWNERS: [&str; 22] = [
    "editor_dialog_renders_title_editor_and_optional_hint",
    "editor_dialog_initializes_only_nonempty_prefill",
    "editor_dialog_submits_through_editor_and_clears_buffer",
    "editor_dialog_delegates_newlines_paste_and_unicode",
    "editor_dialog_cancel_does_not_submit_or_edit",
    "editor_dialog_keeps_independent_binding_routes",
    "editor_dialog_cancel_wins_external_binding_collision",
    "editor_dialog_observes_live_bindings_but_captures_help",
    "editor_dialog_forwards_editor_options_at_construction",
    "editor_dialog_focus_is_independent_and_forwarded_immediately",
    "editor_dialog_keeps_children_identity_and_retained_editor",
    "editor_dialog_refreshes_only_live_border_styles",
    "external_editor_selects_visual_then_editor_without_trimming",
    "external_editor_splits_only_ascii_spaces_and_sets_platform_shell",
    "external_editor_replaces_text_only_after_success_and_removes_one_lf",
    "external_editor_nonzero_signal_and_spawn_failure_preserve_text",
    "external_editor_samples_environment_at_each_invocation",
    "external_editor_finally_cleans_and_restarts_after_io_failures",
    "external_editor_native_command_roundtrip_and_cleanup",
    "editor_dialog_callbacks_allow_reentrant_input_and_render",
    "external_editor_windows_uses_shell_while_unix_executes_directly",
    "editor_dialog_constructor_returns_theme_failure",
];

#[test]
#[cfg(windows)]
fn external_editor_passes_environment_operand_to_native_child() {
    let _guard = input_support::globals();
    if isolated("external_editor_passes_environment_operand_to_native_child") {
        return;
    }
    maestro_tui::set_keybindings(manager());
    let host = dialog(
        &RenderInput {
            visual: Some("echo changed>".into()),
            ..ordinary_input()
        },
        Some("before".into()),
        EditorOptions::default(),
    );
    host.spy
        .clock
        .set_environment("comspec", &std::env::var("COMSPEC").unwrap());
    host.dialog.handle_input("\x07").unwrap();
    assert_eq!(text(&host.dialog), "changed\n");
    assert!(
        !std::env::temp_dir()
            .join("maestro-extension-editor-1767323045006.md")
            .exists()
    );
}
