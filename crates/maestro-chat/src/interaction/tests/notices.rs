//! The full changelog command and the exact submission that runs it.
use super::typed::{poll, recalled, waiting};
use super::{Setup, VERSION, World, cases, isolated_package, painted, started};
use maestro_app::is_whitespace_scalar;
use maestro_theme::ThemeColor;
use maestro_tui::{Component, TerminalCapabilities};
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
    let stderr = isolated_package(TEST, async |directory, package| {
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
            world.mode.init().await.unwrap();
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
    isolated_package(TEST, async |directory, _package| {
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
            let world = started(directory).await;
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
    isolated_package(TEST, async |directory, package| {
        for (input, expected) in cases::<SelectionInput, SelectionExpected>(FIXTURES, TEST) {
            selection_case(directory, package, input, expected).await;
        }
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
    world.mode.init().await.unwrap();
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
    /// Current code-block indentation when explicitly configured.
    indent: Option<String>,
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
    isolated_package(test, async |directory, _| {
        for (input, expected) in cases::<StartupCard, CardExpected>(FIXTURES, test) {
            assert_eq!(input.op, "startup-card");
            let world = Setup {
                configure: Box::new(move |settings| {
                    settings.set_theme(input.palette);
                    settings.set_collapse_changelog(input.collapse);
                    settings.set_global_key(
                        "markdown",
                        "codeBlockIndent",
                        input.indent.unwrap_or_else(|| "  ".to_owned()),
                    );
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
    isolated_package(TEST, async |directory, package| {
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
                    world.mode.init().await.unwrap();
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
    isolated_package(TEST, async |directory, package| {
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
        world.mode.init().await.unwrap();
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
        world.mode.init().await.unwrap();
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
        super::isolated_case(TEST, &index.to_string(), &env, async |directory| {
            reporting_case(directory, &input, expected).await;
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
    world.mode.init().await.unwrap();
    assert_eq!(requests.lock().unwrap().len() - before, expected.len());
    world.mode.init().await.unwrap();
    assert_eq!(requests.lock().unwrap().len() - before, expected.len());
    for work in world.runtime.take_futures() {
        work.await.unwrap();
    }
}

/// A recorded direct update card or a theme change over a retained card.
#[derive(Deserialize)]
#[serde(tag = "op", deny_unknown_fields)]
enum UpdateInput {
    /// A release card.
    #[serde(rename = "showNewVersionNotification")]
    Version {
        /// Version text supplied to the card.
        value: String,
        /// Selected palette.
        palette: String,
        /// Render width.
        width: usize,
    },
    /// A package card.
    #[serde(rename = "showPackageUpdateNotification")]
    Packages {
        /// Names in their supplied order.
        value: Vec<String>,
        /// Selected palette.
        palette: String,
        /// Render width.
        width: usize,
    },
    /// A previously prepared card after a theme change.
    #[serde(rename = "retained-update", rename_all = "camelCase")]
    Retained {
        /// The public render operation.
        method: String,
        /// Palette used while creating the text.
        before_palette: String,
        /// Palette used while rendering the live borders again.
        after_palette: String,
        /// Render width.
        width: usize,
    },
}

/// Full direct or retained card output.
#[derive(Deserialize)]
#[serde(untagged, deny_unknown_fields)]
enum UpdateExpected {
    /// One direct display.
    Direct(CardExpected),
    /// Displays before and after changing the theme.
    Retained {
        /// Rows before the theme change.
        before: Vec<String>,
        /// Rows after the theme change.
        after: Vec<String>,
        /// Frame requests from creating the card.
        renders: usize,
    },
}

#[test]
fn startup_version_notice_keeps_action_link_roles_and_redraw() {
    update_cards("startup_version_notice_keeps_action_link_roles_and_redraw");
}

#[test]
fn update_notice_links_the_changelog_on_link_capable_terminals() {
    isolated_package(
        "update_notice_links_the_changelog_on_link_capable_terminals",
        async |directory, _| {
            let world = update_world(directory, "dark".into());
            let terminal = &world.mode.tool_environment.terminal_image;
            terminal.set_capabilities(TerminalCapabilities {
                hyperlinks: true,
                ..TerminalCapabilities::default()
            });
            world.mode.show_new_version_notification("9.8.7").unwrap();
            let theme = world.themes.theme().get().unwrap();
            let url = "https://github.com/Orchestration-Maestro/maestro/blob/main/CHANGELOG.md";
            let target = format!("\x1b]8;;{url}\x1b\\");
            let row = format!(
                "{}{target}{}\x1b]8;;\x1b\\",
                theme.fg(&ThemeColor::Muted, "Changelog: ").unwrap(),
                theme.fg(&ThemeColor::Accent, "open changelog").unwrap(),
            );
            let rows = world.mode.chat_container.render(80);
            assert_eq!(rows.iter().filter(|line| line.contains(&row)).count(), 1);
            assert!(
                rows.iter()
                    .all(|line| !line.replace(&target, "").contains(url))
            );
            // The row keeps the link it was built with after the terminal loses link support.
            terminal.set_capabilities(TerminalCapabilities::default());
            assert_eq!(world.mode.chat_container.render(80), rows);
        },
    );
}

#[test]
fn startup_package_notice_keeps_order_duplicates_and_redraw() {
    update_cards("startup_package_notice_keeps_order_duplicates_and_redraw");
}

/// Drives both public card operations with the recorded complete inputs.
fn update_cards(test: &str) {
    isolated_package(test, async |directory, _| {
        for (input, expected) in cases::<UpdateInput, UpdateExpected>(FIXTURES, test) {
            match (input, expected) {
                (
                    UpdateInput::Version {
                        value,
                        palette,
                        width,
                    },
                    UpdateExpected::Direct(expected),
                ) => {
                    let world = update_world(directory, palette);
                    world.mode.show_new_version_notification(&value).unwrap();
                    assert_card(&world, width, &expected);
                }
                (
                    UpdateInput::Packages {
                        value,
                        palette,
                        width,
                    },
                    UpdateExpected::Direct(expected),
                ) => {
                    let world = update_world(directory, palette);
                    world.mode.show_package_update_notification(&value).unwrap();
                    assert_card(&world, width, &expected);
                }
                (
                    UpdateInput::Retained {
                        method,
                        before_palette,
                        after_palette,
                        width,
                    },
                    UpdateExpected::Retained {
                        before,
                        after,
                        renders,
                    },
                ) => {
                    retained_update(
                        directory,
                        (&method, &before_palette, &after_palette),
                        width,
                        (&before, &after, renders),
                    );
                }
                _ => panic!("an update operation disagrees with its observation"),
            }
        }
    });
}

/// Builds the same mode setup used by startup, selecting the recorded theme, on a terminal
/// without link support, whichever terminal runs the tests.
fn update_world(directory: &std::path::Path, palette: String) -> World {
    let world = Setup {
        configure: Box::new(move |settings| {
            settings.set_theme(palette);
        }),
        ..Setup::default()
    }
    .build(directory);
    world
        .mode
        .tool_environment
        .terminal_image
        .set_capabilities(TerminalCapabilities::default());
    world
}

/// Checks all transcript rows and the frame request at the consuming boundary.
fn assert_card(world: &World, width: usize, expected: &CardExpected) {
    let theme = world.themes.theme().get().unwrap();
    assert_eq!(
        world.mode.chat_container.render(width),
        painted_update(&theme, &expected.lines)
    );
    assert_eq!(world.runtime.pending(), expected.renders);
}

/// Renders one prepared card under both palettes without rebuilding its text.
fn retained_update(
    directory: &std::path::Path,
    input: (&str, &str, &str),
    width: usize,
    expected: (&[String], &[String], usize),
) {
    let world = update_world(directory, input.1.to_owned());
    if input.0 == "showNewVersionNotification" {
        world.mode.show_new_version_notification("9.8.7").unwrap();
    } else {
        assert_eq!(input.0, "showPackageUpdateNotification");
        world
            .mode
            .show_package_update_notification(&["zeta".into(), "alpha".into(), "zeta".into()])
            .unwrap();
    }
    let before = world.themes.theme().get().unwrap();
    let first = expected
        .0
        .iter()
        .map(|row| row.replace("@before.", "@"))
        .collect::<Vec<_>>();
    assert_eq!(
        world.mode.chat_container.render(width),
        painted(&before, &first)
    );
    let next = if input.2 == "light" {
        super::light()
    } else {
        super::dark()
    };
    world.themes.set_theme_instance(next).unwrap();
    world.mode.chat_container.invalidate();
    let after = world.themes.theme().get().unwrap();
    let lines = expected
        .1
        .iter()
        .map(|row| {
            row.replace(
                "@after.warning@",
                after
                    .get_fg_ansi(&maestro_theme::ThemeColor::Warning)
                    .unwrap(),
            )
            .replace("@before.", "@")
        })
        .collect::<Vec<_>>();
    assert_eq!(
        world.mode.chat_container.render(width),
        painted(&before, &lines)
    );
    assert_eq!(world.runtime.pending(), expected.2);
}

/// Projects semantic foreground colours inside combined SGR re-entry codes.
fn painted_update(theme: &maestro_theme::Theme, rows: &[String]) -> Vec<String> {
    let mut rows = rows.to_vec();
    for (foreground, role) in [
        ("38;2;255;255;0", maestro_theme::ThemeColor::Warning),
        ("38;2;154;115;38", maestro_theme::ThemeColor::Warning),
        ("38;2;128;128;128", maestro_theme::ThemeColor::Muted),
        ("38;2;108;108;108", maestro_theme::ThemeColor::Muted),
    ] {
        let escape = theme.get_fg_ansi(&role).unwrap();
        let parameters = escape
            .strip_prefix("\x1b[")
            .unwrap()
            .strip_suffix('m')
            .unwrap();
        for row in &mut rows {
            *row = row.replace(foreground, parameters);
        }
    }
    painted(theme, &rows)
}

#[test]
fn startup_package_check_skips_before_observations_when_offline() {
    const TEST: &str = "startup_package_check_skips_before_observations_when_offline";
    for (index, value) in [
        None,
        Some(""),
        Some("0"),
        Some("false"),
        Some("1"),
        Some("TRUE"),
        Some("yes"),
        Some(" "),
    ]
    .into_iter()
    .enumerate()
    {
        super::isolated_case(TEST, &index.to_string(), &[], async |directory| {
            let world = native_package_world(directory);
            if let Some(value) = value {
                world.runtime.set_environment("MAESTRO_OFFLINE", value);
            }
            async {
                let set = value.is_some_and(|value| !value.is_empty());
                assert_eq!(maestro_app::configuration::flag_is_set(value), set);
                let (updates, errors) =
                    super::driving(&world, world.mode.check_for_package_updates()).await;
                assert!(errors.is_empty());
                assert_eq!(
                    updates,
                    if set {
                        Vec::<String>::new()
                    } else {
                        vec!["before".to_owned()]
                    }
                );
                assert_eq!(directory.join("npm-observed").exists(), !set);
            }
            .await;
        });
    }
}

/// A native configured-source manager with distinct services and current session directories.
fn native_package_world(root: &std::path::Path) -> World {
    native_package_world_with(root, None, crate::InteractiveModeOptions::default(), None)
}

/// Builds a native manager witness with an optional real input handler.
fn native_package_world_with(
    root: &std::path::Path,
    extension: Option<maestro_app::ExtensionFactory<maestro_app::Local>>,
    options: crate::InteractiveModeOptions,
    offline: Option<bool>,
) -> World {
    for name in [
        "service",
        "session/.maestro/npm/node_modules/before",
        "session/.maestro/npm/node_modules/after",
    ] {
        std::fs::create_dir_all(root.join(name)).unwrap();
    }
    for name in ["before", "after"] {
        std::fs::write(
            root.join(format!(
                "session/.maestro/npm/node_modules/{name}/package.json"
            )),
            "{\n  \"version\": \"1\"\n}\n",
        )
        .unwrap();
    }
    let observed = root.join("npm-observed");
    let script = format!(
        "printf '%s\\n' \"$PWD|$0|$1|$2|$3\" >> '{}'; printf '\"2\"'",
        observed.display()
    );
    let world = Setup {
        options,
        extension,
        offline,
        cwd: Some(root.join("service").to_str().unwrap().to_owned()),
        configure: Box::new(move |settings| {
            settings.set_project_packages(vec![maestro_app::PackageSource::Source(
                "npm:before".into(),
            )]);
            settings.set_npm_command(Some(
                ["/bin/sh", "-c", &script]
                    .into_iter()
                    .map(|s| maestro_app::SettingsListEntry::String(s.into()))
                    .collect(),
            ));
        }),
        ..Setup::default()
    }
    .build(root);
    *world.host.session().session_manager().borrow_mut() =
        maestro_app::SessionManager::in_memory(Some(root.join("session").to_str().unwrap()))
            .unwrap();
    world
}

/// A finite native tmux query result.
#[derive(Deserialize, serde::Serialize)]
#[serde(deny_unknown_fields)]
struct TmuxResult {
    /// Captured standard output.
    text: String,
    /// Native exit code, absent for a signal.
    code: Option<i32>,
    /// Whether the child terminates itself by signal.
    signal: Option<bool>,
}

/// Both native option observations and tmux admission.
#[derive(Deserialize, serde::Serialize)]
#[serde(deny_unknown_fields)]
struct TmuxInput {
    /// Recorded operation.
    op: String,
    /// Ambient tmux value.
    tmux: Option<String>,
    /// Extended-keys result.
    ext: TmuxResult,
    /// Extended-keys-format result.
    format: TmuxResult,
}

/// The warning selected after both queries settle.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct WarningExpected {
    /// Complete warning or silence.
    warning: Option<String>,
}

/// One finite interpreter process for each native query, with no subprocesses.
const TMUX_SCRIPT: &str = r"#!/usr/bin/python3
import json, os, signal, sys
assert sys.argv[1:3] == ['show', '-gv'] and len(sys.argv) == 4
assert sys.argv[3] in ['extended-keys', 'extended-keys-format']
case = json.loads(os.environ['MAESTRO_TMUX_CASE'])
row = case['ext' if sys.argv[3] == 'extended-keys' else 'format']
sys.stdout.write(row['text'])
sys.stdout.flush()
if row.get('signal'):
    os.kill(os.getpid(), signal.SIGTERM)
sys.exit(row.get('code') or 0)
";

/// Installs an executable through the existing checked child owner.
fn install_command(root: &std::path::Path, name: &str, script: &str) {
    let source = root.join(format!("{name}.py"));
    std::fs::write(&source, script).unwrap();
    super::native::install_executable(&source, &root.join("bin").join(name));
    assert_eq!(
        std::fs::read_to_string(root.join("bin").join(name)).unwrap(),
        script
    );
}

#[test]
fn tmux_warning_selects_first_reachable_policy() {
    const TEST: &str = "tmux_warning_selects_first_reachable_policy";
    for (index, (input, expected)) in cases::<TmuxInput, WarningExpected>(FIXTURES, TEST)
        .into_iter()
        .enumerate()
    {
        let encoded = serde_json::to_string(&input).unwrap();
        super::isolated_case(
            TEST,
            &index.to_string(),
            &[("MAESTRO_TMUX_CASE", &encoded)],
            async |directory| {
                install_command(directory, "tmux", TMUX_SCRIPT);
                let world = Setup::default().build(directory);
                assert_eq!(input.op, "tmux");
                if let Some(value) = &input.tmux {
                    world.runtime.set_environment("TMUX", value);
                }
                async {
                    let (warning, errors) =
                        super::driving(&world, world.mode.check_tmux_keyboard_setup()).await;
                    assert!(errors.is_empty());
                    assert_eq!(warning, expected.warning);
                }
                .await;
            },
        );
    }
}

#[test]
fn startup_checks_start_in_order_without_blocking_prompts() {
    const TEST: &str = "startup_checks_start_in_order_without_blocking_prompts";
    super::isolated_case(TEST, "child", &[], async |directory| {
        let (fetch, requests) = reporting_transport();
        let world = Setup {
            options: crate::InteractiveModeOptions {
                service_origin: Some("https://service.invalid".into()),
                fetch: Some(fetch),
                ..Default::default()
            },
            configure: Box::new(|settings| settings.set_enable_install_telemetry(false)),
            ..Setup::default()
        }
        .build(directory);
        let mut run = Box::pin(world.mode.run());
        assert!(poll(&mut run).is_none());
        super::prompts::until_waiting(&world.mode, run.as_mut()).await;
        assert_eq!(
            *requests.lock().unwrap(),
            ["https://service.invalid/api/latest-version"]
        );
        assert!(world.mode.on_input_callback.borrow().is_some());
        for work in world.runtime.take_futures() {
            work.await.unwrap();
        }
        drop(run);
    });
    super::isolated_case(TEST, "ready", &[], async |root| {
        (ready_completion_order(root)).await;
    });
    for (index, order) in [
        [0, 1, 2],
        [0, 2, 1],
        [1, 0, 2],
        [1, 2, 0],
        [2, 0, 1],
        [2, 1, 0],
    ]
    .into_iter()
    .enumerate()
    {
        super::isolated_case(
            TEST,
            &index.to_string(),
            &[("MAESTRO_NOTICE_INHERITED", "retained")],
            async |root| {
                (completion_order(root, order)).await;
            },
        );
    }
}

#[test]
fn startup_package_check_uses_current_directories_settings_and_recovers() {
    const TEST: &str = "startup_package_check_uses_current_directories_settings_and_recovers";
    super::isolated_case(TEST, "child", &[], async |directory| {
        let world = native_package_world(directory);
        let (first, errors) = super::driving(&world, world.mode.check_for_package_updates()).await;
        assert!(errors.is_empty());
        assert_eq!(first, ["before"]);
        let session = world.host.session();
        session
            .settings_manager()
            .borrow_mut()
            .set_project_packages(vec![maestro_app::PackageSource::Source("npm:after".into())]);
        let (second, errors) = super::driving(&world, world.mode.check_for_package_updates()).await;
        assert!(errors.is_empty());
        assert_eq!(second, ["after"]);
        let observed = std::fs::read_to_string(directory.join("npm-observed")).unwrap();
        let cwd = directory.join("session").canonicalize().unwrap();
        assert_eq!(
            observed,
            format!(
                "{}|view|before|version|--json\n{}|view|after|version|--json\n",
                cwd.display(),
                cwd.display()
            )
        );
        assert_current_agent_root(&world, directory).await;
        let observed = std::fs::read_to_string(directory.join("npm-observed")).unwrap();
        session
            .settings_manager()
            .borrow_mut()
            .set_npm_command(Some(vec![maestro_app::SettingsListEntry::String(
                "/missing/maestro-command".into(),
            )]));
        let (failed, errors) = super::driving(&world, world.mode.check_for_package_updates()).await;
        assert!(errors.is_empty() && failed.is_empty());
        assert_eq!(
            std::fs::read_to_string(directory.join("npm-observed")).unwrap(),
            observed
        );
    });
}

/// Settings handed to the real input hook after the mode has been constructed.
type InputSettings = std::rc::Rc<
    std::cell::RefCell<Option<std::rc::Rc<std::cell::RefCell<maestro_app::SettingsManager>>>>,
>;

/// An actual input hook that replaces the manager's configured source.
fn changing_packages(slot: &InputSettings) -> maestro_app::ExtensionFactory<maestro_app::Local> {
    let slot = slot.clone();
    std::rc::Rc::new(move |api| {
        let slot = slot.clone();
        let handler: maestro_app::ExtensionHandler<
            maestro_app::InputEvent,
            maestro_app::InputEventResult,
            maestro_app::Local,
        > = std::rc::Rc::new(move |_, _| {
            let settings = slot.borrow().as_ref().unwrap().clone();
            settings
                .borrow_mut()
                .set_project_packages(vec![maestro_app::PackageSource::Source("npm:after".into())]);
            Box::pin(async { Ok(Some(maestro_app::InputEventResult::Handled)) })
        });
        api.on::<maestro_app::InputEvent, maestro_app::InputEventResult>("input".into(), handler)
            .unwrap();
        Box::pin(async { Ok(()) })
    })
}

#[test]
fn startup_checks_snapshot_packages_before_input_hooks() {
    const TEST: &str = "startup_checks_snapshot_packages_before_input_hooks";
    super::isolated_case(TEST, "child", &[], async |directory| {
        let slot = InputSettings::default();
        let world = native_package_world_with(
            directory,
            Some(changing_packages(&slot)),
            crate::InteractiveModeOptions {
                initial_message: Some("replace packages".into()),
                ..Default::default()
            },
            None,
        );
        *slot.borrow_mut() = Some(world.host.session().settings_manager().clone());
        let mut run = Box::pin(world.mode.run());
        assert!(poll(&mut run).is_none());
        super::prompts::until_waiting(&world.mode, run.as_mut()).await;
        assert_eq!(
            world
                .host
                .session()
                .settings_manager()
                .borrow()
                .get_project_settings()
                .0["packages"],
            serde_json::json!(["npm:after"])
        );
        let ((), errors) = super::driving(&world, until_notice(&world, "- before")).await;
        assert!(errors.is_empty());
        drop(run);
        let lazy = world.mode.check_for_package_updates();
        world
            .host
            .session()
            .prompt("same input hook", maestro_app::PromptOptions::default())
            .await
            .unwrap();
        let (result, errors) = super::driving(&world, lazy).await;
        assert!(errors.is_empty());
        assert_eq!(result, ["after"]);
    });
}

/// Waits for a producer's visible transcript result while the shared task driver runs.
async fn until_notice(world: &World, text: &str) {
    while !world
        .mode
        .chat_container
        .render(120)
        .iter()
        .any(|row| row.contains(text))
    {
        tokio::task::yield_now().await;
    }
}

/// Native children acknowledge their exact operands before waiting for a permit.
const GATED_TMUX: &str = r"#!/usr/bin/python3
import json, os, signal, sys
assert sys.argv[1:3] == ['show', '-gv'] and len(sys.argv) == 4
name = sys.argv[3]
assert name in ['extended-keys', 'extended-keys-format']
root = os.environ['MAESTRO_CODING_AGENT_DIR']
signal.signal(signal.SIGTERM, signal.SIG_IGN)
with open(os.path.join(root, name + '.ready'), 'w') as ready:
    json.dump({'argv':sys.argv[1:], 'cwd':os.getcwd(), 'inherited':os.environ.get('MAESTRO_NOTICE_INHERITED')}, ready)
with open(os.path.join(root, name + '.permit')) as permit:
    assert permit.readline() == 'go\n'
sys.stdout.write('off\n' if name == 'extended-keys' else 'xterm\n')
sys.stdout.flush()
";

/// One joined peer for native readiness and independently released queries.
struct NativeGates {
    /// The finite query names whose descriptors this peer owns.
    names: &'static [&'static str],
    /// Every query's readiness transcript.
    ready: Option<tokio::sync::oneshot::Receiver<Vec<serde_json::Value>>>,
    /// Query permits, followed by the final cleanup message.
    release: std::sync::mpsc::Sender<Option<String>>,
    /// The peer that owns its FIFO descriptors through capture completion.
    worker: std::thread::JoinHandle<()>,
}

impl NativeGates {
    /// Creates both query pipes before any native child starts.
    fn new(root: &std::path::Path) -> Self {
        Self::with_names(root, &["extended-keys", "extended-keys-format"])
    }

    /// Creates the finite named producer gates before any child starts.
    fn with_names(root: &std::path::Path, names: &'static [&'static str]) -> Self {
        let paths: Vec<_> = names
            .iter()
            .flat_map(|name| {
                [
                    root.join(format!("{name}.ready")),
                    root.join(format!("{name}.permit")),
                ]
            })
            .collect();
        assert!(
            std::process::Command::new("mkfifo")
                .args(paths)
                .status()
                .unwrap()
                .success()
        );
        let root = root.to_owned();
        let (send, ready) = tokio::sync::oneshot::channel();
        let (release, receive) = std::sync::mpsc::channel();
        let worker = std::thread::spawn(move || {
            use std::io::Write;
            let observed = names
                .iter()
                .map(|name| {
                    let file = std::fs::File::open(root.join(format!("{name}.ready"))).unwrap();
                    serde_json::from_reader(file).unwrap()
                })
                .collect();
            send.send(observed).unwrap();
            let mut descriptors = Vec::new();
            while let Some(name) = receive.recv().unwrap() {
                let mut file = std::fs::OpenOptions::new()
                    .read(true)
                    .write(true)
                    .open(root.join(format!("{name}.permit")))
                    .unwrap();
                file.write_all(b"go\n").unwrap();
                descriptors.push(file);
            }
        });
        Self {
            names,
            ready: Some(ready),
            release,
            worker,
        }
    }

    /// Requires both finite children to reach their blocking point first.
    async fn ready(&mut self, root: &std::path::Path) {
        let observed = self.ready.take().unwrap().await.unwrap();
        for (name, row) in self.names.iter().zip(observed) {
            let (argv, cwd) = if *name == "init" {
                (serde_json::json!(["--version"]), root.to_owned())
            } else if *name == "package" {
                (
                    serde_json::json!(["view", "before", "version", "--json"]),
                    root.join("session"),
                )
            } else {
                (serde_json::json!(["show", "-gv", name]), root.to_owned())
            };
            assert_eq!(row["argv"], argv);
            assert_eq!(row["cwd"], cwd.canonicalize().unwrap().to_str().unwrap());
            assert_eq!(row["inherited"], "retained");
        }
    }

    /// Lets both producers finish, without waiting for a source deadline.
    fn release(&self) {
        for name in ["extended-keys", "extended-keys-format"] {
            self.release.send(Some(name.into())).unwrap();
        }
    }

    /// Closes the peer only after its native captures have been joined.
    fn finish(self) {
        self.release.send(None).unwrap();
        self.worker.join().unwrap();
    }
}

/// Runs every admitted native capture as a joined test-owned task.
fn capture_tasks(
    world: &World,
) -> Vec<tokio::task::JoinHandle<Result<(), Box<dyn std::error::Error>>>> {
    world
        .runtime
        .take_futures()
        .into_iter()
        .map(tokio::task::spawn_local)
        .collect()
}

/// Joins all actual native capture work before dropping fixture descriptors.
async fn finish_captures(
    tasks: Vec<tokio::task::JoinHandle<Result<(), Box<dyn std::error::Error>>>>,
) {
    for task in tasks {
        task.await.unwrap().unwrap();
    }
}

#[test]
fn tmux_probe_uses_both_options_concurrently() {
    const TEST: &str = "tmux_probe_uses_both_options_concurrently";
    super::isolated_case(
        TEST,
        "child",
        &[("MAESTRO_NOTICE_INHERITED", "retained")],
        async |root| {
            install_command(root, "tmux", GATED_TMUX);
            let mut gates = NativeGates::new(root);
            let world = Setup::default().build(root);
            world.runtime.set_environment("TMUX", "session");
            let mut query = Box::pin(world.mode.check_tmux_keyboard_setup());
            assert!(poll(&mut query).is_none());
            let tasks = capture_tasks(&world);
            gates.ready(root).await;
            assert!(poll(&mut query).is_none());
            gates.release();
            assert!(
                query
                    .await
                    .unwrap()
                    .starts_with("tmux extended-keys is off.")
            );
            finish_captures(tasks).await;
            gates.finish();
        },
    );
}

#[test]
fn tmux_query_failures_and_timeout_are_silent() {
    const TEST: &str = "tmux_query_failures_and_timeout_are_silent";
    if let Some(output) = super::isolated_output(
        TEST,
        "missing",
        &[],
        "trap '' XFSZ; PATH=/nonexistent/maestro-programs",
        async |root| {
            let world = Setup::default().build(root);
            assert_eq!(world.mode.run_tmux_show("extended-keys").await, None);
        },
    ) {
        assert!(output.status.success());
    }
    super::isolated_case(
        TEST,
        "deadline",
        &[("MAESTRO_NOTICE_INHERITED", "retained")],
        async |root| {
            let world = Setup::default().build(root);
            install_command(root, "tmux", GATED_TMUX);
            let mut gates = NativeGates::new(root);
            let mut keys = world.mode.run_tmux_show("extended-keys");
            let mut format = world.mode.run_tmux_show("extended-keys-format");
            let tasks = capture_tasks(&world);
            gates.ready(root).await;
            let weak = std::rc::Rc::downgrade(&world.mode);
            drop(world.mode);
            assert!(weak.upgrade().is_none());
            tokio::time::pause();
            assert!(
                world
                    .runtime
                    .advance_to(std::time::Duration::from_millis(1_999))
                    .into_iter()
                    .all(|result| result.is_ok())
            );
            assert!(poll(&mut keys).is_none() && poll(&mut format).is_none());
            tokio::time::advance(std::time::Duration::from_millis(2_000)).await;
            assert!(tasks.iter().all(|task| !task.is_finished()));
            assert!(
                world
                    .runtime
                    .advance_to(std::time::Duration::from_millis(2_000))
                    .into_iter()
                    .all(|result| result.is_ok())
            );
            assert_eq!(keys.await, None);
            assert_eq!(format.await, None);
            gates.release();
            finish_captures(tasks).await;
            gates.finish();
        },
    );
}

#[test]
fn tmux_capture_uses_raw_status_and_two_second_settlement() {
    const TEST: &str = "tmux_capture_uses_raw_status_and_two_second_settlement";
    let case = serde_json::json!({"ext":{"text":"off\n","signal":true},"format":{"text":"xterm\n","code":0}}).to_string();
    super::isolated_case(
        TEST,
        "signal",
        &[("MAESTRO_TMUX_CASE", &case)],
        async |root| {
            install_command(root, "tmux", TMUX_SCRIPT);
            let world = Setup::default().build(root);
            let query = world.mode.run_tmux_show("extended-keys");
            let tasks = capture_tasks(&world);
            assert_eq!(query.await, None);
            finish_captures(tasks).await;
            let wrong = maestro_app::exec_command(
                "tmux",
                &["show".into(), "-gv".into(), "extended-keys".into()],
                root.to_str().unwrap(),
                maestro_app::ExecOptions::default(),
            )
            .await
            .unwrap();
            assert_eq!((wrong.code, wrong.killed), (0, false));
            assert_eq!(wrong.stdout, "off\n");
        },
    );
}

#[test]
fn notice_background_work_releases_mode_after_completion() {
    const TEST: &str = "notice_background_work_releases_mode_after_completion";
    super::isolated_case(TEST, "child", &[], async |root| {
        let release = std::sync::Arc::new(tokio::sync::Notify::new());
        let fetch = held_release_fetch(release.clone());
        let world = Setup {
            options: crate::InteractiveModeOptions {
                service_origin: Some("https://service.invalid".into()),
                fetch: Some(fetch),
                ..Default::default()
            },
            configure: Box::new(|settings| settings.set_enable_install_telemetry(false)),
            ..Setup::default()
        }
        .build(root);
        let operations = world.mode.native_package_operations();
        let weak = std::rc::Rc::downgrade(&world.mode);
        let mut run = Box::pin(world.mode.run());
        assert!(poll(&mut run).is_none());
        super::prompts::until_waiting(&world.mode, run.as_mut()).await;
        let tasks = capture_tasks(&world);
        drop(run);
        drop(world.mode);
        assert!(weak.upgrade().is_some());
        release.notify_one();
        finish_captures(tasks).await;
        assert!(weak.upgrade().is_none());
        assert_eq!(
            maestro_app::PackageOperations::spawn(&operations, Box::pin(async {}))
                .unwrap_err()
                .to_string(),
            "package runtime is no longer available"
        );
    });
}

#[test]
fn notice_traversals_consume_each_input_once() {
    const TEST: &str = "notice_traversals_consume_each_input_once";
    isolated_package(TEST, async |root, _| {
        for count in [64, 128, 256] {
            let entries: Vec<_> = (0..count)
                .map(|index| maestro_app::ChangelogEntry {
                    major: 1.0,
                    minor: 0.0,
                    patch: 0.0,
                    content: format!("entry-{index}"),
                })
                .collect();
            let visits = std::cell::Cell::new(0);
            let joined = super::super::notices::join_changelog(
                entries.iter().inspect(|_| visits.set(visits.get() + 1)),
            );
            assert_eq!(
                joined.split("\n\n").collect::<Vec<_>>(),
                entries
                    .iter()
                    .map(|entry| entry.content.as_str())
                    .collect::<Vec<_>>()
            );
            assert!(visits.get() <= count);
            assert!(rescanning_visits(&entries) > count);
            let names: Vec<_> = entries.into_iter().map(|entry| entry.content).collect();
            let world = Setup::default().build(root);
            world.mode.show_package_update_notification(&names).unwrap();
            let rows: Vec<_> = world
                .mode
                .chat_container
                .render(120)
                .into_iter()
                .map(|row| maestro_app::strip_ansi(&row).unwrap().trim().to_owned())
                .filter(|row| row.starts_with("- "))
                .collect();
            assert_eq!(
                rows.iter()
                    .map(|row| row.strip_prefix("- ").unwrap())
                    .collect::<Vec<_>>(),
                names
            );
        }
    });
}

#[test]
fn startup_offline_selection_reaches_each_existing_operation() {
    const TEST: &str = "startup_offline_selection_reaches_each_existing_operation";
    let case =
        serde_json::json!({"ext":{"text":"off\n","code":0},"format":{"text":"xterm\n","code":0}})
            .to_string();
    for (index, offline) in [None, Some(false), Some(true)].into_iter().enumerate() {
        for skip in [false, true] {
            super::isolated_case(
                TEST,
                &format!("{index}-{skip}"),
                &[
                    ("MAESTRO_OFFLINE", "1"),
                    ("MAESTRO_SKIP_VERSION_CHECK", if skip { "1" } else { "" }),
                    ("MAESTRO_TMUX_CASE", &case),
                ],
                async |root| {
                    offline_consumers(root, offline, skip).await;
                },
            );
        }
    }
}

/// Observes explicit offline forwarding at the actual request and native command consumers.
async fn offline_consumers(root: &std::path::Path, offline: Option<bool>, skip: bool) {
    install_command(root, "tmux", TMUX_SCRIPT);
    let (fetch, requests) = reporting_transport();
    let world = native_package_world_with(
        root,
        None,
        crate::InteractiveModeOptions {
            service_origin: Some("https://service.invalid".into()),
            fetch: Some(fetch),
            ..Default::default()
        },
        offline,
    );
    world.runtime.set_environment("MAESTRO_OFFLINE", "1");
    world.runtime.set_environment("TMUX", "session");
    let mut run = Box::pin(world.mode.run());
    assert!(poll(&mut run).is_none());
    super::prompts::until_waiting(&world.mode, run.as_mut()).await;
    let online = offline == Some(false);
    let mut expected = Vec::new();
    if online {
        expected.push(format!(
            "https://service.invalid/api/report-install?version={VERSION}"
        ));
    }
    if online && !skip {
        expected.push("https://service.invalid/api/latest-version".into());
    }
    assert_eq!(*requests.lock().unwrap(), expected);
    let ((), errors) = super::driving(&world, async {
        if online {
            until_notice(&world, "- before").await;
        }
        until_notice(&world, "tmux extended-keys is off.").await;
    })
    .await;
    assert!(errors.is_empty());
    assert_eq!(root.join("npm-observed").exists(), online);
    drop(run);
}

/// Holds the real release request at response headers until the caller releases it.
fn held_release_fetch(release: std::sync::Arc<tokio::sync::Notify>) -> maestro_app::Fetch {
    std::sync::Arc::new(move |_| {
        let release = release.clone();
        Box::pin(async move {
            release.notified().await;
            Err(maestro_app::FetchError::Timeout)
        })
    })
}

/// A deliberately rescanning control, counted at the same iterator boundary.
fn rescanning_visits(entries: &[maestro_app::ChangelogEntry]) -> usize {
    let visits = std::cell::Cell::new(0);
    for end in 1..=entries.len() {
        for _ in entries.iter().take(end) {
            visits.set(visits.get() + 1);
        }
    }
    visits.get()
}

/// One finite native package probe at its actual argument and working-directory boundary.
const GATED_PACKAGE: &str = r"#!/usr/bin/python3
import json, os, sys
assert sys.argv[1:] == ['view', 'before', 'version', '--json']
root = os.environ['MAESTRO_CODING_AGENT_DIR']
with open(os.path.join(root, 'package.ready'), 'w') as ready:
    json.dump({'argv':sys.argv[1:], 'cwd':os.getcwd(), 'inherited':os.environ.get('MAESTRO_NOTICE_INHERITED')}, ready)
with open(os.path.join(root, 'package.permit')) as permit:
    assert permit.readline() == 'go\n'
sys.stdout.write(json.dumps('2'))
";

/// A controlled successful release over the existing finite-stream fixture pattern.
fn successful_release(
    release: std::sync::Arc<tokio::sync::Notify>,
) -> (maestro_app::Fetch, Requests) {
    let requests = Requests::default();
    let seen = requests.clone();
    let fetch: maestro_app::Fetch = std::sync::Arc::new(move |request| {
        seen.lock().unwrap().push(request.url);
        let release = release.clone();
        Box::pin(async move {
            release.notified().await;
            Ok(maestro_app::HttpResponse {
                status: 200,
                status_text: "OK".into(),
                headers: std::collections::BTreeMap::default(),
                body: Box::pin(futures_util::stream::iter([Ok(
                    b"{\"version\":\"9.8.7\"}".to_vec()
                )])),
            })
        })
    });
    (fetch, requests)
}

/// Checks each producer's appended transcript slice in its completion order.
async fn completion_order(root: &std::path::Path, order: [usize; 3]) {
    install_command(root, "tmux", GATED_TMUX);
    install_command(root, "npm", GATED_PACKAGE);
    let mut gates =
        NativeGates::with_names(root, &["package", "extended-keys", "extended-keys-format"]);
    let release = std::sync::Arc::new(tokio::sync::Notify::new());
    let (fetch, requests) = successful_release(release.clone());
    let world = completion_world(root, fetch);
    let mut run = Box::pin(world.mode.run());
    assert!(poll(&mut run).is_none());
    super::prompts::until_waiting(&world.mode, run.as_mut()).await;
    assert!(world.mode.on_input_callback.borrow().is_some());
    assert_eq!(
        *requests.lock().unwrap(),
        ["https://service.invalid/api/latest-version"]
    );
    let tasks = capture_tasks(&world);
    gates.ready(root).await;
    let mut previous = world.mode.chat_container.render(120).len();
    for producer in order {
        match producer {
            0 => release.notify_one(),
            1 => gates.release.send(Some("package".into())).unwrap(),
            _ => gates.release(),
        }
        let text = [
            "New version 9.8.7 is available.",
            "- before",
            "tmux extended-keys is off.",
        ][producer];
        until_notice(&world, text).await;
        let rows = world.mode.chat_container.render(120);
        let current = &rows[previous..];
        assert!(current.iter().any(|row| row.contains(text)));
        previous = rows.len();
    }
    finish_captures(tasks).await;
    gates.finish();
    drop(run);
}

/// Builds the actual configured sources and modes used by completion permutations.
fn completion_world(root: &std::path::Path, fetch: maestro_app::Fetch) -> World {
    let world = native_package_world_with(
        root,
        None,
        crate::InteractiveModeOptions {
            service_origin: Some("https://service.invalid".into()),
            fetch: Some(fetch),
            ..Default::default()
        },
        None,
    );
    world
        .host
        .session()
        .settings_manager()
        .borrow_mut()
        .set_enable_install_telemetry(false);
    world
        .host
        .session()
        .settings_manager()
        .borrow_mut()
        .set_npm_command(Some(vec![maestro_app::SettingsListEntry::String(
            "npm".into(),
        )]));
    world.runtime.set_environment("TMUX", "session");
    world
}

/// Native metadata proves the fresh manager uses the ambient agent root, not services' root.
async fn assert_current_agent_root(world: &World, root: &std::path::Path) {
    let script = r"#!/usr/bin/python3
import json, os, sys
root = os.environ['MAESTRO_CODING_AGENT_DIR']
with open(os.path.join(root, 'git-observed'), 'a') as record:
    record.write(json.dumps({'cwd':os.getcwd(), 'argv':sys.argv[1:], 'prompt':os.environ.get('GIT_TERMINAL_PROMPT')}) + '\n')
if sys.argv[1:] == ['rev-parse', 'HEAD']:
    print('a' * 40)
elif sys.argv[1:] == ['rev-parse', '--abbrev-ref', '@{upstream}']:
    print('origin/main')
else:
    assert sys.argv[1:] == ['ls-remote', 'origin', 'refs/heads/main']
    print('b' * 40 + '\trefs/heads/main')
";
    install_command(root, "git", script);
    let checkout = root.join("git/example.test/team/repo");
    std::fs::create_dir_all(&checkout).unwrap();
    let session = world.host.session();
    session
        .settings_manager()
        .borrow_mut()
        .set_packages(vec![maestro_app::PackageSource::Source(
            "https://example.test/team/repo".into(),
        )]);
    assert_ne!(world.host.services().agent_dir, root.to_str().unwrap());
    let (result, errors) = super::driving(world, world.mode.check_for_package_updates()).await;
    assert!(errors.is_empty());
    assert_eq!(result, ["after", "example.test/team/repo"]);
    let rows: Vec<serde_json::Value> = std::fs::read_to_string(root.join("git-observed"))
        .unwrap()
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect();
    assert_eq!(rows.len(), 3);
    assert!(
        rows.iter()
            .all(|row| row["cwd"] == checkout.canonicalize().unwrap().to_str().unwrap())
    );
    assert_eq!(
        rows[2]["argv"],
        serde_json::json!(["ls-remote", "origin", "refs/heads/main"])
    );
    assert_eq!(rows[2]["prompt"], "0");
    session
        .settings_manager()
        .borrow_mut()
        .set_packages(Vec::new());
}

/// An already-ready release still follows the synchronous startup warning.
async fn ready_completion_order(root: &std::path::Path) {
    ready_prefix_root(root).await;
    let release = std::sync::Arc::new(tokio::sync::Notify::new());
    release.notify_one();
    let (fetch, _) = successful_release(release);
    let world = Setup {
        options: crate::InteractiveModeOptions {
            service_origin: Some("https://service.invalid".into()),
            fetch: Some(fetch),
            model_fallback_message: Some("SYNC_WARNING".into()),
            ..Default::default()
        },
        configure: Box::new(|settings| settings.set_enable_install_telemetry(false)),
        ..Setup::default()
    }
    .build(root);
    let mut run = Box::pin(world.mode.run());
    assert!(poll(&mut run).is_none());
    super::prompts::until_waiting(&world.mode, run.as_mut()).await;
    for work in world.runtime.take_futures() {
        work.await.unwrap();
    }
    let rows = world.mode.chat_container.render(120);
    let warning = rows
        .iter()
        .position(|row| row.contains("SYNC_WARNING"))
        .unwrap();
    let release = rows
        .iter()
        .position(|row| row.contains("New version 9.8.7 is available."))
        .unwrap();
    assert!(
        warning < release,
        "synchronous warning must precede ready release notification"
    );
    drop(run);
}

/// Distinguishes eager producer admission from deferred notification at their shared owner.
async fn ready_prefix_root(root: &std::path::Path) {
    let release = std::sync::Arc::new(tokio::sync::Notify::new());
    release.notify_one();
    let (fetch, requests) = successful_release(release);
    let world = Setup {
        options: crate::InteractiveModeOptions {
            service_origin: Some("https://service.invalid".into()),
            fetch: Some(fetch),
            ..Default::default()
        },
        ..Setup::default()
    }
    .build(root);
    world.mode.start_startup_checks();
    assert_eq!(
        *requests.lock().unwrap(),
        ["https://service.invalid/api/latest-version"]
    );
    assert!(
        world.mode.chat_container.render(120).is_empty(),
        "eager producer must defer its notification"
    );
    for work in world.runtime.take_futures() {
        work.await.unwrap();
    }
    assert!(
        world
            .mode
            .chat_container
            .render(120)
            .iter()
            .any(|row| row.contains("New version 9.8.7 is available."))
    );
}

/// A single search probe paused on the shared native producer gate.
const INIT_SCRIPT: &str = r"#!/usr/bin/python3
import json, os, sys
assert sys.argv[1:] == ['--version']
root = os.environ['MAESTRO_CODING_AGENT_DIR']
with open(os.path.join(root, 'init.ready'), 'w') as ready:
    json.dump({'argv': sys.argv[1:], 'cwd': os.getcwd(), 'inherited': os.environ.get('MAESTRO_NOTICE_INHERITED')}, ready)
with open(os.path.join(root, 'init.permit')) as permit:
    assert permit.readline() == 'go\n'
";

/// Runs one initialization witness with search discovery outside its managed directory.
fn initialization_case(test: &str, body: impl AsyncFnOnce(&std::path::Path)) {
    let path = format!("probes/bin:{}", std::env::var("PATH").unwrap());
    super::isolated_case(
        test,
        "child",
        &[("PATH", &path), ("MAESTRO_NOTICE_INHERITED", "retained")],
        async |root| {
            std::fs::create_dir_all(root.join("probes/bin")).unwrap();
            install_command(&root.join("probes"), "fd", INIT_SCRIPT);
            super::scripted(&root.join("bin"), "rg", "exit 0");
            body(root).await;
        },
    );
}

/// Delivers a distinguishable user row through the same synchronous session-event path.
fn initialization_event(world: &World, text: &str) {
    super::events::dispatch(
        world,
        maestro_app::AgentEvent::MessageStart {
            message: super::transcript::message(serde_json::json!({
                "role": "user", "content": text, "timestamp": 0
            })),
        },
    );
}

/// Holds initialization pending while one or two events arrive, then observes their rows.
async fn early_events(root: &std::path::Path, texts: &[&str]) {
    let mut gates = NativeGates::with_names(root, &["init"]);
    let world = Setup::default().build(root);
    initialization_event(&world, texts[0]);
    let mut work = world.runtime.take_futures();
    assert_eq!(work.len(), 1, "one initialization continuation");
    let work = tokio::task::spawn_local(work.pop().unwrap());
    gates.ready(root).await;
    assert!(!world.mode.is_initialized.get());
    assert!(world.mode.chat_container.children().borrow().is_empty());
    for text in &texts[1..] {
        initialization_event(&world, text);
    }
    assert_eq!(world.runtime.futures(), 0, "no second initialization task");
    assert!(world.mode.chat_container.children().borrow().is_empty());
    gates.release.send(Some("init".into())).unwrap();
    work.await.unwrap().unwrap();
    assert!(world.mode.is_initialized.get());
    let rows = world.mode.chat_container.render(100).join("\n");
    let positions: Vec<_> = texts.iter().map(|text| rows.find(text).unwrap()).collect();
    assert!(positions.windows(2).all(|pair| pair[0] < pair[1]), "{rows}");
    gates.finish();
}

#[test]
fn early_event_waits_for_initialization_to_complete() {
    initialization_case(
        "early_event_waits_for_initialization_to_complete",
        async |root| {
            early_events(root, &["EARLY_ROW"]).await;
        },
    );
}

#[test]
fn early_events_keep_their_arrival_order() {
    initialization_case("early_events_keep_their_arrival_order", async |root| {
        early_events(root, &["FIRST_ROW", "SECOND_ROW"]).await;
    });
}

#[test]
fn initialized_event_is_handled_synchronously() {
    super::isolated("initialized_event_is_handled_synchronously", async |root| {
        let world = super::started(root).await;
        initialization_event(&world, "IMMEDIATE_ROW");
        assert!(
            world
                .mode
                .chat_container
                .render(100)
                .join("\n")
                .contains("IMMEDIATE_ROW")
        );
        assert_eq!(world.runtime.futures(), 0);
    });
}

#[test]
fn run_awaits_initialization_before_startup_checks() {
    initialization_case(
        "run_awaits_initialization_before_startup_checks",
        async |root| {
            let mut gates = NativeGates::with_names(root, &["init"]);
            let calls = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
            let seen = calls.clone();
            let world = Setup {
                offline: Some(false),
                options: crate::InteractiveModeOptions {
                    service_origin: Some("https://service.invalid".into()),
                    fetch: Some(std::sync::Arc::new(move |request| {
                        if request.url.ends_with("/api/latest-version") {
                            seen.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                        }
                        Box::pin(async { Err(maestro_app::FetchError::Timeout) })
                    })),
                    ..Default::default()
                },
                ..Setup::default()
            }
            .build(root);
            let prefix_work = world.runtime.futures();
            let mut run = std::pin::pin!(world.mode.run());
            tokio::select! {
                () = gates.ready(root) => {},
                result = run.as_mut() => panic!("run ended before initialization: {result:?}"),
            }
            assert!(!world.mode.is_initialized.get());
            assert_eq!(calls.load(std::sync::atomic::Ordering::SeqCst), 0);
            assert_eq!(
                world.runtime.futures(),
                prefix_work,
                "startup checks have not started"
            );
            gates.release.send(Some("init".into())).unwrap();
            super::prompts::until_waiting(&world.mode, run.as_mut()).await;
            assert!(world.mode.is_initialized.get());
            assert!(calls.load(std::sync::atomic::Ordering::SeqCst) > 0);
            for work in world.runtime.take_futures() {
                work.await.unwrap();
            }
            gates.finish();
        },
    );
}

#[test]
fn queued_event_failure_does_not_drop_later_events() {
    super::isolated(
        "queued_event_failure_does_not_drop_later_events",
        async |root| {
            let world = Setup {
                configure: Box::new(|settings| settings.set_show_terminal_progress(true)),
                ..Setup::default()
            }
            .build(root);
            world
                .terminal
                .set_progress_error(Some(std::io::ErrorKind::PermissionDenied));
            super::events::dispatch(&world, maestro_app::AgentEvent::AgentStart);
            initialization_event(&world, "AFTER_ERROR");
            let mut work = world.runtime.take_futures();
            assert_eq!(work.len(), 1);
            let result = work.pop().unwrap().await;
            let rows = world.mode.chat_container.render(100).join("\n");
            assert_eq!((result.is_ok(), rows.contains("AFTER_ERROR")), (true, true));
            let mut reports = world.runtime.take_futures();
            assert_eq!(reports.len(), 1);
            let error = reports.pop().unwrap().await.unwrap_err();
            assert_eq!(
                error.downcast_ref::<std::io::Error>().unwrap().kind(),
                std::io::ErrorKind::PermissionDenied
            );
        },
    );
}

#[test]
fn later_event_retries_initialization_after_failure() {
    super::isolated(
        "later_event_retries_initialization_after_failure",
        async |root| {
            let world = Setup::default().build(root);
            world
                .terminal
                .set_hide_error(Some(std::io::ErrorKind::BrokenPipe));
            initialization_event(&world, "FAILED_BATCH");
            let mut work = world.runtime.take_futures();
            assert_eq!(work.len(), 1);
            let error = work.pop().unwrap().await.unwrap_err();
            assert_eq!(
                error.downcast_ref::<std::io::Error>().unwrap().kind(),
                std::io::ErrorKind::BrokenPipe
            );
            assert!(!world.mode.is_initialized.get());
            world.terminal.set_hide_error(None);
            initialization_event(&world, "RETRIED_BATCH");
            let mut work = world.runtime.take_futures();
            assert_eq!(work.len(), 1, "a later event starts a fresh continuation");
            work.pop().unwrap().await.unwrap();
            assert!(world.mode.is_initialized.get());
            let rows = world.mode.chat_container.render(100).join("\n");
            assert!(!rows.contains("FAILED_BATCH"));
            assert!(rows.contains("RETRIED_BATCH"));
        },
    );
}

#[test]
fn initialization_registers_signals_before_search_suspends() {
    initialization_case(
        "initialization_registers_signals_before_search_suspends",
        async |root| {
            let mut gates = NativeGates::with_names(root, &["init"]);
            let effects = std::rc::Rc::new(super::lifecycle::Effects::default());
            let world = Setup {
                lifecycle: effects.clone(),
                ..Setup::default()
            }
            .build(root);
            initialization_event(&world, "AFTER_SEARCH");
            let mut work = world.runtime.take_futures();
            assert_eq!(work.len(), 1);
            let work = tokio::task::spawn_local(work.pop().unwrap());
            gates.ready(root).await;
            let prefix = effects.events.borrow().clone();
            gates.release.send(Some("init".into())).unwrap();
            work.await.unwrap().unwrap();
            gates.finish();
            assert_eq!(
                prefix,
                ["register"],
                "signals are active while search is pending"
            );
        },
    );
}
