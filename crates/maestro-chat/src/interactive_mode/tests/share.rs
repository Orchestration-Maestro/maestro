//! The share and copy commands through the installed submission and their controlled
//! native programs.
use super::super::share::ShareArtifact;
use super::transfer::{assert_last_row_holds, bundle, layout_dir, rows, themed};
use super::typed::{poll, recalled, submit, waiting};
use super::{World, cases as recorded_cases, started};
use crate::components::session_selector::list::tests::session_picker::native_child as child;
use maestro_app::configuration::config::{
    APP_NAME, ENV_AGENT_DIR, SERVICE_ORIGIN, get_share_viewer_url,
};
use maestro_app::{NativeAgentMessage, SessionManager};
use maestro_test_process::TestProcess;
use maestro_theme::ThemeColor;
use serde::Deserialize;
use serde_json::{Value, json};
use std::{
    cell::RefCell,
    fs,
    path::{Path, PathBuf},
    rc::Rc,
    sync::LazyLock,
};

/// Every recorded command case.
const FIXTURES: &str = include_str!("../../../tests/fixtures/interactive_share.json");

/// The package-directory environment key the installation reads.
static ENV_PACKAGE_DIR: LazyLock<String> =
    LazyLock::new(|| format!("{}_PACKAGE_DIR", APP_NAME.to_uppercase()));

/// The state directory the controlled program reads its configuration from.
const ENV_STATE: &str = "MAESTRO_SHARE_STATE";

/// The program directory the child puts first on its own path.
const ENV_PROGRAMS: &str = "MAESTRO_SHARE_PROGRAMS";

/// The temporary directory the child's artifacts are created in.
const ENV_TEMP: &str = "MAESTRO_SHARE_TEMP";

/// The environment keys a controlled child keeps unset.
const CONTROLLED: [&str; 5] = [
    "DISPLAY",
    "WAYLAND_DISPLAY",
    "SSH_CONNECTION",
    "SSH_CLIENT",
    "TERMUX_VERSION",
];

/// The programs a child must not reach outside its controlled directory.
const HOST_PROGRAMS: [&str; 4] = ["gh", "xclip", "xsel", "wl-copy"];

/// The controlled program installed as `gh` for the commands under test.
///
/// It records its arguments, then answers `auth status` with the configured status and
/// `gist create` with the configured frames, so no live GitHub CLI, network or credential
/// is used. A frame writes raw bytes to a stream, pauses, changes a mode or copies the
/// document the command exported.
const GH: &str = r#"#!/usr/bin/python3
import json, os, shutil, socket, sys, time
state = os.environ["MAESTRO_SHARE_STATE"]
with open(os.path.join(state, "config.json")) as source:
    config = json.load(source)
with open(os.path.join(state, "argv.jsonl"), "a") as log:
    log.write(json.dumps(sys.argv[1:]) + "\n")
    log.flush()
if sys.argv[1:3] == ["auth", "status"]:
    step = config.get("auth", {})
    if step.get("remove_self"):
        os.remove(sys.argv[0])
    if "signal" in step:
        os.kill(os.getpid(), step["signal"])
    sys.exit(step.get("exit", 0))
step = config.get("gist", {})
control = None
if "port" in step:
    control = socket.create_connection(("127.0.0.1", int(step["port"])))
for frame in step.get("frames", []):
    kind = frame["stream"]
    if kind in ("out", "err"):
        os.write(1 if kind == "out" else 2, bytes.fromhex(frame["bytes"]))
    elif kind == "send":
        control.sendall(bytes.fromhex(frame["bytes"]))
    elif kind == "hold":
        assert control.recv(1) == b""
    elif kind == "sleep":
        time.sleep(frame["seconds"])
    elif kind == "chmod_artifact":
        os.chmod(os.path.dirname(sys.argv[-1]), int(frame["mode"], 8))
    elif kind == "copy":
        shutil.copy(sys.argv[-1], os.path.join(state, frame["as"]))
sys.exit(step.get("exit", 0))
"#;

/// The recorded gist result of one case.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct GistOutput {
    /// The recorded exit code; absent for a signalled child.
    code: Option<i32>,
    /// The recorded standard output.
    stdout: String,
    /// The recorded standard error.
    stderr: String,
    /// The recorded application origin.
    #[serde(default)]
    origin: Option<String>,
}

/// The recorded report of one gist result.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Report {
    /// The notice helper the recorded result selected.
    kind: String,
    /// The complete text of the notice without the presenter's own prefix.
    text: String,
}

/// One recorded submission.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Submitted {
    /// The submitted text.
    text: String,
}

/// Whether the recorded submission was handled as a command.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Routed {
    /// Whether the mode handled the text as a command.
    routed: bool,
}

/// The recorded messages of one copy case.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct History {
    /// The session's messages, oldest first.
    messages: Vec<Value>,
}

/// The recorded copy result of one case.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Copied {
    /// The text the clipboard owner receives, absent when nothing may be copied.
    text: Option<String>,
    /// The clipboard sequence that owner writes for that text.
    frame: Option<String>,
}

/// The process status a recorded exit code describes.
fn status(code: Option<i32>) -> std::process::ExitStatus {
    use std::os::unix::process::ExitStatusExt as _;
    code.map_or_else(
        || std::process::ExitStatus::from_raw(15),
        |code| std::process::ExitStatus::from_raw(code << 8),
    )
}

/// The captured output of one recorded case, as the process owner returns it.
fn finished(case: &GistOutput) -> std::process::Output {
    std::process::Output {
        status: status(case.code),
        stdout: case.stdout.clone().into_bytes(),
        stderr: case.stderr.clone().into_bytes(),
    }
}

/// The `PATH` of the host without any directory carrying one of `names`.
fn path_without(names: &[&str]) -> String {
    std::env::var("PATH")
        .unwrap_or_default()
        .split(':')
        .filter(|directory| {
            !names
                .iter()
                .any(|name| Path::new(directory).join(name).exists())
        })
        .collect::<Vec<_>>()
        .join(":")
}

/// Runs `case` of `test` in a fresh child; the parent receives its standard output.
///
/// The child gets a disposable agent directory, package directory, program directory
/// holding `program` as `gh` with `mode`, state directory, temporary directory and a path
/// whose only GitHub CLI and clipboard programs are the controlled ones. `body` runs in
/// the selected child over its agent directory.
fn run(
    test: &str,
    case: &str,
    program: Option<(&str, &str)>,
    body: impl AsyncFnOnce(&Path),
) -> Option<String> {
    if maestro_test_process::selection().is_child() {
        if maestro_test_process::selection().runs_case(case) {
            super::on_tasks(body(Path::new(&std::env::var(&*ENV_AGENT_DIR).unwrap())));
        }
        return None;
    }
    let scratch = child::Scratch::new();
    let root = scratch.0.path().to_path_buf();
    let package = root.join("package");
    let programs = root.join("programs");
    let state = root.join("state");
    let temp = root.join("temp");
    for directory in [&package, &programs, &state, &temp] {
        fs::create_dir(directory).unwrap();
    }
    if let Some((source, mode)) = program {
        fs::write(root.join("gh-source"), source).unwrap();
        assert!(
            std::process::Command::new("/usr/bin/install")
                .args(["-m", mode])
                .arg(root.join("gh-source"))
                .arg(programs.join("gh"))
                .status()
                .unwrap()
                .success(),
            "the controlled program is installed"
        );
    }
    let path = format!("{}:{}", programs.display(), path_without(&HOST_PROGRAMS));
    let mut command = TestProcess::current(test)
        .unwrap()
        .case(case)
        .nocapture()
        .current_dir(&root)
        .temp_dir(&temp)
        .env(ENV_AGENT_DIR.as_str(), &root)
        .env(&*ENV_PACKAGE_DIR, &package)
        .env(ENV_STATE, &state)
        .env(ENV_PROGRAMS, &programs)
        .env(ENV_TEMP, &temp)
        .env("PATH", path);
    for key in CONTROLLED {
        command = command.env_remove(key);
    }
    let finished = command.output().unwrap();
    assert!(
        finished.status.success(),
        "child {test}/{case} failed: {}",
        String::from_utf8_lossy(&finished.stderr)
    );
    Some(String::from_utf8_lossy(&finished.stdout).into_owned())
}

/// Whether the copy route starts at the controlled programs and the terminal sequence. On a
/// host with a system clipboard the route reaches that clipboard first, outside the child's
/// control, so the copy cases report the premise there instead of writing the real clipboard.
fn clipboard_route_controlled(test: &str) -> bool {
    let controlled = !cfg!(any(target_os = "macos", windows));
    if !controlled {
        eprintln!("{test} needs a host whose copy route starts at the controlled programs");
    }
    controlled
}

/// The directory the child's temporary artifacts are created in.
fn temp_root() -> PathBuf {
    PathBuf::from(std::env::var(ENV_TEMP).unwrap())
}

/// The state directory of the child.
fn state_root() -> PathBuf {
    PathBuf::from(std::env::var(ENV_STATE).unwrap())
}

/// The artifact directories present in the child's temporary directory.
fn artifacts() -> Vec<PathBuf> {
    let mut found: Vec<PathBuf> = fs::read_dir(temp_root())
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .filter(|path| {
            path.file_name()
                .is_some_and(|name| name.to_string_lossy().starts_with("maestro-share-"))
        })
        .collect();
    found.sort();
    found
}

/// Writes the controlled program's configuration for its next call.
fn configure(settings: &Value) {
    fs::write(
        state_root().join("config.json"),
        serde_json::to_string(settings).unwrap(),
    )
    .unwrap();
}

/// Forgets the arguments recorded so far.
fn forget_arguments() {
    drop(fs::remove_file(state_root().join("argv.jsonl")));
}

/// The arguments the controlled program received, in call order.
fn arguments() -> Vec<Vec<String>> {
    fs::read_to_string(state_root().join("argv.jsonl"))
        .unwrap_or_default()
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect()
}

/// The clipboard escape sequences written to standard output, in the order written.
fn frames(stdout: &str) -> Vec<&str> {
    let mut written = Vec::new();
    let mut rest = stdout;
    while let Some(start) = rest.find("\u{1b}]52;c;") {
        let tail = &rest[start..];
        let Some(end) = tail.find('\u{7}') else {
            break;
        };
        written.push(&tail[..=end]);
        rest = &tail[end + 1..];
    }
    written
}

/// The published session of `world` holding `messages`, oldest first.
///
/// The session file receives every message and the agent receives the same history, so
/// both the export and the live answer-time getters read it.
fn publish(world: &World, directory: &Path, messages: &[Value]) {
    let cwd = directory.to_str().unwrap();
    let sessions = directory.join("sessions");
    let mut manager = SessionManager::create(cwd, Some(sessions.to_str().unwrap()), cwd).unwrap();
    for message in messages {
        manager.append_message(message.clone()).unwrap();
    }
    *world.host.session().session_manager().borrow_mut() = manager;
    let history: Vec<NativeAgentMessage> = serde_json::from_value(json!(messages)).unwrap();
    world
        .host
        .session()
        .agent()
        .state()
        .write()
        .unwrap()
        .set_messages(history);
}

/// A started mode over `directory` whose session holds `messages`.
async fn with_messages(directory: &Path, messages: &[Value]) -> World {
    let world = started(directory).await;
    publish(&world, directory, messages);
    world
}

/// One recorded assistant answer.
fn answer(text: &str) -> Value {
    json!({"role": "assistant", "content": [{"type": "text", "text": text}],
        "api": "faux", "provider": "faux", "model": "plain",
        "usage": {"input": 1, "output": 1, "cacheRead": 0, "cacheWrite": 0, "totalTokens": 2,
            "cost": {"input": 0, "output": 0, "cacheRead": 0, "cacheWrite": 0, "total": 0}},
        "stopReason": "stop", "timestamp": 2})
}

/// Runs the single admitted operation to completion, returning the operation's own result.
async fn drive(world: &World) -> Result<(), Box<dyn std::error::Error>> {
    let mut admitted = world.runtime.take_futures();
    assert_eq!(admitted.len(), 1, "one admitted operation");
    admitted.pop().unwrap().await
}

/// Starts the single admitted operation on the current runtime and polls it once.
fn start(world: &World) -> maestro_tui::tui::LocalFuture {
    let mut admitted = world.runtime.take_futures();
    assert_eq!(admitted.len(), 1, "one admitted operation");
    let mut operation = admitted.pop().unwrap();
    assert!(poll(&mut operation).is_none(), "the operation is running");
    operation
}

/// The last transcript row of `world`, rendered wide.
fn last_row(world: &World) -> String {
    rows(world).last().cloned().unwrap_or_default()
}

/// Whether the transcript's last row holds `needle`.
fn last_row_holds(world: &World, needle: &str) -> bool {
    last_row(world).contains(needle)
}

/// The transcript rows of the latest notice, one per displayed line.
///
/// The padding is dropped without counting the text's characters, because a row is
/// measured in display cells and a recorded notice may hold a scalar of no width.
fn notice_rows(world: &World) -> Vec<String> {
    let rendered = rows(world);
    let start = rendered
        .iter()
        .rposition(|row| row.trim().is_empty())
        .map_or(0, |index| index + 1);
    rendered[start..]
        .iter()
        .map(|row| row.trim_end_matches(' ').to_owned())
        .collect()
}

/// Advances past one loader animation interval and asserts nothing stays scheduled, which
/// a stopped loader leaves behind and a running animation does not.
fn assert_loader_stopped(world: &World) {
    drop(
        world
            .runtime
            .advance_to(std::time::Duration::from_millis(500)),
    );
    assert_eq!(world.runtime.pending(), 0, "the loader is disposed");
}

/// Asserts the transcript's latest notice is exactly `text` painted in `color`.
fn assert_notice(world: &World, color: &ThemeColor, text: &str) {
    let theme = world.themes.theme().get().unwrap();
    let painted = format!(" {}", theme.get_fg_ansi(color).unwrap());
    let opened: Vec<String> = text
        .split('\n')
        .map(|line| format!("{painted}{line}"))
        .collect();
    let observed = notice_rows(world).join("\n");
    assert_eq!(observed, format!("{}\u{1b}[39m", opened.join("\n")));
}

/// The complete text of one recorded notice, with the presenter's own prefix.
fn presented(report: &Report) -> (ThemeColor, String) {
    match report.kind.as_str() {
        "error" => (ThemeColor::Error, format!("Error: {}", report.text)),
        "status" => (ThemeColor::Dim, report.text.clone()),
        other => panic!("a recorded notice kind, not {other}"),
    }
}

/// Replays the recorded results of `test` through the private report formatter.
fn reports(test: &str, expected: usize) {
    let cases: Vec<(GistOutput, Report)> = recorded_cases(FIXTURES, test);
    assert_eq!(cases.len(), expected);
    drop(run(test, "child", None, async |directory| {
        for (case, report) in &cases {
            let world = started(directory).await;
            let origin = case.origin.clone().unwrap_or_default();
            world
                .mode
                .show_share_result(&finished(case), &origin)
                .unwrap();
            let (color, notice) = presented(report);
            assert_notice(&world, &color, &notice);
        }
    }));
}

/// The package directory of the running child.
fn package_root() -> PathBuf {
    PathBuf::from(std::env::var(&*ENV_PACKAGE_DIR).unwrap())
}

/// A started mode over `directory` whose package carries the export bundle.
async fn exportable(directory: &Path) -> World {
    let package = package_root();
    bundle(&layout_dir(&package, "src"), "SRC");
    themed(directory, &package).await
}

/// The hexadecimal bytes of `text`, for a controlled frame.
fn hex(text: &str) -> String {
    use std::fmt::Write as _;
    text.bytes().fold(String::new(), |mut hex, byte| {
        let written = write!(hex, "{byte:02x}");
        assert!(written.is_ok());
        hex
    })
}

/// One byte as the hexadecimal text of a controlled frame.
fn hex_byte(byte: u8) -> String {
    format!("{byte:02x}")
}

#[test]
fn share_failed_exit_keeps_trimmed_stderr_or_unknown_error() {
    reports("share_failed_exit_keeps_trimmed_stderr_or_unknown_error", 5);
}

#[test]
fn share_output_uses_last_slash_segment() {
    reports("share_output_uses_last_slash_segment", 10);
}

#[test]
fn share_formats_gist_with_configured_or_empty_viewer() {
    const TEST: &str = "share_formats_gist_with_configured_or_empty_viewer";
    let cases: Vec<(GistOutput, Report)> = recorded_cases(FIXTURES, TEST);
    assert_eq!(cases.len(), 2);
    drop(run(TEST, "child", None, async |directory| {
        for (case, report) in &cases {
            let world = started(directory).await;
            let origin = case.origin.clone().unwrap_or_default();
            world
                .mode
                .show_share_result(&finished(case), &origin)
                .unwrap();
            let gist = case.stdout.trim_matches(maestro_app::is_whitespace_scalar);
            let id = gist.rsplit('/').next().unwrap();
            // The selected identifier reaches the application's viewer owner unchanged.
            let expected = get_share_viewer_url(id, &origin).map_or_else(
                || format!("Gist: {gist}"),
                |viewer| format!("Share URL: {viewer}\nGist: {gist}"),
            );
            assert_eq!(expected, report.text);
            assert_eq!(origin == SERVICE_ORIGIN, report.text.starts_with("Gist: "));
            assert_notice(&world, &ThemeColor::Dim, &expected);
        }
    }));
}

#[test]
fn copy_refuses_when_no_assistant_text() {
    const TEST: &str = "copy_refuses_when_no_assistant_text";
    let cases: Vec<(History, Copied)> = recorded_cases(FIXTURES, TEST);
    assert_eq!(cases.len(), 4);
    let Some(stdout) = run(TEST, "child", None, async |directory| {
        for (case, expected) in &cases {
            assert_eq!(expected.text, None);
            let world = with_messages(directory, &case.messages).await;
            submit(&world.mode, "/copy");
            drive(&world).await.unwrap();
            assert_notice(
                &world,
                &ThemeColor::Error,
                "Error: No agent messages to copy yet.",
            );
        }
    }) else {
        return;
    };
    assert!(frames(&stdout).is_empty(), "nothing reached the clipboard");
}

#[test]
fn copy_uses_current_last_assistant_and_reports_result() {
    const TEST: &str = "copy_uses_current_last_assistant_and_reports_result";
    if !clipboard_route_controlled(TEST) {
        return;
    }
    let cases: Vec<(History, Copied)> = recorded_cases(FIXTURES, TEST);
    assert_eq!(cases.len(), 6);
    let Some(stdout) = run(TEST, "child", None, async |directory| {
        for (case, _) in &cases {
            let world = with_messages(directory, &case.messages).await;
            submit(&world.mode, "/copy");
            drive(&world).await.unwrap();
            assert_notice(
                &world,
                &ThemeColor::Dim,
                "Copied last agent message to clipboard",
            );
        }
        // The command reads the current session, including one replaced between calls.
        let world = started(directory).await;
        publish(&world, directory, &[answer("replaced")]);
        submit(&world.mode, "/copy");
        drive(&world).await.unwrap();
        assert_notice(
            &world,
            &ThemeColor::Dim,
            "Copied last agent message to clipboard",
        );
    }) else {
        return;
    };
    let expected: Vec<&str> = cases
        .iter()
        .filter_map(|(_, copied)| copied.frame.as_deref())
        .chain(["\u{1b}]52;c;cmVwbGFjZWQ=\u{7}"])
        .collect();
    assert_eq!(
        frames(&stdout),
        expected,
        "each answer reached the clipboard once"
    );
}

#[test]
fn copy_reports_clipboard_failure() {
    const TEST: &str = "copy_reports_clipboard_failure";
    if !clipboard_route_controlled(TEST) {
        return;
    }
    let Some(stdout) = run(TEST, "child", None, async |directory| {
        // Beyond the terminal fallback's 100000 encoded bytes with no desktop route.
        let world = with_messages(directory, &[answer(&"a".repeat(90_000))]).await;
        submit(&world.mode, "/copy");
        drive(&world).await.unwrap();
        assert_notice(
            &world,
            &ThemeColor::Error,
            "Error: Failed to copy to clipboard",
        );
        assert!(!last_row_holds(&world, "Copied last"));
    }) else {
        return;
    };
    assert!(frames(&stdout).is_empty(), "no copied escape sequence");
}

#[test]
fn share_reports_missing_cli_before_auth_failure() {
    const TEST: &str = "share_reports_missing_cli_before_auth_failure";
    drop(run(TEST, "missing", None, async |directory| {
        let world = started(directory).await;
        submit(&world.mode, "/share");
        drive(&world).await.unwrap();
        assert_notice(
            &world,
            &ThemeColor::Error,
            "Error: GitHub CLI (gh) is not installed. Install it from https://cli.github.com/",
        );
        assert!(artifacts().is_empty(), "no export was started");
        assert!(arguments().is_empty(), "no program ran");
    }));
    // The same program installed without execute permission is refused at startup too.
    drop(run(TEST, "denied", Some((GH, "0600")), async |directory| {
        let world = started(directory).await;
        submit(&world.mode, "/share");
        drive(&world).await.unwrap();
        assert_notice(
            &world,
            &ThemeColor::Error,
            "Error: GitHub CLI (gh) is not installed. Install it from https://cli.github.com/",
        );
        assert!(artifacts().is_empty(), "no export was started");
        assert!(arguments().is_empty(), "no program ran");
    }));
}

#[test]
fn share_auth_failure_does_not_export() {
    const TEST: &str = "share_auth_failure_does_not_export";
    drop(run(TEST, "child", Some((GH, "0700")), async |directory| {
        for settings in [
            json!({"auth": {"exit": 1}}),
            json!({"auth": {"signal": 15}}),
        ] {
            forget_arguments();
            configure(&settings);
            let world = started(directory).await;
            submit(&world.mode, "/share");
            drive(&world).await.unwrap();
            assert_notice(
                &world,
                &ThemeColor::Error,
                "Error: GitHub CLI is not logged in. Run 'gh auth login' first.",
            );
            assert_eq!(arguments(), [vec!["auth".to_owned(), "status".to_owned()]]);
            assert!(artifacts().is_empty(), "no export was started");
        }
        // A completed login permits the export and the gist.
        forget_arguments();
        configure(&json!({"auth": {"exit": 0}, "gist": {"frames": [
            {"stream": "out", "bytes": hex("https://gist.github.com/alice/abc123\n")}
        ]}}));
        let world = exportable(directory).await;
        submit(&world.mode, "/share");
        drive(&world).await.unwrap();
        assert_eq!(arguments().len(), 2, "both calls were made");
        assert_notice(
            &world,
            &ThemeColor::Dim,
            "Gist: https://gist.github.com/alice/abc123",
        );
    }));
}

#[test]
fn share_creates_secret_gist_and_restores_editor() {
    const TEST: &str = "share_creates_secret_gist_and_restores_editor";
    drop(run(TEST, "child", Some((GH, "0700")), async |directory| {
        configure(&json!({"auth": {"exit": 0}, "gist": {"frames": [
            {"stream": "copy", "as": "artifact.html"},
            {"stream": "out", "bytes": hex("https://gist.github.com/alice/abc123\n")}
        ]}}));
        let world = exportable(directory).await;
        let editor = Rc::clone(&world.mode.editor);
        submit(&world.mode, "/share");
        let operation = start(&world);
        // The loader replaced the editor, holds the input and names the gist.
        let loader = world.mode.editor_container.children().borrow().clone();
        assert_eq!(loader.len(), 1);
        assert!(!std::ptr::addr_eq(
            Rc::as_ptr(&loader[0]),
            Rc::as_ptr(&editor)
        ));
        assert!(
            loader[0]
                .render(80)
                .iter()
                .any(|row| row.contains("Creating gist..."))
        );
        world.terminal.send_input("typed at the loader");
        assert_eq!(editor.get_text(), "");
        // The export wrote this operation's own document inside its own directory.
        let artifact = artifacts()[0].join("session.html");
        assert!(artifact.exists());
        // Completion restores the same editor and removes the artifact before its notice.
        assert!(operation.await.is_ok());
        assert!(artifacts().is_empty(), "the artifact is gone");
        // The child received that path and read the exported document.
        let calls = arguments();
        let share = calls.last().cloned().unwrap();
        assert_eq!(share[..3], ["gist", "create", "--public=false"]);
        assert_eq!(PathBuf::from(&share[3]), artifact);
        let exported = fs::read_to_string(state_root().join("artifact.html")).unwrap();
        assert!(exported.starts_with("SRC-HTML|"), "{exported:.40}");
        let restored = world.mode.editor_container.children().borrow().clone();
        assert_eq!(restored.len(), 1);
        assert!(std::ptr::addr_eq(
            Rc::as_ptr(&restored[0]),
            Rc::as_ptr(&editor)
        ));
        world.terminal.send_input("later");
        assert_eq!(editor.get_text(), "later");
        assert_notice(
            &world,
            &ThemeColor::Dim,
            "Gist: https://gist.github.com/alice/abc123",
        );
    }));
}

#[test]
fn share_cancel_terminates_child_and_suppresses_late_outcome() {
    use std::io::{BufRead, BufReader, Read as _};
    const TEST: &str = "share_cancel_terminates_child_and_suppresses_late_outcome";
    drop(run(TEST, "child", Some((GH, "0700")), async |directory| {
        // The controlled program announces its gist step on a socket this test owns.
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        configure(
            &json!({"auth": {"exit": 0}, "gist": {"port": port, "frames": [
                {"stream": "send", "bytes": hex("ready\n")},
                {"stream": "hold"}
            ]}}),
        );
        let world = exportable(directory).await;
        let editor = Rc::clone(&world.mode.editor);
        submit(&world.mode, "/share");
        let operation = start(&world);
        // Readiness comes from the running child, not from a pause in this test.
        let (mut control, _) = listener.accept().unwrap();
        let mut ready = String::new();
        BufReader::new(control.try_clone().unwrap())
            .read_line(&mut ready)
            .unwrap();
        assert_eq!(ready, "ready\n");
        world.terminal.send_input("\x1b");
        // The cancellation is handled at once: the editor and its artifact are restored
        // before the operation settles.
        assert_notice(&world, &ThemeColor::Dim, "Share cancelled");
        assert!(artifacts().is_empty(), "the artifact is gone");
        // The wait requests ordinary termination and settles once the child is reaped.
        assert!(operation.await.is_ok());
        let mut reached = [0u8; 1];
        assert_eq!(
            control.read(&mut reached).unwrap(),
            0,
            "the direct child is terminated"
        );
        let restored = world.mode.editor_container.children().borrow().clone();
        assert!(std::ptr::addr_eq(
            Rc::as_ptr(&restored[0]),
            Rc::as_ptr(&editor)
        ));
        // A repeated cancel reaches the restored editor and reports nothing more.
        let before = rows(&world);
        world.terminal.send_input("\x1b");
        assert_eq!(rows(&world), before);
        assert_loader_stopped(&world);
        // A later share runs again and reports its own result.
        configure(&json!({"auth": {"exit": 0}, "gist": {"frames": [
            {"stream": "out", "bytes": hex("https://gist.github.com/alice/second\n")}
        ]}}));
        submit(&world.mode, "/share");
        drive(&world).await.unwrap();
        assert_notice(
            &world,
            &ThemeColor::Dim,
            "Gist: https://gist.github.com/alice/second",
        );
    }));
}

#[test]
fn share_and_copy_commands_clear_editor_after_completion() {
    const TEST: &str = "share_and_copy_commands_clear_editor_after_completion";
    let cases: Vec<(Submitted, Routed)> = recorded_cases(FIXTURES, TEST);
    assert_eq!(cases.len(), 6);
    drop(run(TEST, "child", None, async |directory| {
        for (case, expected) in &cases {
            let world = started(directory).await;
            let mut waiter = waiting(&world.mode);
            submit(&world.mode, &case.text);
            if !expected.routed {
                let trimmed = case.text.trim_matches(maestro_app::is_whitespace_scalar);
                assert_eq!(poll(&mut waiter).as_deref(), Some(trimmed), "{}", case.text);
                assert_eq!(world.runtime.futures(), 0, "{}", case.text);
                assert_eq!(recalled(&world.mode.editor), [trimmed]);
                continue;
            }
            assert_eq!(poll(&mut waiter), None, "{}", case.text);
            assert!(
                world.mode.on_input_callback.borrow().is_some(),
                "{}",
                case.text
            );
            assert!(recalled(&world.mode.editor).is_empty(), "{}", case.text);
            assert_eq!(world.runtime.futures(), 1, "{}", case.text);
            // A buffer seeded while the command is pending is cleared once it settles.
            world.mode.editor.set_text("seeded");
            drive(&world).await.unwrap();
            assert_eq!(world.mode.editor.get_text(), "", "{}", case.text);
            assert_eq!(world.runtime.futures(), 0, "{}", case.text);
        }
    }));
}

#[test]
fn share_export_failure_cleans_only_owned_artifact() {
    const TEST: &str = "share_export_failure_cleans_only_owned_artifact";
    drop(run(TEST, "child", Some((GH, "0700")), async |directory| {
        configure(&json!({"auth": {"exit": 0}}));
        let keep = temp_root().join("keep.txt");
        fs::write(&keep, "unrelated").unwrap();
        // A published session whose package carries no export bundle fails at the
        // exporter's own asset step.
        let published = exportable(directory).await;
        fs::remove_dir_all(layout_dir(&package_root(), "src")).unwrap();
        publish(&published, directory, &[answer("exported")]);
        submit(&published.mode, "/share");
        drive(&published).await.unwrap();
        assert_last_row_holds(
            &published,
            "Error: Failed to export session: ",
            "a missing bundle",
        );
        assert!(keep.exists(), "an unrelated file is kept");
        assert!(artifacts().is_empty(), "the partial artifact is gone");
        assert_eq!(arguments().len(), 1, "no gist was started");
        // A temporary directory that refuses the operation's own directory is reported at
        // the same export step. A privileged run has nothing to check here.
        let Ok(Some(_restored)) = maestro_test_directory::deny(temp_root(), 0o500) else {
            return;
        };
        forget_arguments();
        let world = started(directory).await;
        publish(&world, directory, &[answer("exported")]);
        submit(&world.mode, "/share");
        drive(&world).await.unwrap();
        assert_last_row_holds(
            &world,
            "Error: Failed to export session: ",
            "a refused directory",
        );
        assert_eq!(arguments().len(), 1, "no gist was started");
    }));
}

#[test]
fn share_spawn_failure_restores_editor_and_cleans_artifact() {
    const TEST: &str = "share_spawn_failure_restores_editor_and_cleans_artifact";
    drop(run(TEST, "child", Some((GH, "0700")), async |directory| {
        // The authentication answer succeeds and removes the program the gist needs.
        configure(&json!({"auth": {"exit": 0, "remove_self": true}}));
        let world = exportable(directory).await;
        let editor = Rc::clone(&world.mode.editor);
        submit(&world.mode, "/share");
        drive(&world).await.unwrap();
        assert_last_row_holds(&world, "Error: Failed to create gist: ", "a refused launch");
        assert!(!last_row_holds(&world, "Share URL: "));
        assert!(artifacts().is_empty(), "the artifact is gone");
        let restored = world.mode.editor_container.children().borrow().clone();
        assert!(std::ptr::addr_eq(
            Rc::as_ptr(&restored[0]),
            Rc::as_ptr(&editor)
        ));
        assert_loader_stopped(&world);
        assert_eq!(arguments(), [vec!["auth".to_owned(), "status".to_owned()]]);
    }));
}

#[test]
fn share_cleanup_failure_does_not_replace_result() {
    const TEST: &str = "share_cleanup_failure_does_not_replace_result";
    drop(run(TEST, "child", Some((GH, "0700")), async |directory| {
        configure(&json!({"auth": {"exit": 0}, "gist": {"frames": [
            {"stream": "chmod_artifact", "mode": "0500"},
            {"stream": "out", "bytes": hex("https://gist.github.com/alice/kept\n")}
        ]}}));
        let world = exportable(directory).await;
        submit(&world.mode, "/share");
        drive(&world).await.unwrap();
        assert_notice(
            &world,
            &ThemeColor::Dim,
            "Gist: https://gist.github.com/alice/kept",
        );
        assert!(!last_row_holds(&world, "Failed to"));
        assert_loader_stopped(&world);
        assert_eq!(world.mode.editor_container.children().borrow().len(), 1);
        // A privileged run removes the artifact despite the mode; either way the result
        // stands and the directory is left removable.
        for artifact in artifacts() {
            let mode = std::os::unix::fs::PermissionsExt::from_mode(0o700);
            fs::set_permissions(&artifact, mode).unwrap();
        }
    }));
}

#[test]
fn share_temp_artifacts_do_not_collide() {
    const TEST: &str = "share_temp_artifacts_do_not_collide";
    drop(run(TEST, "child", None, async |_directory| {
        let first = ShareArtifact::new().unwrap();
        let second = ShareArtifact::new().unwrap();
        assert_ne!(first.file, second.file);
        fs::write(&first.file, "first session").unwrap();
        fs::write(&second.file, "second session").unwrap();
        assert_eq!(fs::read_to_string(&first.file).unwrap(), "first session");
        assert_eq!(fs::read_to_string(&second.file).unwrap(), "second session");
        second.remove();
        assert!(first.file.exists(), "the other artifact is kept");
        assert!(!second.directory.exists());
        first.remove();
        assert!(!first.directory.exists(), "this operation removed its own");
        // An existing directory is refused, never reused or overwritten.
        let seeded = ShareArtifact::next_directory();
        fs::create_dir(&seeded).unwrap();
        fs::write(seeded.join("session.html"), "seeded").unwrap();
        assert!(
            ShareArtifact::new().is_err(),
            "an existing directory is refused"
        );
        assert_eq!(
            fs::read_to_string(seeded.join("session.html")).unwrap(),
            "seeded"
        );
        fs::remove_file(seeded.join("session.html")).unwrap();
        fs::remove_dir(&seeded).unwrap();
    }));
}

#[test]
fn share_capture_preserves_unicode_across_pipe_boundaries() {
    const TEST: &str = "share_capture_preserves_unicode_across_pipe_boundaries";
    drop(run(TEST, "child", Some((GH, "0700")), async |directory| {
        // Two-, three- and four-byte scalars split across native writes, an unsplit
        // control, and genuinely invalid bytes.
        let cases = [
            json!({"auth": {"exit": 0}, "gist": {"frames": [
                {"stream": "out", "bytes": hex_byte(0xc3)},
                {"stream": "sleep", "seconds": 0.05},
                {"stream": "out", "bytes": hex_byte(0xa9)},
                {"stream": "out", "bytes": hex("\n")}
            ]}}),
            json!({"auth": {"exit": 0}, "gist": {"frames": [
                {"stream": "out", "bytes": hex_byte(0xe2)},
                {"stream": "sleep", "seconds": 0.05},
                {"stream": "out", "bytes": hex_byte(0x82)},
                {"stream": "sleep", "seconds": 0.05},
                {"stream": "out", "bytes": hex_byte(0xac)},
                {"stream": "out", "bytes": hex("\n")}
            ]}}),
            json!({"auth": {"exit": 0}, "gist": {"frames": [
                {"stream": "out", "bytes": hex("gists/\u{1f980}\n")}
            ]}}),
            json!({"auth": {"exit": 0}, "gist": {"frames": [
                {"stream": "out", "bytes": hex_byte(0xff)},
                {"stream": "out", "bytes": hex("\n")}
            ]}}),
        ];
        for (settings, notice) in
            cases
                .into_iter()
                .zip(["Gist: é", "Gist: €", "Gist: gists/🦀", "Gist: \u{fffd}"])
        {
            configure(&settings);
            let world = exportable(directory).await;
            submit(&world.mode, "/share");
            drive(&world).await.unwrap();
            assert_notice(&world, &ThemeColor::Dim, notice);
        }
        // A failed gist keeps a split standard-error scalar too.
        configure(&json!({"auth": {"exit": 0}, "gist": {"exit": 1, "frames": [
            {"stream": "err", "bytes": hex_byte(0xc3)},
            {"stream": "sleep", "seconds": 0.05},
            {"stream": "err", "bytes": hex_byte(0xa9)}
        ]}}));
        let world = exportable(directory).await;
        submit(&world.mode, "/share");
        drive(&world).await.unwrap();
        assert_notice(
            &world,
            &ThemeColor::Error,
            "Error: Failed to create gist: é",
        );
    }));
}

#[test]
fn command_submission_preserves_editor_callback_order() {
    const TEST: &str = "command_submission_preserves_editor_callback_order";
    drop(run(TEST, "child", None, async |directory| {
        for command in ["/share", "/copy"] {
            let world = started(directory).await;
            let editor = Rc::clone(&world.mode.editor);
            let seen: Rc<RefCell<Vec<(String, String, String)>>> = Rc::default();
            let recorded = Rc::clone(&seen);
            let live = Rc::downgrade(&editor);
            editor.set_on_change(Some(Rc::new(move |text| {
                let current = live
                    .upgrade()
                    .map_or_else(String::new, |editor| editor.get_text());
                recorded
                    .borrow_mut()
                    .push(("change".to_owned(), text.to_owned(), current));
            })));
            let installed = editor.on_submit().unwrap();
            let recorded = Rc::clone(&seen);
            let live = Rc::downgrade(&editor);
            editor.set_on_submit(Some(Rc::new(move |text| {
                let current = live
                    .upgrade()
                    .map_or_else(String::new, |editor| editor.get_text());
                recorded
                    .borrow_mut()
                    .push(("submit".to_owned(), text.to_owned(), current));
                installed(text);
            })));
            world.terminal.send_input(command);
            for work in world.runtime.take_futures() {
                work.await.unwrap();
            }
            world.terminal.send_input("\x1b");
            world.terminal.send_input("\r");
            let events = seen.borrow().clone();
            let submitted = events.last().cloned().unwrap();
            assert_eq!(submitted.0, "submit", "{command}");
            assert_eq!(submitted.1, command, "{command}");
            assert_eq!(
                submitted.2, "",
                "the editor cleared before its submit callback"
            );
            let change = &events[events.len() - 2];
            assert_eq!(change.0, "change", "{command}");
            assert_eq!(change.1, "", "{command}");
            assert_eq!(change.2, "", "{command}");
            assert_eq!(world.runtime.futures(), 1, "{command} is admitted");
        }
    }));
}

#[test]
fn command_callbacks_release_containing_mode() {
    const TEST: &str = "command_callbacks_release_containing_mode";
    drop(run(TEST, "child", None, async |directory| {
        let World {
            mode,
            terminal,
            runtime,
            themes,
            host,
            loader,
        } = started(directory).await;
        let editor = Rc::clone(&mode.editor);
        let callback = editor.on_submit().unwrap();
        let retained = mode
            .show_share_loader(Rc::new(ShareArtifact::new().unwrap()))
            .unwrap();
        let weak = Rc::downgrade(&mode);
        drop(mode);
        assert!(
            weak.upgrade().is_none(),
            "the callback and loader retain nothing"
        );
        // The retained callback neither revives the mode nor admits work.
        callback("/share");
        assert!(weak.upgrade().is_none());
        assert_eq!(runtime.futures(), 0);
        retained.dispose();
        drop((terminal, runtime, themes, host, loader, editor, retained));
        // A second live mode admits its own operation independently.
        let world = started(directory).await;
        submit(&world.mode, "/copy");
        assert_eq!(world.runtime.futures(), 1);
    }));
}
