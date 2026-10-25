//! Effective keyboard help through the current mode and its containing transcript.
use super::{Setup, cases, isolated};
use maestro_tui::Component;
use serde::Deserialize;
use std::collections::BTreeMap;

/// Recorded keyboard documents and complete containing-container frames.
const FIXTURES: &str = include_str!("../../../tests/fixtures/interactive_hotkeys.json");

/// Inputs to one help invocation.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct DocumentInput {
    /// The bindings loaded by the real manager.
    config: serde_json::Value,
    /// The selected extension entries, empty for built-in help.
    entries: Vec<serde_json::Value>,
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
        assert_eq!(documents.len(), 10);
        for (input, expected) in documents {
            assert!(input.entries.is_empty());
            std::fs::write(
                directory.join("keybindings.json"),
                serde_json::to_vec(&input.config).unwrap(),
            )
            .unwrap();
            let world = Setup {
                configure: Box::new(move |settings| {
                    settings.set_global_key("markdown", "codeBlockIndent", input.indent);
                }),
                ..Setup::default()
            }
            .build(directory);
            assert_eq!(
                super::InteractiveMode::hotkeys_markdown(),
                expected.markdown
            );
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
