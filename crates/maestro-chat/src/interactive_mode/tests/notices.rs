//! The full changelog command and the exact submission that runs it.
use super::typed::{poll, recalled, waiting};
use super::{Setup, World, cases, isolated_package, painted, started};
use maestro_app::is_whitespace_scalar;
use maestro_tui::Component;
use serde::Deserialize;

/// Every recorded changelog case.
const FIXTURES: &str = include_str!("../../../tests/fixtures/interactive_notices.json");

/// One recorded changelog operation.
#[derive(Deserialize)]
#[serde(tag = "op", rename_all = "camelCase", deny_unknown_fields)]
enum Input {
    /// The command run over a changelog file with these contents.
    Command {
        /// The changelog file's contents.
        contents: String,
        /// The theme name selected before the command runs.
        palette: String,
        /// The render width.
        width: usize,
    },
    /// One submitted editor text.
    Dispatch {
        /// The submitted text.
        text: String,
    },
}

/// What one recorded operation observed.
#[derive(Deserialize)]
#[serde(untagged, deny_unknown_fields)]
enum Expected {
    /// The transcript rows and the frame requests.
    Command {
        /// The transcript rows.
        lines: Vec<String>,
        /// Frames requested.
        renders: usize,
    },
    /// The command invocations and the reference editor's text after the submission.
    Dispatch {
        /// The command invocations the submission made.
        calls: usize,
        /// The text the reference editor was left with.
        editor: String,
    },
}

#[test]
fn changelog_command_uses_all_entries_and_empty_fallback() {
    const TEST: &str = "changelog_command_uses_all_entries_and_empty_fallback";
    isolated_package(TEST, |directory, package| {
        for (input, expected) in cases::<Input, Expected>(FIXTURES, TEST) {
            let (
                Input::Command {
                    contents,
                    palette,
                    width,
                },
                Expected::Command { lines, renders },
            ) = (input, expected)
            else {
                panic!("a command case and its observation disagree");
            };
            std::fs::write(package.join("CHANGELOG.md"), contents).unwrap();
            let world = Setup {
                configure: Box::new(move |settings| settings.set_theme(palette)),
                ..Setup::default()
            }
            .build(directory);
            world.mode.init().unwrap();
            world.runtime.settle().unwrap();
            world.mode.handle_changelog_command().unwrap();
            assert_eq!(world.runtime.pending(), renders);
            let theme = world.themes.theme().get().unwrap();
            assert_eq!(
                world.mode.chat_container.render(width),
                painted(&theme, &lines)
            );
        }
    });
}

#[test]
fn changelog_command_dispatch_is_exact_and_never_prompts() {
    const TEST: &str = "changelog_command_dispatch_is_exact_and_never_prompts";
    isolated_package(TEST, |directory, _package| {
        for (input, expected) in cases::<Input, Expected>(FIXTURES, TEST) {
            let (Input::Dispatch { text }, Expected::Dispatch { calls, editor }) =
                (input, expected)
            else {
                panic!("a dispatch case and its observation disagree");
            };
            // The package holds no changelog, so the command always appends its fallback card.
            let world = started(directory);
            let recorded = submitted(&world, &text);
            let rows = world.mode.chat_container.children().borrow().len();
            // The reference clears its editor only in the command branch; this editor clears
            // its own buffer before every callback, so only the branch distinction carries.
            assert_eq!(editor.is_empty(), calls > 0, "{text:?}");
            assert!(world.mode.editor.get_text().is_empty(), "{text:?}");
            if calls > 0 {
                assert_eq!(rows, recorded.transcript + 6, "{text:?}");
                assert_eq!(recorded.delivered, None, "{text:?}");
                assert_eq!(recorded.history, Vec::<String>::new(), "{text:?}");
            } else {
                assert_eq!(rows, recorded.transcript, "{text:?}");
                let trimmed = text.trim_matches(is_whitespace_scalar);
                assert_eq!(recorded.delivered.as_deref(), Some(trimmed), "{text:?}");
                assert_eq!(recorded.history, [trimmed], "{text:?}");
            }
        }
    });
}

/// What one editor submission changed, recorded before the submission.
struct Submitted {
    /// The transcript's child count before the submission.
    transcript: usize,
    /// The text delivered to the waiting input.
    delivered: Option<String>,
    /// The editor's history after the submission, newest first.
    history: Vec<String>,
}

/// Submits `text` through the installed editor and reports what it changed.
fn submitted(world: &World, text: &str) -> Submitted {
    let transcript = world.mode.chat_container.children().borrow().len();
    let mut waiter = waiting(&world.mode);
    world.mode.editor.set_text(text);
    world.terminal.send_input("\r");
    let delivered = poll(&mut waiter);
    Submitted {
        transcript,
        delivered,
        history: recalled(&world.mode.editor),
    }
}
