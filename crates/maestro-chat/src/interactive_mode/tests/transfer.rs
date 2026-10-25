//! The export command's path argument and the installed submission that runs it.
use super::super::transfer::get_path_command_argument;
use super::typed::{poll, recalled, submit, waiting};
use super::{Setup, World, cases as recorded_cases, isolated_package, started};
use maestro_app::configuration::config::{APP_NAME, get_theme_directories};
use maestro_app::{NativeExportOperations, ReadonlySessionManager, SessionManager};
use maestro_theme::{ColorMode, ColorValue, Theme, ThemeColor, ThemeDirectories, ThemeOptions};
use maestro_tui::Component;
use serde::Deserialize;
use serde_json::{Value, json};
use std::{path::Path, rc::Rc};

/// Every recorded argument case.
const FIXTURES: &str = include_str!("../../../tests/fixtures/interactive_transfer.json");

/// One recorded argument read.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Argument {
    /// The command token the argument follows.
    command: String,
    /// The submitted text.
    text: String,
}

/// What the reader returned: whether it returned anything at all, and what.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Parsed {
    /// Whether a path argument was present.
    present: bool,
    /// The returned bytes, which are empty for a present empty argument.
    path: Option<String>,
}

/// Compares every recorded case of `test` with the reader's result, keeping a present
/// empty argument apart from an absent one.
fn reads(test: &str, expected_cases: usize) {
    let cases: Vec<(Argument, Parsed)> = recorded_cases(FIXTURES, test);
    assert_eq!(cases.len(), expected_cases);
    for (input, expected) in cases {
        let observed = get_path_command_argument(&input.text, &input.command);
        assert_eq!(observed.is_some(), expected.present, "{:?}", input.text);
        assert_eq!(
            observed.map(str::to_owned),
            expected.path,
            "{:?}",
            input.text
        );
    }
}

#[test]
fn transfer_path_argument_keeps_token_quotes_and_literal_bytes() {
    reads(
        "transfer_path_argument_keeps_token_quotes_and_literal_bytes",
        20,
    );
}

#[test]
fn transfer_path_argument_uses_ecmascript_whitespace() {
    reads("transfer_path_argument_uses_ecmascript_whitespace", 50);
}

#[test]
fn transfer_import_argument_preserves_quotes_apostrophes_and_command_boundary() {
    reads(
        "transfer_import_argument_preserves_quotes_apostrophes_and_command_boundary",
        5,
    );
}

/// The directory the export bundle of `layout` sits in inside `package`.
fn layout_dir(package: &Path, layout: &str) -> std::path::PathBuf {
    package.join(layout).join("core").join("export-html")
}

/// Writes the five bundle assets, each carrying `mark` so its document names its bundle.
fn bundle(directory: &Path, mark: &str) {
    let vendor = directory.join("vendor");
    std::fs::create_dir_all(&vendor).unwrap();
    for (path, body) in [
        (
            directory.join("template.html"),
            format!(
                "{mark}-HTML|{{{{CSS}}}}|{{{{JS}}}}|{{{{MARKED_JS}}}}|{{{{HIGHLIGHT_JS}}}}|{{{{SESSION_DATA}}}}"
            ),
        ),
        (
            directory.join("template.css"),
            format!("{mark}-CSS|{{{{THEME_VARS}}}}"),
        ),
        (directory.join("template.js"), format!("{mark}-JS")),
        (vendor.join("marked.min.js"), format!("{mark}-MARKED")),
        (vendor.join("highlight.min.js"), format!("{mark}-HIGHLIGHT")),
    ] {
        std::fs::write(path, body).unwrap();
    }
}

/// Copies the shipped theme documents into the bundle's package so the registry reads
/// them, returning the directories it resolves for the running checkout.
fn themes(package: &Path) -> ThemeDirectories {
    let shipped = Path::new(env!("CARGO_MANIFEST_DIR")).join("../maestro-theme/assets/theme");
    let themes_dir = package.join("crates/maestro-theme/assets/theme");
    std::fs::create_dir_all(&themes_dir).unwrap();
    for name in ["dark.json", "light.json"] {
        std::fs::copy(shipped.join(name), themes_dir.join(name)).unwrap();
    }
    let directories = get_theme_directories().unwrap();
    std::fs::create_dir_all(&directories.custom_themes_dir).unwrap();
    directories
}

/// A published two-branch history installed into the mode's shared session handle: the
/// current branch keeps `BRANCH_ONE` and `BRANCH_TWO`, its sibling keeps `ANSWER_ONE`.
fn branches(world: &World, directory: &Path) {
    let cwd = directory.to_str().unwrap();
    let sessions = directory.join("sessions");
    let mut manager = SessionManager::create(cwd, Some(sessions.to_str().unwrap()), cwd).unwrap();
    let one = manager
        .append_message(json!({"role": "user", "content": "BRANCH_ONE", "timestamp": 1}))
        .unwrap();
    manager
        .append_message(
            json!({"role": "assistant", "content": [{"type": "text", "text": "ANSWER_ONE"}],
            "api": "controlled", "provider": "controlled", "model": "controlled",
            "stopReason": "stop", "timestamp": 2}),
        )
        .unwrap();
    manager.branch(&one).unwrap();
    manager
        .append_message(json!({"role": "user", "content": "BRANCH_TWO", "timestamp": 3}))
        .unwrap();
    *world.host.session().session_manager().borrow_mut() = manager;
}

/// A started mode over `directory` whose session is a published two-branch history.
fn exportable(directory: &Path) -> World {
    let world = started(directory);
    branches(&world, directory);
    world
}

/// A started mode over `directory` whose session is published and whose theme registry
/// reads the shipped documents the bundle package carries.
fn themed(directory: &Path, package: &Path) -> World {
    let directories = themes(package);
    let world = Setup {
        directories: Some(directories),
        ..Setup::default()
    }
    .build(directory);
    world.mode.init().unwrap();
    world.runtime.settle().unwrap();
    branches(&world, directory);
    world
}

/// The process directory as spelled in returned paths.
fn cwd() -> String {
    std::env::current_dir().unwrap().display().to_string()
}

/// `world`'s transcript rows rendered wide enough to hold them.
fn rows(world: &World) -> Vec<String> {
    world.mode.chat_container.render(WIDTH)
}

/// The row width every notice assertion renders at, wider than any recorded notice.
const WIDTH: usize = 200;

/// Asserts the transcript's last row is `message` painted in `role`, padded to the width.
fn assert_last_row(world: &World, role: &ThemeColor, message: &str, label: &str) {
    let theme = world.themes.theme().get().unwrap();
    let painted = format!(" {}{message}\u{1b}[39m", theme.get_fg_ansi(role).unwrap());
    let padding = WIDTH.saturating_sub(1 + message.chars().count());
    let expected = format!("{painted}{}", " ".repeat(padding));
    assert_eq!(rows(world).last().cloned(), Some(expected), "{label}");
}

/// Asserts the transcript's last row carries `needle`.
fn assert_last_row_holds(world: &World, needle: &str, label: &str) {
    let row = rows(world).last().cloned().unwrap_or_default();
    assert!(row.contains(needle), "{label}: {row}");
}

/// The session file's base name without its `.jsonl` suffix.
fn session_stem(world: &World) -> String {
    let session = world.host.session();
    let manager = session.session_manager().borrow();
    let file = manager.get_session_file().unwrap_or_default().to_owned();
    drop(manager);
    maestro_path::basename(&file, Some(".jsonl"))
}

/// The leaf entry the session selects, if any.
fn leaf(world: &World) -> Option<String> {
    let session = world.host.session();
    let manager = session.session_manager().borrow();
    manager.get_leaf_id().map(str::to_owned)
}

/// The format one export submission selects.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Format {
    /// The current branch as JSONL lines.
    Jsonl,
    /// The whole session as an HTML document.
    Html,
}

/// The eleven export submissions and the format each selects; `Some` is an authored path.
const EXPORT_CASES: [(&str, Format, Option<&str>); 11] = [
    ("/export", Format::Html, None),
    ("/export notes.html", Format::Html, Some("notes.html")),
    ("/export \"\"", Format::Html, None),
    ("/export \"unmatched.html", Format::Html, None),
    ("/export branch.jsonl", Format::Jsonl, Some("branch.jsonl")),
    ("/export upper.JSONL", Format::Html, Some("upper.JSONL")),
    (
        "/export dashed.jsonl-summary",
        Format::Html,
        Some("dashed.jsonl-summary"),
    ),
    (
        "/export \"with space.jsonl\"",
        Format::Jsonl,
        Some("with space.jsonl"),
    ),
    (
        "/export ünïcode.jsonl",
        Format::Jsonl,
        Some("ünïcode.jsonl"),
    ),
    (
        "/export picked.jsonl ignored.txt",
        Format::Jsonl,
        Some("picked.jsonl"),
    ),
    (
        "/export archive.jsonl.bak",
        Format::Html,
        Some("archive.jsonl.bak"),
    ),
];

#[test]
fn transfer_export_selects_suffix_and_reports_the_returned_path() {
    isolated_package(
        "transfer_export_selects_suffix_and_reports_the_returned_path",
        |directory, package| {
            bundle(&layout_dir(package, "src"), "SRC");
            let world = themed(directory, package);
            for (text, format, authored) in EXPORT_CASES {
                submit(&world.mode, text);
                let returned = match (format, authored) {
                    // The JSONL owner resolves the authored path against the process
                    // directory and returns that resolution; the HTML owner returns the
                    // authored path, or a default name when it selected none.
                    (Format::Jsonl, Some(path)) => format!("{}/{path}", cwd()),
                    (_, Some(path)) => path.to_owned(),
                    (_, None) => format!("maestro-session-{}.html", session_stem(&world)),
                };
                let message = format!("Session exported to: {returned}");
                assert_last_row(&world, &ThemeColor::Dim, &message, text);
                let written = std::fs::read_to_string(&returned).unwrap_or_default();
                match format {
                    Format::Jsonl => assert!(
                        written.starts_with("{\"type\":\"session\""),
                        "{text}: {written:.40}"
                    ),
                    Format::Html => {
                        assert!(written.starts_with("SRC-HTML|"), "{text}: {written:.40}");
                    }
                }
            }
        },
    );
}

#[test]
fn transfer_export_errors_are_recoverable_and_keep_the_exact_prefix() {
    isolated_package(
        "transfer_export_errors_are_recoverable_and_keep_the_exact_prefix",
        |directory, package| {
            bundle(&layout_dir(package, "src"), "SRC");
            let world = exportable(directory);
            // A file where the export needs a directory refuses both export routes.
            std::fs::write(directory.join("blocked"), "").unwrap();
            let session = world.host.session();
            let before = leaf(&world);
            let template_dir = layout_dir(package, "src");
            let operations = NativeExportOperations {
                app_name: APP_NAME,
                template_dir: template_dir.to_str().unwrap(),
                theme: &world.themes,
            };
            for (text, authored, jsonl) in [
                ("/export blocked/one.jsonl", "blocked/one.jsonl", true),
                ("/export blocked/one.html", "blocked/one.html", false),
            ] {
                let diagnostic = if jsonl {
                    session.export_to_jsonl(Some(authored)).unwrap_err()
                } else {
                    session
                        .export_to_html(
                            Some(authored),
                            &operations,
                            Rc::clone(&world.mode.tool_environment),
                        )
                        .unwrap_err()
                };
                submit(&world.mode, text);
                let message = format!("Error: Failed to export session: {diagnostic}");
                assert_last_row(&world, &ThemeColor::Error, &message, text);
            }
            // Neither failure replaced the session or delivered a prompt.
            assert_eq!(leaf(&world), before);
            assert!(recalled(&world.mode.editor).is_empty());
        },
    );
}

/// The eight submissions and whether the mode handles each as a command.
const SUBMISSIONS: [(&str, bool); 8] = [
    ("/export", true),
    ("  /export  ", true),
    ("/export picks.jsonl", true),
    ("/export \u{feff}picks.html", true),
    ("/exporter picks.jsonl", false),
    ("/export\tpicks.jsonl", false),
    ("ordinary text", false),
    ("\u{feff}", false),
];

#[test]
fn transfer_installed_submit_consumes_only_export_commands() {
    isolated_package(
        "transfer_installed_submit_consumes_only_export_commands",
        |directory, package| {
            bundle(&layout_dir(package, "src"), "SRC");
            let world = themed(directory, package);
            // The history the earlier submissions left, oldest first.
            let mut recorded: Vec<String> = Vec::new();
            for (text, command) in SUBMISSIONS {
                let mut expected: Vec<String> = recorded.clone();
                expected.reverse();
                assert_eq!(recalled(&world.mode.editor), expected, "before {text}");
                let mut waiter = waiting(&world.mode);
                submit(&world.mode, text);
                let delivered: Vec<String> = poll(&mut waiter).into_iter().collect();
                let pending = world.mode.on_input_callback.borrow().is_some();
                if command {
                    // A recognized command answers on the transcript and leaves the
                    // waiting input and the history alone.
                    assert!(delivered.is_empty(), "{text}");
                    assert!(pending, "{text}");
                    assert!(
                        rows(&world)
                            .last()
                            .is_some_and(|row| row.contains("Session exported to: ")),
                        "{text}"
                    );
                    continue;
                }
                let trimmed = text.trim_matches(maestro_app::is_whitespace_scalar);
                if trimmed.is_empty() {
                    // Blank input reaches neither the waiting input nor the history.
                    assert!(delivered.is_empty(), "{text}");
                    assert!(pending, "{text}");
                    continue;
                }
                assert_eq!(delivered, [trimmed], "{text}");
                assert!(!pending, "{text}");
                recorded.push(trimmed.to_owned());
            }
            recorded.reverse();
            assert_eq!(recalled(&world.mode.editor), recorded);
        },
    );
}

#[test]
fn transfer_submit_clears_after_export_failure_without_consuming_input() {
    isolated_package(
        "transfer_submit_clears_after_export_failure_without_consuming_input",
        |directory, package| {
            bundle(&layout_dir(package, "src"), "SRC");
            let world = exportable(directory);
            std::fs::write(directory.join("blocked"), "").unwrap();
            let mut waiter = waiting(&world.mode);
            submit(&world.mode, "/export blocked/lost.jsonl");
            assert_last_row_holds(&world, "Error: Failed to export session: ", "a failure");
            assert_eq!(poll(&mut waiter), None);
            assert!(world.mode.on_input_callback.borrow().is_some());
            // The same waiting input receives a later ordinary submission.
            submit(&world.mode, " later text ");
            assert_eq!(poll(&mut waiter).as_deref(), Some("later text"));
            assert_eq!(recalled(&world.mode.editor), ["later text"]);
        },
    );
}

#[test]
fn transfer_jsonl_command_writes_current_branch_with_original_path() {
    isolated_package(
        "transfer_jsonl_command_writes_current_branch_with_original_path",
        |directory, package| {
            bundle(&layout_dir(package, "src"), "SRC");
            let world = exportable(directory);
            let before = leaf(&world);
            let mut waiter = waiting(&world.mode);
            submit(&world.mode, "/export \"with space.jsonl\"");
            let returned = format!("{}/with space.jsonl", cwd());
            assert_last_row(
                &world,
                &ThemeColor::Dim,
                &format!("Session exported to: {returned}"),
                "the authored path with spaces",
            );
            let body = std::fs::read_to_string(&returned).unwrap();
            let entries: Vec<Value> = body
                .lines()
                .map(|line| serde_json::from_str(line).unwrap())
                .collect();
            assert_eq!(entries[0]["type"], "session");
            let texts: Vec<&str> = entries[1..]
                .iter()
                .map(|entry| entry["message"]["content"].as_str().unwrap_or_default())
                .collect();
            assert_eq!(texts, ["BRANCH_ONE", "BRANCH_TWO"]);
            // The written branch is re-chained from no parent.
            assert_eq!(entries[1]["parentId"], Value::Null);
            assert_eq!(entries[2]["parentId"], entries[1]["id"]);
            // Nothing reached a model: the branch and the waiting input are unchanged.
            assert_eq!(leaf(&world), before);
            assert_eq!(poll(&mut waiter), None);
            assert!(recalled(&world.mode.editor).is_empty());
        },
    );
}

#[test]
fn transfer_html_command_uses_native_assets_and_the_current_session_environment() {
    isolated_package(
        "transfer_html_command_uses_native_assets_and_the_current_session_environment",
        |directory, package| {
            bundle(&layout_dir(package, "src"), "SRC");
            let world = themed(directory, package);
            submit(&world.mode, "/export page.html");
            assert_last_row(
                &world,
                &ThemeColor::Dim,
                "Session exported to: page.html",
                "an authored page path",
            );
            let document = std::fs::read_to_string(directory.join("page.html")).unwrap();
            assert!(document.starts_with("SRC-HTML|SRC-CSS|"), "{document:.40}");
            // The same bundle, theme and environment through the application owner
            // produce the same document, so the mode forwarded all three unchanged.
            let template_dir = layout_dir(package, "src");
            let operations = NativeExportOperations {
                app_name: APP_NAME,
                template_dir: template_dir.to_str().unwrap(),
                theme: &world.themes,
            };
            let direct = world
                .host
                .session()
                .export_to_html(
                    Some(&format!("{}/direct.html", cwd())),
                    &operations,
                    Rc::clone(&world.mode.tool_environment),
                )
                .unwrap();
            assert_eq!(
                std::fs::read_to_string(&direct).unwrap(),
                document,
                "the mode's document differs from the owner's"
            );
            // The document carries the colours of the theme document the settings name.
            assert!(
                document.contains("--accent: #"),
                "the document lacks the selected theme's accent"
            );
        },
    );
}

#[test]
fn transfer_html_native_preparation_keeps_package_layout_and_lazy_branch_order() {
    isolated_package(
        "transfer_html_native_preparation_keeps_package_layout_and_lazy_branch_order",
        |directory, package| {
            bundle(&layout_dir(package, "src"), "SRC");
            bundle(&layout_dir(package, "dist"), "DIST");
            let world = themed(directory, package);
            // The source checkout selects its source bundle, not the distribution one.
            submit(&world.mode, "/export chosen.html");
            let chosen = std::fs::read_to_string(directory.join("chosen.html")).unwrap();
            assert!(chosen.starts_with("SRC-HTML|"), "{chosen:.40}");
            assert!(!chosen.contains("DIST-"), "{chosen:.40}");
            // Without either bundle the JSONL branch still succeeds: it prepares no HTML
            // bundle at all.
            std::fs::remove_dir_all(package.join("src")).unwrap();
            std::fs::remove_dir_all(package.join("dist")).unwrap();
            let returned = format!("{}/ready.jsonl", cwd());
            submit(&world.mode, "/export ready.jsonl");
            assert_last_row(
                &world,
                &ThemeColor::Dim,
                &format!("Session exported to: {returned}"),
                "JSONL without HTML assets",
            );
            assert!(Path::new(&returned).exists());
            // The HTML branch reports the missing bundle as a recoverable failure.
            submit(&world.mode, "/export gone.html");
            assert_last_row_holds(
                &world,
                "Error: Failed to export session: ",
                "missing assets",
            );
        },
    );
}

#[test]
fn transfer_export_failure_keeps_waiter_and_releases_the_mode() {
    isolated_package(
        "transfer_export_failure_keeps_waiter_and_releases_the_mode",
        |directory, package| {
            bundle(&layout_dir(package, "src"), "SRC");
            let World {
                mode,
                terminal,
                runtime,
                themes,
                host,
                loader,
            } = exportable(directory);
            std::fs::write(directory.join("blocked"), "").unwrap();
            let mut waiter = waiting(&mode);
            submit(&mode, "/export blocked/lost.jsonl");
            assert_eq!(poll(&mut waiter), None);
            assert!(mode.on_input_callback.borrow().is_some());
            let editor = Rc::clone(&mode.editor);
            let callback = editor.on_submit().unwrap();
            let weak = Rc::downgrade(&mode);
            drop(mode);
            assert!(weak.upgrade().is_none());
            // The retained callback neither revives the mode nor writes anything.
            callback("/export released.html");
            assert!(weak.upgrade().is_none());
            assert!(!directory.join("released.html").exists());
            assert_eq!(poll(&mut waiter), None);
            assert!(recalled(&editor).is_empty());
            drop((terminal, runtime, themes, host, loader));
        },
    );
}

#[test]
fn transfer_notice_failure_uses_existing_runtime_error_route() {
    isolated_package(
        "transfer_notice_failure_uses_existing_runtime_error_route",
        |directory, package| {
            bundle(&layout_dir(package, "src"), "SRC");
            let world = exportable(directory);
            // A published theme without the status colour makes the notice fail after the
            // export itself wrote its file.
            let bare = Rc::new(
                Theme::new(
                    [(
                        ThemeColor::Named("thinkingOff".to_owned()),
                        ColorValue::Index(1),
                    )],
                    [],
                    ColorMode::Truecolor,
                    ThemeOptions::default(),
                )
                .unwrap(),
            );
            world.themes.set_theme_instance(bare).unwrap();
            let frames = world.runtime.pending();
            submit(&world.mode, "/export reported.jsonl");
            assert!(directory.join("reported.jsonl").exists());
            assert_eq!(world.runtime.pending(), frames, "no frame was requested");
            let errors = world.runtime.drain_futures();
            assert_eq!(errors.len(), 1, "the runtime route reports once");
            assert_eq!(errors[0].to_string(), "Unknown theme color: dim");
        },
    );
}
