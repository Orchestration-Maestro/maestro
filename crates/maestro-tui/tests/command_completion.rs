//! Command routing through the public provider.
/// Shared controlled completion fixtures.
pub mod fixtures {
    pub mod completion;
    pub mod futures;
}
use fixtures::completion::{
    ApplicationCase, CommandInput, Expected, FileInput, Files, SuggestionCase, item,
    run_applications, run_suggestions,
};
use maestro_tui::autocomplete::{CursorPosition, DirectoryEntryKind};
use maestro_tui::{AutocompleteProvider, CombinedAutocompleteProvider};

#[test]
fn commands_route_and_preserve_argument_results() {
    run_suggestions(COMMANDS_ROUTE_AND_PRESERVE_ARGUMENT_RESULTS_CASES);
}

#[test]
fn abort_does_not_suppress_command_callbacks() {
    run_suggestions(ABORT_DOES_NOT_SUPPRESS_COMMAND_CALLBACKS_CASES);
}

#[test]
fn cursor_selects_only_the_current_prefix() {
    run_suggestions(CURSOR_SELECTS_ONLY_THE_CURRENT_PREFIX_CASES);
}

#[test]
fn force_bypasses_slash_commands() {
    run_suggestions(FORCE_BYPASSES_SLASH_COMMANDS_CASES);
}

#[test]
fn tab_trigger_keeps_its_distinct_slash_rule() {
    for (text, expected) in [
        ("", true),
        ("/", false),
        ("/model", false),
        (" /model", false),
        ("/model ", false),
        ("/model /", true),
        ("/a/b", false),
        ("x /model", true),
        ("\u{feff}/model", false),
        ("\u{85}/model", true),
        ("/model\u{9}arg", false),
        ("/model\u{a0}arg", false),
        ("/model\u{a}arg", false),
        ("/model a", true),
    ] {
        let lines = [text.to_owned()];
        let provider =
            CombinedAutocompleteProvider::new(vec![], "/work".into(), None, Files::default());
        assert_eq!(
            provider.should_trigger_file_completion(
                &lines,
                CursorPosition {
                    line: 0,
                    col: text.len()
                }
            ),
            Some(expected),
            "{text:?}"
        );
    }
}

#[test]
fn argument_futures_propagate_failure_without_filesystem_fallback() {
    observe_argument_completion(true).expect("callback finishes");
    run_suggestions(ARGUMENT_FUTURES_PROPAGATE_FAILURE_WITHOUT_FILESYSTEM_FALLBACK_CASES);
}

#[test]
fn command_values_starting_with_slash_are_inserted_as_paths() {
    run_applications(COMMAND_VALUES_STARTING_WITH_SLASH_ARE_INSERTED_AS_PATHS_CASES);
}

/// Await an explicit callback-entered/release boundary on the same request.
fn observe_argument_completion(
    fail: bool,
) -> Result<(), maestro_tui::autocomplete::CompletionError> {
    use maestro_tui::autocomplete::{Command, CompletionOptions, SlashCommand};
    use std::cell::Cell;
    use std::rc::Rc;
    use std::task::{Context, Poll, Waker};
    let entered = Rc::new(Cell::new(false));
    let released = Rc::new(Cell::new(false));
    let signal = Cell::new(false);
    let complete = deferred_callback(&entered, &released, fail);
    let files = Files::default();
    let trace = Rc::clone(&files.trace);
    let provider = CombinedAutocompleteProvider::new(
        vec![Command::SlashCommand(SlashCommand {
            name: "load".into(),
            description: None,
            argument_hint: None,
            get_argument_completions: Some(complete),
        })],
        "/work".into(),
        None,
        files,
    );
    let lines = ["/load a".into()];
    let mut future = provider.get_suggestions(
        &lines,
        CursorPosition { line: 0, col: 7 },
        CompletionOptions {
            signal: &signal,
            force: None,
        },
    );
    let mut context = Context::from_waker(Waker::noop());
    assert!(future.as_mut().poll(&mut context).is_pending());
    assert!(entered.get());
    signal.set(true);
    released.set(true);
    let Poll::Ready(result) = future.as_mut().poll(&mut context) else {
        return Err(std::io::Error::other("release did not complete callback").into());
    };
    assert!(trace.borrow().is_empty());
    let expected = if fail {
        Err("deferred argument failure".to_owned())
    } else {
        Ok(Some(maestro_tui::AutocompleteSuggestions {
            prefix: "a".into(),
            items: vec![item("alpha", "alpha", None)],
        }))
    };
    assert_eq!(result.map_err(|error| error.to_string()), expected);
    Ok(())
}

#[test]
fn argument_results_wait_for_the_callback_completion() {
    observe_argument_completion(false).expect("callback finishes");
}

/// Controlled inputs and complete public outputs for argument futures propagate failure without filesystem fallback.
const ARGUMENT_FUTURES_PROPAGATE_FAILURE_WITHOUT_FILESYSTEM_FALLBACK_CASES: &[SuggestionCase] =
    &[SuggestionCase {
        id: "cmd_reject",
        lines: &["/fail x"],
        line: 0,
        col: 7,
        force: None,
        aborted: false,
        commands: CommandInput::Standard,
        files: FileInput {
            entries: &[],
            stat: &[],
            read_error: false,
            home_error: false,
            compare_error: false,
        },
        expected: Expected::Error("argument failure"),
        calls: &[&["arguments", "fail", "x"]],
    }];

/// Controlled inputs and complete public outputs for force bypasses slash commands.
const FORCE_BYPASSES_SLASH_COMMANDS_CASES: &[SuggestionCase] = &[SuggestionCase {
    id: "cmd_forced",
    lines: &["/he"],
    line: 0,
    col: 3,
    force: Some(true),
    aborted: false,
    commands: CommandInput::Standard,
    files: FileInput {
        entries: &[("hello.txt", DirectoryEntryKind::Other)],
        stat: &[],
        read_error: false,
        home_error: false,
        compare_error: false,
    },
    expected: Expected::Items("/he", &[("/hello.txt", "hello.txt", None)]),
    calls: &[&["read_dir", "/"]],
}];

/// Controlled inputs and complete public outputs for cursor selects only the current prefix.
const CURSOR_SELECTS_ONLY_THE_CURRENT_PREFIX_CASES: &[SuggestionCase] = &[SuggestionCase {
    id: "cmd_partial",
    lines: &["ignored", "/helloTAIL"],
    line: 1,
    col: 3,
    force: None,
    aborted: false,
    commands: CommandInput::Standard,
    files: FileInput {
        entries: &[],
        stat: &[],
        read_error: false,
        home_error: false,
        compare_error: false,
    },
    expected: Expected::Items(
        "/he",
        &[
            ("help", "help", Some("<topic> \u{2014} Help")),
            ("hello", "hello", None),
        ],
    ),
    calls: &[],
}];

/// Controlled inputs and complete public outputs for abort does not suppress command callbacks.
const ABORT_DOES_NOT_SUPPRESS_COMMAND_CALLBACKS_CASES: &[SuggestionCase] = &[SuggestionCase {
    id: "cmd_abort",
    lines: &["/load x"],
    line: 0,
    col: 7,
    force: None,
    aborted: true,
    commands: CommandInput::Standard,
    files: FileInput {
        entries: &[],
        stat: &[],
        read_error: false,
        home_error: false,
        compare_error: false,
    },
    expected: Expected::Items("x", &[("x!", "argument", Some("returned unchanged"))]),
    calls: &[&["arguments", "load", "x"]],
}];

/// Controlled inputs and complete public outputs for commands route and preserve argument results.
const COMMANDS_ROUTE_AND_PRESERVE_ARGUMENT_RESULTS_CASES: &[SuggestionCase] = &[
    SuggestionCase {
        id: "cmd_all",
        lines: &["/"],
        line: 0,
        col: 1,
        force: None,
        aborted: false,
        commands: CommandInput::Standard,
        files: FileInput {
            entries: &[],
            stat: &[],
            read_error: false,
            home_error: false,
            compare_error: false,
        },
        expected: Expected::Items(
            "/",
            &[
                ("help", "help", Some("<topic> \u{2014} Help")),
                ("hello", "hello", None),
                ("clear", "clear", Some("Clear")),
                ("hint", "hint", Some("<arg>")),
                ("blank", "blank", None),
                ("cl", "cl", None),
                ("load", "load", None),
                ("none", "none", None),
                ("empty", "empty", None),
                ("invalid", "invalid", None),
                ("fail", "fail", None),
                ("dup", "dup", None),
                ("dup", "dup", None),
            ],
        ),
        calls: &[],
    },
    SuggestionCase {
        id: "cmd_exact",
        lines: &["/cl"],
        line: 0,
        col: 3,
        force: None,
        aborted: false,
        commands: CommandInput::Standard,
        files: FileInput {
            entries: &[],
            stat: &[],
            read_error: false,
            home_error: false,
            compare_error: false,
        },
        expected: Expected::Items(
            "/cl",
            &[("cl", "cl", None), ("clear", "clear", Some("Clear"))],
        ),
        calls: &[],
    },
    SuggestionCase {
        id: "cmd_fuzzy",
        lines: &["/hl"],
        line: 0,
        col: 3,
        force: None,
        aborted: false,
        commands: CommandInput::Standard,
        files: FileInput {
            entries: &[],
            stat: &[],
            read_error: false,
            home_error: false,
            compare_error: false,
        },
        expected: Expected::Items(
            "/hl",
            &[
                ("help", "help", Some("<topic> \u{2014} Help")),
                ("hello", "hello", None),
            ],
        ),
        calls: &[],
    },
    SuggestionCase {
        id: "cmd_missing",
        lines: &["/zzz"],
        line: 0,
        col: 4,
        force: None,
        aborted: false,
        commands: CommandInput::Standard,
        files: FileInput {
            entries: &[],
            stat: &[],
            read_error: false,
            home_error: false,
            compare_error: false,
        },
        expected: Expected::None,
        calls: &[],
    },
    SuggestionCase {
        id: "cmd_tab",
        lines: &["/he\u{9}"],
        line: 0,
        col: 4,
        force: None,
        aborted: false,
        commands: CommandInput::Standard,
        files: FileInput {
            entries: &[],
            stat: &[],
            read_error: false,
            home_error: false,
            compare_error: false,
        },
        expected: Expected::Items(
            "/he\u{9}",
            &[
                ("help", "help", Some("<topic> \u{2014} Help")),
                ("hello", "hello", None),
            ],
        ),
        calls: &[],
    },
    SuggestionCase {
        id: "cmd_space_args",
        lines: &["/load  a $& \u{1f600}"],
        line: 0,
        col: 16,
        force: None,
        aborted: false,
        commands: CommandInput::Standard,
        files: FileInput {
            entries: &[],
            stat: &[],
            read_error: false,
            home_error: false,
            compare_error: false,
        },
        expected: Expected::Items(
            " a $& \u{1f600}",
            &[(" a $& \u{1f600}!", "argument", Some("returned unchanged"))],
        ),
        calls: &[&["arguments", "load", " a $& \u{1f600}"]],
    },
    SuggestionCase {
        id: "cmd_null",
        lines: &["/none x"],
        line: 0,
        col: 7,
        force: None,
        aborted: false,
        commands: CommandInput::Standard,
        files: FileInput {
            entries: &[],
            stat: &[],
            read_error: false,
            home_error: false,
            compare_error: false,
        },
        expected: Expected::None,
        calls: &[&["arguments", "none", "x"]],
    },
    SuggestionCase {
        id: "cmd_empty",
        lines: &["/empty x"],
        line: 0,
        col: 8,
        force: None,
        aborted: false,
        commands: CommandInput::Standard,
        files: FileInput {
            entries: &[],
            stat: &[],
            read_error: false,
            home_error: false,
            compare_error: false,
        },
        expected: Expected::None,
        calls: &[&["arguments", "empty", "x"]],
    },
    SuggestionCase {
        id: "cmd_no_callback",
        lines: &["/help x"],
        line: 0,
        col: 7,
        force: None,
        aborted: false,
        commands: CommandInput::Standard,
        files: FileInput {
            entries: &[],
            stat: &[],
            read_error: false,
            home_error: false,
            compare_error: false,
        },
        expected: Expected::None,
        calls: &[],
    },
    SuggestionCase {
        id: "cmd_unknown",
        lines: &["/missing x"],
        line: 0,
        col: 10,
        force: None,
        aborted: false,
        commands: CommandInput::Standard,
        files: FileInput {
            entries: &[],
            stat: &[],
            read_error: false,
            home_error: false,
            compare_error: false,
        },
        expected: Expected::None,
        calls: &[],
    },
    SuggestionCase {
        id: "cmd_case",
        lines: &["/Load x"],
        line: 0,
        col: 7,
        force: None,
        aborted: false,
        commands: CommandInput::Standard,
        files: FileInput {
            entries: &[],
            stat: &[],
            read_error: false,
            home_error: false,
            compare_error: false,
        },
        expected: Expected::None,
        calls: &[],
    },
    SuggestionCase {
        id: "cmd_first_duplicate",
        lines: &["/dup x"],
        line: 0,
        col: 6,
        force: None,
        aborted: false,
        commands: CommandInput::Standard,
        files: FileInput {
            entries: &[],
            stat: &[],
            read_error: false,
            home_error: false,
            compare_error: false,
        },
        expected: Expected::Items("x", &[("x!", "argument", Some("returned unchanged"))]),
        calls: &[&["arguments", "dup", "x"]],
    },
    SuggestionCase {
        id: "cmd_at_precedence",
        lines: &["/load @thing"],
        line: 0,
        col: 12,
        force: None,
        aborted: false,
        commands: CommandInput::Standard,
        files: FileInput {
            entries: &[],
            stat: &[],
            read_error: false,
            home_error: false,
            compare_error: false,
        },
        expected: Expected::None,
        calls: &[],
    },
    SuggestionCase {
        id: "cmd_empty_set",
        lines: &["/"],
        line: 0,
        col: 1,
        force: None,
        aborted: false,
        commands: CommandInput::Empty,
        files: FileInput {
            entries: &[],
            stat: &[],
            read_error: false,
            home_error: false,
            compare_error: false,
        },
        expected: Expected::None,
        calls: &[],
    },
    SuggestionCase {
        id: "cmd_empty_name",
        lines: &["/"],
        line: 0,
        col: 1,
        force: None,
        aborted: false,
        commands: CommandInput::EmptyName,
        files: FileInput {
            entries: &[],
            stat: &[],
            read_error: false,
            home_error: false,
            compare_error: false,
        },
        expected: Expected::Items("/", &[("", "", None)]),
        calls: &[],
    },
];

/// Insertion inputs with complete edited lines and byte cursor outputs.
const COMMAND_VALUES_STARTING_WITH_SLASH_ARE_INSERTED_AS_PATHS_CASES: &[ApplicationCase] = &[(
    "cmd_slash_value",
    &["/he"],
    CursorPosition { line: 0, col: 3 },
    "/he",
    ("/help", "/help", None),
    &["/help"],
    0,
    5,
)];

/// Create a callback whose completion is owned by the release barrier.
fn deferred_callback(
    entered: &std::rc::Rc<std::cell::Cell<bool>>,
    released: &std::rc::Rc<std::cell::Cell<bool>>,
    fail: bool,
) -> maestro_tui::autocomplete::ArgumentCompletions {
    let entered = std::rc::Rc::clone(entered);
    let released = std::rc::Rc::clone(released);
    std::rc::Rc::new(move |_| {
        let entered = std::rc::Rc::clone(&entered);
        let released = std::rc::Rc::clone(&released);
        Box::pin(deferred_result(entered, released, fail))
    })
}

/// Wait for release before returning the callback's own success or failure.
async fn deferred_result(
    entered: std::rc::Rc<std::cell::Cell<bool>>,
    released: std::rc::Rc<std::cell::Cell<bool>>,
    fail: bool,
) -> Result<Option<Vec<maestro_tui::AutocompleteItem>>, maestro_tui::autocomplete::CompletionError>
{
    entered.set(true);
    std::future::poll_fn(|_| {
        if released.get() {
            std::task::Poll::Ready(())
        } else {
            std::task::Poll::Pending
        }
    })
    .await;
    if fail {
        Err(std::io::Error::other("deferred argument failure").into())
    } else {
        Ok(Some(vec![item("alpha", "alpha", None)]))
    }
}
