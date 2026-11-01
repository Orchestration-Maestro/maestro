//! Effective keyboard help through the current mode and its containing transcript.
use super::extensions::{idle, owners, register};
use super::{Setup, World, cases, isolated};
use maestro_app::ExtensionShortcut;
use maestro_tui::Component;
use serde::Deserialize;
use std::{collections::BTreeMap, path::Path, rc::Rc};

/// Recorded keyboard documents and complete containing-container frames.
const FIXTURES: &str = include_str!("../../../tests/fixtures/interactive_hotkeys.json");

/// A real registration's supplied metadata.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Registration {
    /// Missing and null descriptions fall back to the owning extension's path.
    description: Option<String>,
    /// The extension that registers this shortcut.
    extension_path: String,
}

/// Builds the actual file-configured manager and runner over supplied registrations.
fn configured(
    directory: &Path,
    config: &serde_json::Value,
    entries: Vec<(String, Registration)>,
    indent: Option<String>,
    setup: Setup,
) -> World {
    std::fs::write(
        directory.join("keybindings.json"),
        serde_json::to_vec(config).unwrap(),
    )
    .unwrap();
    let further = entries
        .iter()
        .map(|(_, entry)| (entry.extension_path.clone(), idle()))
        .collect();
    let world = Setup {
        configure: Box::new(move |settings| {
            if let Some(indent) = indent {
                settings.set_global_key("markdown", "codeBlockIndent", indent);
            }
        }),
        extension: Some(idle()),
        further,
        ..setup
    }
    .build(directory);
    for (owner, (key, entry)) in owners(&world).into_iter().skip(1).zip(entries) {
        register(
            &owner,
            ExtensionShortcut {
                shortcut: key,
                description: entry.description,
                extension_path: entry.extension_path,
                handler: Rc::new(|_| Box::pin(std::future::ready(Ok(())))),
            },
        );
    }
    world
}

/// Inputs to one help invocation.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct DocumentInput {
    /// The bindings loaded by the real manager.
    config: serde_json::Value,
    /// The extension entries in registration order.
    entries: Vec<(String, Registration)>,
    /// The configured code-block indentation.
    indent: Option<String>,
}

/// The complete document and its rendered card.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct DocumentExpected {
    /// The complete Markdown, including row order and empty bindings.
    markdown: String,
    /// The captured heading's styled text.
    title: String,
    /// Each width's complete containing-container frame.
    frames: BTreeMap<usize, Vec<String>>,
    /// The number of children in the card.
    children: usize,
    /// The Markdown factory's selected indentation.
    code_block_indent: String,
}

#[test]
fn hotkeys_document_keeps_rows_extensions_and_settings() {
    const TEST: &str = "hotkeys_document_keeps_rows_extensions_and_settings";
    isolated(TEST, async |directory| {
        let documents = cases::<DocumentInput, DocumentExpected>(FIXTURES, TEST);
        assert_eq!(documents.len(), 19);
        for (input, expected) in documents {
            let world = configured(
                directory,
                &input.config,
                input.entries,
                input.indent,
                Setup::default(),
            );
            assert_eq!(world.mode.hotkeys_markdown(), expected.markdown);
            world.mode.handle_hotkeys_command().unwrap();
            assert_eq!(
                world.mode.chat_container.children().borrow().len(),
                expected.children
            );
            assert_eq!(
                world
                    .mode
                    .get_markdown_theme_with_settings()
                    .code_block_indent,
                Some(expected.code_block_indent)
            );
            for (width, frame) in expected.frames {
                let actual = world.mode.chat_container.render(width);
                assert_eq!(
                    actual[2].trim_end_matches(' '),
                    format!(" {}", expected.title)
                );
                assert_eq!(actual, frame);
            }
            assert_eq!(world.runtime.pending(), 1);
        }
    });
}

/// Checks the containing container, never an isolated Markdown component.
fn assert_frames(world: &World, frames: &BTreeMap<usize, Vec<String>>) {
    for (width, expected) in frames {
        assert_eq!(world.mode.chat_container.render(*width), *expected);
    }
}

/// A recorded runner selection with real extension registrations.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct SelectionInput {
    /// The retained manager's file configuration.
    config: serde_json::Value,
    /// Loaded registrations in source order.
    registrations: Vec<Vec<(String, Registration)>>,
}

/// A complete selected document and its painted frames.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct SelectionExpected<T = (String, String)> {
    /// The complete Markdown.
    markdown: String,
    /// The complete containing frames.
    frames: BTreeMap<usize, Vec<String>>,
    /// The runner's ordered selected keys or key-description pairs.
    selected: Vec<T>,
}

/// Checks the real runner's ordered surviving records under retained reservation policy.
fn assert_selection(world: &World, expected: &SelectionExpected) {
    assert_eq!(world.mode.hotkeys_markdown(), expected.markdown);
    let selected = world
        .host
        .session()
        .extension_runner()
        .get_shortcuts(&world.mode.keybindings.get_effective_config());
    let actual: Vec<_> = selected
        .into_iter()
        .map(|(key, shortcut)| {
            let value = shortcut.read().unwrap();
            (
                key,
                value
                    .description
                    .as_ref()
                    .unwrap_or(&value.extension_path)
                    .clone(),
            )
        })
        .collect();
    assert_eq!(actual, expected.selected);
    world.mode.handle_hotkeys_command().unwrap();
    assert_frames(world, &expected.frames);
}

#[test]
fn hotkeys_observe_real_shortcut_selection_and_replacement() {
    const TEST: &str = "hotkeys_observe_real_shortcut_selection_and_replacement";
    isolated(TEST, async |directory| {
        let scenarios = cases::<SelectionInput, SelectionExpected>(FIXTURES, TEST);
        assert_eq!(scenarios.len(), 4);
        for (input, expected) in scenarios {
            let world = configured(
                directory,
                &input.config,
                input.registrations.into_iter().flatten().collect(),
                None,
                Setup::default(),
            );
            assert_selection(&world, &expected);
        }
    });
}

/// Opposite live-display and retained-reservation policies.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct IndependentInput {
    /// The interrupt key displayed from the global registry.
    global: String,
    /// The independently retained interrupt reservation.
    retained: String,
}

/// A supplied shortcut and its owning path.
fn entry(key: &str, description: Option<String>, path: &str) -> (String, Registration) {
    (
        key.to_owned(),
        Registration {
            description,
            extension_path: path.to_owned(),
        },
    )
}

#[test]
fn hotkeys_keep_display_registry_and_reserved_config_independent() {
    const TEST: &str = "hotkeys_keep_display_registry_and_reserved_config_independent";
    isolated(TEST, async |directory| {
        let scenarios = cases::<IndependentInput, SelectionExpected<String>>(FIXTURES, TEST);
        assert_eq!(scenarios.len(), 2);
        for (input, expected) in scenarios {
            let world = configured(
                directory,
                &serde_json::json!({"app.interrupt": input.retained}),
                vec![
                    entry("alt+y", Some("control".into()), "control.ts"),
                    entry("alt+x", Some("alternate".into()), "alternate.ts"),
                ],
                None,
                Setup::default(),
            );
            let display = maestro_app::KeybindingsManager::new(
                vec![(
                    "app.interrupt".into(),
                    Some(maestro_tui::KeybindingKeys::Single(input.global)),
                )],
                None,
            );
            maestro_tui::set_keybindings((*display).clone());
            assert_eq!(world.mode.hotkeys_markdown(), expected.markdown);
            let selected = world
                .host
                .session()
                .extension_runner()
                .get_shortcuts(&world.mode.keybindings.get_effective_config());
            assert_eq!(selected.into_keys().collect::<Vec<_>>(), expected.selected);
            world.mode.handle_hotkeys_command().unwrap();
            assert_frames(&world, &expected.frames);
        }
    });
}

/// A fenced description whose indentation reaches painted code.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct IndentInput {
    /// The selected indentation; absence and null use the same default.
    indent: Option<String>,
    /// The authored description containing a code block.
    description: String,
}

/// Complete frame observations without document metadata.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct FramesExpected {
    /// The frames at both recorded widths.
    frames: BTreeMap<usize, Vec<String>>,
}

#[test]
fn hotkeys_markdown_indent_reaches_rendered_code() {
    const TEST: &str = "hotkeys_markdown_indent_reaches_rendered_code";
    isolated(TEST, async |directory| {
        let scenarios = cases::<IndentInput, FramesExpected>(FIXTURES, TEST);
        assert_eq!(scenarios.len(), 3);
        for (input, expected) in scenarios {
            indent_case(directory, &input, &expected, false);
            if input.indent.is_none() {
                indent_case(directory, &input, &expected, true);
            }
        }
    });
}

/// Renders one indentation input, optionally publishing an explicit null setting.
fn indent_case(directory: &Path, input: &IndentInput, expected: &FramesExpected, null: bool) {
    let world = configured(
        directory,
        &serde_json::json!({}),
        vec![entry(
            "alt+z",
            Some(input.description.clone()),
            "markdown.ts",
        )],
        input.indent.clone(),
        Setup::default(),
    );
    if null {
        world
            .host
            .session()
            .settings_manager()
            .borrow_mut()
            .set_global_key("markdown", "codeBlockIndent", serde_json::Value::Null);
    }
    world.mode.handle_hotkeys_command().unwrap();
    assert_frames(&world, &expected.frames);
}

/// Whether this retained card includes extension rows.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ThemeInput {
    /// The selected extension presence.
    extensions: bool,
}

/// Complete frames surrounding a live mutation.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RetainedExpected {
    /// Frames before publication or append.
    before: BTreeMap<usize, Vec<String>>,
    /// The first frames after publication or append.
    after: BTreeMap<usize, Vec<String>>,
}

#[test]
fn hotkeys_retained_panel_uses_live_theme_without_changing_title() {
    const TEST: &str = "hotkeys_retained_panel_uses_live_theme_without_changing_title";
    isolated(TEST, async |directory| {
        let scenarios = cases::<ThemeInput, RetainedExpected>(FIXTURES, TEST);
        assert_eq!(scenarios.len(), 2);
        for (input, expected) in scenarios {
            let entries = if input.extensions {
                vec![entry("alt+z", Some("Inspect".into()), "inspect.ts")]
            } else {
                Vec::new()
            };
            let world = configured(
                directory,
                &serde_json::json!({}),
                entries,
                None,
                Setup::default(),
            );
            world.mode.handle_hotkeys_command().unwrap();
            assert_frames(&world, &expected.before);
            world.themes.set_theme_instance(super::light()).unwrap();
            world.mode.chat_container.invalidate();
            assert_frames(&world, &expected.after);
        }
    });
}

/// A shared description changed between help invocations.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct DescriptionInput {
    /// The captured first description.
    before: String,
    /// The value read by the next invocation.
    after: String,
}

#[test]
fn hotkeys_use_current_session_without_retaining_mode() {
    const TEST: &str = "hotkeys_use_current_session_without_retaining_mode";
    isolated(TEST, async |directory| {
        let scenarios = cases::<DescriptionInput, RetainedExpected>(FIXTURES, TEST);
        assert_eq!(scenarios.len(), 2);
        for (input, expected) in scenarios {
            let world = configured(
                directory,
                &serde_json::json!({}),
                vec![entry("alt+z", Some(input.before), "live.ts")],
                None,
                Setup::default(),
            );
            world.mode.handle_hotkeys_command().unwrap();
            assert_frames(&world, &expected.before);
            let owner = owners(&world).pop().unwrap();
            let shortcut = Rc::clone(owner.read().unwrap().shortcuts.get("alt+z").unwrap());
            shortcut.write().unwrap().description = Some(input.after);
            world.mode.handle_hotkeys_command().unwrap();
            assert_frames(&world, &expected.after);
        }
        replacement(directory).await;
    });
}

/// Replaces the session through the existing factory and retains only its painted cards.
async fn replacement(directory: &Path) {
    let second = configured(
        directory,
        &serde_json::json!({}),
        vec![entry("alt+y", Some("session B".into()), "second.ts")],
        Some("> ".into()),
        Setup::default(),
    );
    let factory = super::create(second.host.services(), false);
    drop(second);
    let world = configured(
        directory,
        &serde_json::json!({}),
        vec![entry("alt+z", Some("session A".into()), "first.ts")],
        Some(String::new()),
        Setup {
            create: Some(factory),
            ..Setup::default()
        },
    );
    let first = world.host.session();
    world.mode.handle_hotkeys_command().unwrap();
    let before = world.mode.chat_container.render(80);
    assert!(
        !world
            .host
            .new_session(maestro_app::NewSessionCommandOptions::default())
            .await
            .unwrap()
            .cancelled
    );
    assert!(!Rc::ptr_eq(&first, &world.host.session()));
    let markdown = world.mode.hotkeys_markdown();
    assert!(markdown.contains(&super::platform_keys::on_this_platform(
        "| `Alt+Y` | session B |"
    )));
    assert!(!markdown.contains("session A"));
    assert_eq!(
        world
            .mode
            .get_markdown_theme_with_settings()
            .code_block_indent
            .as_deref(),
        Some("> ")
    );
    world.mode.handle_hotkeys_command().unwrap();
    let retained = Rc::clone(&world.mode.chat_container);
    let after = retained.render(80);
    assert_eq!(&after[..before.len()], before);
    let weak = Rc::downgrade(&world.mode);
    drop(first);
    drop(world);
    assert!(weak.upgrade().is_none());
    assert_eq!(retained.render(80), after);
}
