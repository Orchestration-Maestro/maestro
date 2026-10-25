//! The full changelog command and the exact submission that runs it.
use super::typed::{poll, recalled, waiting};
use super::{Setup, VERSION, World, cases, isolated_package, painted, started};
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
        /// A directory at the file path makes the read fail on every Unix host.
        #[serde(default)]
        unreadable: bool,
        /// The configured code-block indentation.
        indent: Option<String>,
        /// The previously seen version.
        last: Option<String>,
        /// Whether the startup notice would be collapsed.
        collapse: Option<bool>,
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
    /// The command invocations after the submission.
    Dispatch {
        /// The command invocations the submission made.
        calls: usize,
    },
}

#[test]
fn changelog_command_uses_all_entries_and_empty_fallback() {
    const TEST: &str = "changelog_command_uses_all_entries_and_empty_fallback";
    let stderr = isolated_package(TEST, |directory, package| {
        for (input, expected) in cases::<Input, Expected>(FIXTURES, TEST) {
            let (
                Input::Command {
                    contents,
                    palette,
                    width,
                    unreadable,
                    indent,
                    last,
                    collapse,
                },
                Expected::Command { lines, renders },
            ) = (input, expected)
            else {
                panic!("a command case and its observation disagree");
            };
            let path = package.join("CHANGELOG.md");
            std::fs::write(&path, "").unwrap();
            let world = Setup {
                configure: Box::new(move |settings| {
                    settings.set_theme(palette);
                    settings.set_global_key(
                        "markdown",
                        "codeBlockIndent",
                        indent.unwrap_or_else(|| "  ".to_owned()),
                    );
                    settings.set_last_changelog_version(last.unwrap_or_default());
                    settings.set_collapse_changelog(collapse.unwrap_or(false));
                }),
                ..Setup::default()
            }
            .build(directory);
            world.mode.init().unwrap();
            world.runtime.settle().unwrap();
            if unreadable {
                std::fs::remove_file(&path).unwrap();
                std::fs::create_dir(&path).unwrap();
            } else {
                std::fs::write(&path, contents).unwrap();
            }

            world.mode.handle_changelog_command().unwrap();
            assert_eq!(world.runtime.pending(), renders);
            let theme = world.themes.theme().get().unwrap();
            assert_eq!(
                world.mode.chat_container.render(width),
                painted(&theme, &lines)
            );
        }
    });
    if !maestro_test_process::selection().is_child() {
        let error = std::fs::read(".").unwrap_err();
        assert_eq!(
            String::from_utf8(stderr).unwrap(),
            format!("Warning: Could not parse changelog: {error}\n")
        );
    }
}

#[test]
fn changelog_command_dispatch_is_exact_and_never_prompts() {
    const TEST: &str = "changelog_command_dispatch_is_exact_and_never_prompts";
    isolated_package(TEST, |directory, _package| {
        let (_, Expected::Command { lines: fallback, .. }) =
            cases::<Input, Expected>(FIXTURES, "changelog_command_uses_all_entries_and_empty_fallback")
                .into_iter()
                .find(|(input, _)| matches!(input, Input::Command { contents, palette, unreadable: false, .. } if contents.is_empty() && palette == "dark"))
                .unwrap()
        else {
            panic!("the empty changelog has recorded rows");
        };
        for (input, expected) in cases::<Input, Expected>(FIXTURES, TEST) {
            let (Input::Dispatch { text }, Expected::Dispatch { calls }) = (input, expected) else {
                panic!("a dispatch case and its observation disagree");
            };
            // The package holds no changelog, so the command always appends its fallback card.
            let world = started(directory);
            let recorded = submitted(&world, &text);
            let rows = world.mode.chat_container.children().borrow().len();
            assert!(world.mode.editor.get_text().is_empty(), "{text:?}");
            if calls > 0 {
                assert_eq!(rows, recorded.transcript + 6, "{text:?}");
                let theme = world.themes.theme().get().unwrap();
                let rendered = world.mode.chat_container.render(80);
                assert!(rendered.ends_with(&painted(&theme, &fallback)), "{text:?}");
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

/// A recorded selection input.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct SelectionInput {
    /// The recorded operation name.
    op: String,
    /// Existing agent messages.
    messages: usize,
    /// Previously recorded version.
    last: Option<String>,
    /// Packaged release history.
    changelog: String,
}

/// The selected content and stored version.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct SelectionExpected {
    /// Content selected for startup.
    markdown: Option<String>,
    /// Saved version after selection.
    last: Option<String>,
    /// Versions reported by the source.
    reports: Vec<String>,
}

#[test]
fn changelog_startup_selects_new_entries_and_records_seen_version() {
    const TEST: &str = "changelog_startup_selects_new_entries_and_records_seen_version";
    isolated_package(TEST, |directory, package| {
        super::on_tasks(async {
            for (input, expected) in cases::<SelectionInput, SelectionExpected>(FIXTURES, TEST) {
                selection_case(directory, package, input, expected).await;
            }
        });
    });
}

/// Runs one selection through initialization and the actual reporting transport.
async fn selection_case(
    directory: &std::path::Path,
    package: &std::path::Path,
    input: SelectionInput,
    expected: SelectionExpected,
) {
    assert_eq!(input.op, "select");
    std::fs::write(package.join("CHANGELOG.md"), input.changelog).unwrap();
    let last = input.last;
    let (fetch, requests) = reporting_transport();
    let world = Setup {
        options: crate::InteractiveModeOptions {
            service_origin: Some("https://service.invalid".into()),
            fetch: Some(fetch),
            ..Default::default()
        },
        configure: Box::new(move |settings| {
            settings.set_enable_install_telemetry(true);
            if let Some(last) = last {
                settings.set_last_changelog_version(last);
            }
        }),
        ..Setup::default()
    }
    .build(directory);
    seed_current_messages(&world, input.messages);
    REPORT_SETTINGS.with(|slot| {
        *slot.borrow_mut() = Some(world.host.session().settings_manager().clone());
    });
    world.mode.init().unwrap();
    REPORT_SETTINGS.with(|slot| {
        slot.borrow_mut().take();
    });
    let selected = world.mode.changelog_markdown.borrow().clone();
    assert_eq!(selected, expected.markdown);
    let version = expected
        .last
        .map(|value| value.replace("<version>", VERSION));
    assert_eq!(
        world
            .host
            .session()
            .settings_manager()
            .borrow()
            .get_last_changelog_version(),
        version
    );
    let reports: Vec<_> = expected
        .reports
        .iter()
        .map(|version| {
            format!(
                "https://service.invalid/api/report-install?version={}",
                version.replace("<version>", VERSION)
            )
        })
        .collect();
    assert_eq!(*requests.lock().unwrap(), reports);
    for work in world.runtime.take_futures() {
        work.await.unwrap();
    }
}

/// Seeds the actual current agent state, independently of session-file records.
fn seed_current_messages(world: &World, count: usize) {
    if count > 0 {
        world.host.session().messages().write().unwrap().push(
            maestro_app::NativeAgentMessage::User(std::sync::Arc::new(std::sync::RwLock::new(
                maestro_app::UserMessage {
                    content: maestro_app::UserContent::Text("resumed".into()),
                    timestamp: 0.0,
                },
            ))),
        );
    }
}

/// One recorded startup card input.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct StartupCard {
    /// Recorded operation.
    op: String,
    /// Whether only the update sentence is displayed.
    collapse: bool,
    /// Text already in the transcript.
    initial: String,
    /// Selected release history.
    markdown: Option<String>,
    /// Selected palette.
    palette: String,
    /// Render width.
    width: usize,
}

/// Painted rows and requested frames.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct CardExpected {
    /// Full transcript rows.
    lines: Vec<String>,
    /// Frame requests from displaying the card.
    renders: usize,
}

#[test]
fn changelog_condensed_notice_keeps_heading_capture_and_fallback() {
    startup_cards("changelog_condensed_notice_keeps_heading_capture_and_fallback");
}

#[test]
fn changelog_expanded_notice_keeps_markdown_theme_padding_and_borders() {
    startup_cards("changelog_expanded_notice_keeps_markdown_theme_padding_and_borders");
}

/// Replays actual startup card observations through retained transcript components.
fn startup_cards(test: &str) {
    isolated_package(test, |directory, _| {
        for (input, expected) in cases::<StartupCard, CardExpected>(FIXTURES, test) {
            assert_eq!(input.op, "startup-card");
            let world = Setup {
                configure: Box::new(move |settings| {
                    settings.set_theme(input.palette);
                    settings.set_collapse_changelog(input.collapse);
                }),
                ..Setup::default()
            }
            .build(directory);
            if !input.initial.is_empty() {
                world
                    .mode
                    .chat_container
                    .add_child(std::rc::Rc::new(maestro_tui::Text::new(
                        input.initial,
                        0,
                        0,
                        None,
                    )));
            }
            *world.mode.changelog_markdown.borrow_mut() = input.markdown;
            world.mode.show_startup_notices_if_needed().unwrap();
            assert_eq!(world.runtime.pending(), expected.renders);
            let theme = world.themes.theme().get().unwrap();
            let lines: Vec<_> = expected
                .lines
                .iter()
                .map(|line| line.replace("<version>", VERSION))
                .collect();
            assert_eq!(
                world.mode.chat_container.render(input.width),
                painted(&theme, &lines)
            );
        }
    });
}

/// A stored preference's input domain.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct SavedInput {
    /// Operation name.
    op: String,
    /// JSON kind, including missing.
    kind: String,
    /// Raw value when present.
    value: serde_json::Value,
    /// Render width for a collapse preference.
    width: Option<usize>,
}

/// Selection after typed setting fallback.
#[derive(Deserialize)]
#[serde(untagged, deny_unknown_fields)]
enum SavedExpected {
    /// Selection and the recorded version.
    Selection {
        /// Selected history.
        markdown: Option<String>,
        /// Recorded version.
        last: String,
    },
    /// Complete painted collapse preference observation.
    Card(CardExpected),
}

#[test]
fn changelog_saved_settings_keep_typed_fallback_and_raw_values() {
    const TEST: &str = "changelog_saved_settings_keep_typed_fallback_and_raw_values";
    isolated_package(TEST, |directory, package| {
        let (base, _) = cases::<SelectionInput, SelectionExpected>(
            FIXTURES,
            "changelog_startup_selects_new_entries_and_records_seen_version",
        )
        .remove(0);
        std::fs::write(package.join("CHANGELOG.md"), base.changelog).unwrap();
        for (input, expected) in cases::<SavedInput, SavedExpected>(FIXTURES, TEST) {
            let raw = (input.kind != "missing").then_some(input.value);
            let selection = input.op == "saved-domain";
            let field = if selection {
                "lastChangelogVersion"
            } else {
                "collapseChangelog"
            };
            let world = Setup {
                configure: Box::new(move |settings| {
                    settings.set_global_key_path(field, &[], raw);
                    settings.set_global_key_path(
                        "untouched",
                        &[],
                        Some(serde_json::json!({"kept": 7})),
                    );
                }),
                ..Setup::default()
            }
            .build(directory);
            let settings = world.host.session().settings_manager().clone();
            let before = serde_json::to_value(settings.borrow().get_global_settings()).unwrap();
            match expected {
                SavedExpected::Selection { markdown, last } => {
                    assert!(selection && input.width.is_none());
                    world.mode.init().unwrap();
                    assert_eq!(*world.mode.changelog_markdown.borrow(), markdown);
                    assert_eq!(
                        settings.borrow().get_last_changelog_version(),
                        Some(last.replace("<version>", VERSION))
                    );
                }
                SavedExpected::Card(card) => {
                    assert_eq!(input.op, "collapse-domain");
                    *world.mode.changelog_markdown.borrow_mut() = Some("## 1.2.3\nbody".into());
                    world.mode.show_startup_notices_if_needed().unwrap();
                    let theme = world.themes.theme().get().unwrap();
                    assert_eq!(
                        world.mode.chat_container.render(input.width.unwrap()),
                        painted(&theme, &card.lines)
                    );
                    assert_eq!(world.runtime.pending(), card.renders);
                }
            }
            let after = serde_json::to_value(settings.borrow().get_global_settings()).unwrap();
            assert_eq!(
                after.get("collapseChangelog"),
                before.get("collapseChangelog")
            );
            assert_eq!(after["untouched"], serde_json::json!({"kept": 7}));
        }
    });
}

#[test]
fn changelog_startup_is_shown_once_after_resources() {
    const TEST: &str = "changelog_startup_is_shown_once_after_resources";
    isolated_package(TEST, |directory, package| {
        std::fs::write(package.join("CHANGELOG.md"), "## [3.2.1]\nSELECTED_HISTORY").unwrap();
        let world = Setup {
            configure: Box::new(|settings| settings.set_last_changelog_version("1.0.0".into())),
            context: vec![maestro_app::ContextFile {
                path: "/workspace/AGENTS.md".into(),
                content: "context".into(),
            }],
            ..Setup::default()
        }
        .build(directory);
        world
            .host
            .session()
            .session_manager()
            .borrow_mut()
            .append_message(
                serde_json::json!({"role":"user", "content":"REPLAYED_MESSAGE", "timestamp": 1}),
            )
            .unwrap();
        world.mode.init().unwrap();
        let rows = world.mode.chat_container.render(120);
        let resources = rows
            .iter()
            .position(|row| row.contains("AGENTS.md"))
            .unwrap();
        let history = rows
            .iter()
            .position(|row| row.contains("SELECTED_HISTORY"))
            .unwrap();
        let replay = rows
            .iter()
            .position(|row| row.contains("REPLAYED_MESSAGE"))
            .unwrap();
        assert!(resources < history && history < replay);
        world.mode.init().unwrap();
        world.mode.show_startup_notices_if_needed().unwrap();
        assert_eq!(world.mode.chat_container.render(120), rows);
        let empty = Setup::default().build(directory);
        empty.mode.show_startup_notices_if_needed().unwrap();
        *empty.mode.changelog_markdown.borrow_mut() = Some("late".into());
        empty.mode.show_startup_notices_if_needed().unwrap();
        assert!(empty.mode.chat_container.render(80).is_empty());
        let independent = Setup::default().build(directory);
        *independent.mode.changelog_markdown.borrow_mut() = Some("independent".into());
        independent.mode.show_startup_notices_if_needed().unwrap();
        assert!(
            independent
                .mode
                .chat_container
                .render(80)
                .iter()
                .any(|row| row.contains("independent"))
        );
    });
}

/// Reporting policy inputs captured from the environment and settings.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ReportInput {
    /// Recorded operation.
    op: String,
    /// Ambient offline spelling.
    offline: Option<String>,
    /// Stored opt-in preference.
    preference: bool,
    /// Exact environment override.
    #[serde(rename = "override")]
    override_value: Option<String>,
    /// Reported version, including encoding-sensitive text.
    version: String,
}

/// The consumed request URL.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ReportExpected {
    /// Complete request URL.
    url: String,
}

/// Captured URLs from the supplied native HTTP boundary.
type Requests = std::sync::Arc<std::sync::Mutex<Vec<String>>>;

std::thread_local! {
    /// The single isolated reporting transaction's callback-editable settings.
    static REPORT_SETTINGS: std::cell::RefCell<Option<std::rc::Rc<std::cell::RefCell<maestro_app::SettingsManager>>>> = const { std::cell::RefCell::new(None) };
}

/// A transport that records the actual request and fails without network access.
fn reporting_transport() -> (maestro_app::Fetch, Requests) {
    let requests = Requests::default();
    let seen = requests.clone();
    let fetch: maestro_app::Fetch = std::sync::Arc::new(move |request| {
        REPORT_SETTINGS.with(|slot| {
            let settings = slot.borrow().clone();
            if let Some(settings) = settings {
                assert_eq!(
                    settings.borrow().get_last_changelog_version().as_deref(),
                    Some(VERSION)
                );
                settings.borrow_mut().set_collapse_changelog(true);
            }
        });
        seen.lock().unwrap().push(request.url);
        Box::pin(async { Err(maestro_app::FetchError::Timeout) })
    });
    (fetch, requests)
}

#[test]
fn changelog_startup_reports_only_recorded_versions() {
    const TEST: &str = "changelog_startup_reports_only_recorded_versions";
    for (index, (input, expected)) in cases::<ReportInput, Vec<ReportExpected>>(FIXTURES, TEST)
        .into_iter()
        .enumerate()
    {
        let mut env = Vec::new();
        if let Some(value) = &input.offline {
            env.push(("MAESTRO_OFFLINE", value.as_str()));
        }
        if let Some(value) = &input.override_value {
            env.push(("MAESTRO_TELEMETRY", value.as_str()));
        }
        super::isolated_case(TEST, &index.to_string(), &env, |directory| {
            super::on_tasks(async {
                reporting_case(directory, &input, expected).await;
            });
        });
    }
}

/// Checks captured reporting policy directly and through once-only initialization.
async fn reporting_case(
    directory: &std::path::Path,
    input: &ReportInput,
    expected: Vec<ReportExpected>,
) {
    let preference = input.preference;
    assert_eq!(input.op, "report");
    let (fetch, requests) = reporting_transport();
    let world = Setup {
        options: crate::InteractiveModeOptions {
            service_origin: Some("https://service.invalid".into()),
            fetch: Some(fetch),
            ..Default::default()
        },
        configure: Box::new(move |settings| {
            settings.set_enable_install_telemetry(preference);
        }),
        ..Setup::default()
    }
    .build(directory);
    world.mode.report_install_telemetry(&input.version);
    let expected: Vec<_> = expected
        .into_iter()
        .map(|request| {
            request
                .url
                .replace("<service-origin>", "https://service.invalid")
        })
        .collect();
    assert_eq!(*requests.lock().unwrap(), expected);
    let before = requests.lock().unwrap().len();
    world.mode.init().unwrap();
    assert_eq!(requests.lock().unwrap().len() - before, expected.len());
    world.mode.init().unwrap();
    assert_eq!(requests.lock().unwrap().len() - before, expected.len());
    for work in world.runtime.take_futures() {
        work.await.unwrap();
    }
}
