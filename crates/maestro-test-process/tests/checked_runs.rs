//! The checked test process through its public interface.
#![cfg(test)]

use std::collections::BTreeMap;
use std::io::{Read, Write};
use std::path::Path;
use std::process::{Command, ExitStatus, Output, Stdio};

use maestro_test_directory::Scratch;
use maestro_test_process::{TestProcess, run_alone, test_name};

/// The harness bytes that open a one-test run.
const START: &[u8] = b"\nrunning 1 test\n";
/// Argument spelled with a space and shell metacharacters that must reach the child literally.
const LITERAL: &str = "literal & | ; $HOME %PATH% ^ < >";

/// Probes in nested modules share a leaf name and differ only in their module path.
mod left {
    /// Probe reached as `checked_runs::left::selected_child`.
    #[test]
    #[ignore = "runs only inside a controlled child"]
    pub(super) fn selected_child() {
        crate::behave("left");
    }
}

/// Second same-leaf probe in a sibling module.
mod right {
    /// Probe reached as `checked_runs::right::selected_child`.
    #[test]
    #[ignore = "runs only inside a controlled child"]
    pub(super) fn selected_child() {
        crate::behave("right");
    }
}

/// Full identity of the left probe.
fn left_name() -> String {
    test_name(left::selected_child).unwrap()
}

/// Full identity of the right probe.
fn right_name() -> String {
    test_name(right::selected_child).unwrap()
}

/// Full identity of the standalone probe.
fn bare_name() -> String {
    test_name(crate::bare_child).unwrap()
}

/// A builder for an ignored probe that prints its output and performs `behavior`.
fn probe(test: &str, behavior: &str) -> TestProcess {
    TestProcess::named(test)
        .unwrap()
        .ignored()
        .nocapture()
        .env("MTP_BEHAVIOR", behavior)
}

/// A builder for an ignored probe with no environment, directory, descriptor or shell setting.
fn plain(test: &str) -> TestProcess {
    TestProcess::named(test).unwrap().ignored().nocapture()
}

/// Lossy text of captured bytes.
fn text(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes).into_owned()
}

/// Whether `haystack` holds `needle` as a contiguous run.
fn contains(haystack: &[u8], needle: &[u8]) -> bool {
    haystack
        .windows(needle.len())
        .any(|window| window == needle)
}

/// A real successful exit status.
fn success() -> ExitStatus {
    ExitStatus::default()
}

/// Output a wrapper could return, with the given bytes and a zero exit.
fn fake(stdout: &[u8], stderr: &[u8]) -> Output {
    Output {
        status: success(),
        stdout: stdout.to_vec(),
        stderr: stderr.to_vec(),
    }
}

/// Run this test's own function again in a raw child that carries `MTP_OUTER`.
fn outer(leaf: &str) -> Command {
    let mut command = Command::new(std::env::current_exe().unwrap());
    command
        .args(["--exact", &format!("checked_runs::{leaf}"), "--nocapture"])
        .env("MTP_OUTER", "1");
    command
}

/// Whether this process is the raw outer child of the running test.
fn is_outer() -> bool {
    std::env::var_os("MTP_OUTER").is_some()
}

/// The `name=hex` lines a probe printed, keyed by variable.
fn described(stdout: &[u8]) -> BTreeMap<String, String> {
    text(stdout)
        .lines()
        .filter_map(|line| line.split_once('='))
        .map(|(name, value)| (name.to_owned(), value.to_owned()))
        .collect()
}

/// Lower-case hexadecimal spelling of `value`.
fn hex(value: &str) -> String {
    crate::hex(value.as_bytes())
}

#[test]
fn current_identity_keeps_module_and_unit_paths() {
    const LEAF: &str = "current_identity_keeps_module_and_unit_paths";
    let full = format!("checked_runs::{LEAF}");
    assert_eq!(TestProcess::current(&full).unwrap().name(), full);
    assert_eq!(TestProcess::current(LEAF).unwrap().name(), full);
    assert_eq!(left_name(), "checked_runs::left::selected_child");
    assert_eq!(bare_name(), "bare_child");
    for (name, leaf) in [
        (left_name(), "selected_child"),
        (right_name(), "selected_child"),
        (bare_name(), "bare_child"),
    ] {
        let output = probe(&name, "name").output().unwrap();
        assert!(
            text(&output.stdout).contains(&format!("identity={name}\n")),
            "{name}"
        );
        assert!(
            !name.starts_with("integration") && !name.contains("tests::"),
            "{name}"
        );
        assert!(name.ends_with(leaf));
    }
}

#[test]
fn current_identity_rejects_mismatch_and_unnamed_threads() {
    assert!(TestProcess::current("some_other_test").is_err());
    assert!(TestProcess::current("identity_rejects_mismatch_and_unnamed_threads").is_err());
    let unnamed = std::thread::spawn(|| TestProcess::current("x").unwrap_err())
        .join()
        .unwrap();
    assert!(
        unnamed.to_string().contains("not a named test"),
        "{unnamed}"
    );
}

#[test]
fn sibling_names_are_exact_not_suffixes() {
    let scratch = Scratch::new().unwrap();
    let marker = scratch.path().join("marker");
    for (name, label) in [
        (left_name(), "left"),
        (right_name(), "right"),
        (bare_name(), "bare"),
    ] {
        let _ = std::fs::remove_file(&marker);
        probe(&name, "marker")
            .env("MTP_MARKER", &marker)
            .output()
            .unwrap();
        assert_eq!(
            std::fs::read_to_string(&marker).unwrap(),
            format!("{label}\n")
        );
    }
}

#[test]
fn missing_selection_is_not_a_checked_success() {
    let raw = Command::new(std::env::current_exe().unwrap())
        .args(["--exact", "checked_runs::stale_name"])
        .output()
        .unwrap();
    assert!(raw.status.success() && text(&raw.stdout).contains("running 0 tests"));
    for name in [
        "checked_runs::stale_name",
        "selected_child",
        "left::selected_child",
    ] {
        let error = TestProcess::named(name)
            .unwrap()
            .ignored()
            .output()
            .unwrap_err();
        assert!(error.to_string().starts_with(name), "{error}");
    }
    assert!(TestProcess::named("").is_err());
}

#[test]
fn ignored_selection_requires_explicit_execution() {
    let scratch = Scratch::new().unwrap();
    let marker = scratch.path().join("marker");
    let name = left_name();
    let raw = Command::new(std::env::current_exe().unwrap())
        .args(["--exact", &name])
        .env("MTP_BEHAVIOR", "marker")
        .env("MTP_MARKER", &marker)
        .output()
        .unwrap();
    assert!(raw.stdout.starts_with(START) && text(&raw.stdout).contains("ignored"));
    let ordinary = TestProcess::named(&name).unwrap().nocapture();
    assert!(
        ordinary
            .env("MTP_BEHAVIOR", "marker")
            .env("MTP_MARKER", &marker)
            .output()
            .is_err()
    );
    assert!(!marker.exists());
    probe(&name, "marker")
        .env("MTP_MARKER", &marker)
        .output()
        .unwrap();
    assert_eq!(std::fs::read_to_string(&marker).unwrap(), "left\n");
    let this = TestProcess::current("ignored_selection_requires_explicit_execution").unwrap();
    assert!(this.ignored().output().is_err());
}

#[test]
fn skip_matches_exact_identity_and_preserves_literal_operands() {
    let name = left_name();
    let scratch = Scratch::new().unwrap();
    let marker = scratch.path().join("marker");
    let excluded = probe(&name, "marker")
        .env("MTP_MARKER", &marker)
        .skip(&name)
        .output();
    assert!(excluded.is_err() && !marker.exists());
    let operands = [
        "selected_child",
        "argument with spaces",
        LITERAL,
        "selected_child",
    ];
    let output = operands
        .iter()
        .fold(probe(&name, "argv"), |process, operand| {
            process.skip(*operand)
        })
        .output()
        .unwrap();
    let mut expected = vec!["--exact", &name, "--nocapture", "--ignored"];
    expected.extend(operands.iter().flat_map(|operand| ["--skip", operand]));
    let printed: Vec<_> = text(&output.stdout)
        .lines()
        .filter_map(|line| line.strip_prefix("arg=").map(str::to_owned))
        .collect();
    let expected: Vec<_> = expected.iter().map(|arg| format!("{arg:?}")).collect();
    assert_eq!(printed, expected);
}

#[test]
fn captured_output_keeps_raw_bytes_and_unsuccessful_status() {
    let name = left_name();
    for end in ["complete", "panic", "exit7", "signal"] {
        let output = probe(&name, "bytes").env("MTP_END", end).output().unwrap();
        assert!(output.stdout.starts_with(START), "{end}");
        assert!(contains(&output.stdout, b"out\0\xff"), "{end}");
        assert!(contains(&output.stderr, b"err\0\xfe"), "{end}");
        match end {
            "complete" => assert!(output.status.success()),
            "panic" => assert_eq!(output.status.code(), Some(101)),
            "exit7" => assert_eq!(
                (output.status.code(), output.stderr.as_slice()),
                (Some(7), &b"err\0\xfe"[..])
            ),
            _ => assert_signalled(&output),
        }
    }
}

/// Require death by a signal, which leaves no exit code.
fn assert_signalled(output: &Output) {
    #[cfg(unix)]
    assert_eq!(
        std::os::unix::process::ExitStatusExt::signal(&output.status),
        Some(6)
    );
    #[cfg(not(unix))]
    assert!(!output.status.success());
    assert_eq!(output.stderr, b"err\0\xfe");
}

#[test]
fn intentional_exit_needs_no_success_epilogue() {
    let output = probe(&left_name(), "exit").output().unwrap();
    assert!(output.status.success());
    assert!(!text(&output.stdout).contains("test result"));
}

#[test]
fn start_prefix_rejects_wrong_count_short_and_embedded_headers() {
    let this = || {
        TestProcess::current("start_prefix_rejects_wrong_count_short_and_embedded_headers").unwrap()
    };
    let wrong: [&[u8]; 7] = [
        b"\nrunning 0 tests\n",
        b"\nrunning 2 tests\n",
        b"\nrunning 1 test",
        b"",
        b"junk\nrunning 1 test\n",
        b"running 1 test\n",
        b"\n\nrunning 1 test\n",
    ];
    for bytes in wrong {
        let result = this().output_with(|_| Ok::<_, std::io::Error>(fake(bytes, b"")));
        assert!(result.is_err(), "{bytes:?}");
    }
    let valid = [START, b"anything\0\xff"].concat();
    let output = this()
        .output_with(|_| Ok::<_, std::io::Error>(fake(&valid, b"e")))
        .unwrap();
    assert_eq!((output.stdout, output.stderr), (valid, b"e".to_vec()));
}

/// Read until the child's `READY` line, returning everything read.
fn read_until_ready(stdout: &mut impl Read) -> Vec<u8> {
    let mut seen = Vec::new();
    while !seen.ends_with(b"READY\n") {
        let mut byte = [0];
        assert_eq!(
            stdout.read(&mut byte).unwrap(),
            1,
            "stream ended before READY"
        );
        seen.push(byte[0]);
    }
    seen
}

#[test]
fn live_reader_replays_prefix_without_consuming_product() {
    let mut child = probe(&left_name(), "live")
        .stdin(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let mut stdout = child.stdout.take().unwrap();
    let mut prologue = [0; 16];
    stdout.read_exact(&mut prologue).unwrap();
    assert_eq!(prologue, START);
    assert!(read_until_ready(&mut stdout).ends_with(b"READY\n"));
    child
        .stdin
        .as_mut()
        .unwrap()
        .write_all(b"continue\n")
        .unwrap();
    let mut rest = String::new();
    stdout.read_to_string(&mut rest).unwrap();
    assert!(rest.contains("after\n"), "{rest}");
    assert!(child.wait().unwrap().success());
}

#[test]
fn dropping_live_stdout_closes_the_only_read_end() {
    if run_alone("dropping_live_stdout_closes_the_only_read_end").unwrap() {
        return;
    }
    let mut child = probe(&left_name(), "pipe")
        .stdin(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let mut stdout = child.stdout.take().unwrap();
    read_until_ready(&mut stdout);
    drop(stdout);
    child
        .stdin
        .as_mut()
        .unwrap()
        .write_all(b"continue\n")
        .unwrap();
    let output = child.wait_with_output().unwrap();
    assert!(
        text(&output.stderr).contains("broken-pipe"),
        "{}",
        text(&output.stderr)
    );
}

#[cfg(unix)]
#[test]
fn shell_setup_runs_when_the_child_replaces_path() {
    let scratch = Scratch::new().unwrap();
    let marker = scratch.path().join("setup-ran");
    let setup = format!("echo ran > '{}'", marker.display());
    plain(&left_name())
        .env("PATH", scratch.path())
        .shell_setup("/bin/bash", setup)
        .output()
        .unwrap();
    assert_eq!(std::fs::read_to_string(&marker).unwrap(), "ran\n");
}

#[cfg(unix)]
#[test]
fn failed_live_start_closes_and_reaps_direct_child() {
    let scratch = Scratch::new().unwrap();
    let pid_file = scratch.path().join("pid");
    let setup = format!(
        "echo $$ > '{}'; printf 'invalid startup\\n'; read -r line",
        pid_file.display()
    );
    let result = probe(&left_name(), "live")
        .shell_setup("/bin/bash", setup)
        .stdin(Stdio::piped())
        .spawn();
    let error = result.unwrap_err().to_string();
    assert!(
        error.contains("did not start exactly") && error.contains("stdout:\ni"),
        "{error}"
    );
    assert_reaped(&pid_file);
}

/// Require that the process whose id `pid_file` holds no longer exists, so it was reaped.
#[cfg(unix)]
fn assert_reaped(pid_file: &Path) {
    let pid = std::fs::read_to_string(pid_file).unwrap();
    let alive = Command::new("kill")
        .args(["-0", pid.trim()])
        .stderr(Stdio::null())
        .status()
        .unwrap()
        .success();
    assert!(!alive, "child {pid} was not reaped");
}

#[test]
fn status_replays_streams_separately_with_inherited_stdin() {
    if is_outer() {
        let status = probe(&left_name(), "echo").status().unwrap();
        assert!(status.success());
        return;
    }
    let mut child = outer("status_replays_streams_separately_with_inherited_stdin")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child
        .stdin
        .take()
        .unwrap()
        .write_all(b"typed input")
        .unwrap();
    let output = child.wait_with_output().unwrap();
    assert!(output.status.success(), "{}", text(&output.stderr));
    assert!(contains(&output.stdout, b"stdin:typed input"));
    assert!(contains(&output.stderr, b"err\0\xff"));
    assert!(!contains(&output.stderr, b"stdin:") && !contains(&output.stdout, b"err\0\xff"));
}

#[cfg(unix)]
#[test]
fn file_stdout_remains_a_file_and_preserves_shared_cursor() {
    use std::io::Seek;
    let scratch = Scratch::new().unwrap();
    let (input, err_path, out_path) = (
        scratch.path().join("in"),
        scratch.path().join("err"),
        scratch.path().join("out"),
    );
    std::fs::write(&input, "from file").unwrap();
    let out = std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(true)
        .open(&out_path)
        .unwrap();
    let mut held = out.try_clone().unwrap();
    let status = probe(&left_name(), "file")
        .stdin(std::fs::File::open(&input).unwrap())
        .stderr(std::fs::File::create(&err_path).unwrap())
        .status_with_stdout(out)
        .unwrap();
    assert!(status.success());
    let written = std::fs::read(&out_path).unwrap();
    assert!(written.starts_with(START));
    let shown = text(&written);
    for line in [
        "stdin-file=true",
        "stdout-file=true",
        "stderr-file=true",
        "input=\"from file\"",
    ] {
        assert!(shown.contains(line), "{shown}");
    }
    assert_eq!(held.stream_position().unwrap(), written.len() as u64);
    assert_eq!(std::fs::read_to_string(&err_path).unwrap(), "diagnostic\n");
}

#[test]
fn file_stdout_refuses_existing_bytes_or_offset_before_launch() {
    use std::io::{Seek, SeekFrom};
    let scratch = Scratch::new().unwrap();
    for (index, contents, append, offset) in [
        (0, b"existing bytes\n".as_slice(), true, 0),
        (1, START, false, 0),
        (2, b"".as_slice(), false, 1),
    ] {
        let path = scratch.path().join(format!("out-{index}"));
        let marker = scratch.path().join(format!("marker-{index}"));
        std::fs::write(&path, contents).unwrap();
        let mut file = std::fs::OpenOptions::new()
            .read(true)
            .write(!append)
            .append(append)
            .open(&path)
            .unwrap();
        file.seek(SeekFrom::Start(offset)).unwrap();
        let error = probe(&left_name(), "marker")
            .env("MTP_MARKER", &marker)
            .status_with_stdout(file)
            .unwrap_err();
        assert!(
            error
                .to_string()
                .contains("must be empty with its cursor at zero"),
            "{error}"
        );
        assert!(!marker.exists(), "child started for rejected file {index}");
        assert_eq!(std::fs::read(path).unwrap(), contents);
    }
}

#[test]
fn file_stdout_accepts_an_empty_append_file() {
    let scratch = Scratch::new().unwrap();
    let path = scratch.path().join("out");
    let file = std::fs::OpenOptions::new()
        .create_new(true)
        .read(true)
        .append(true)
        .open(&path)
        .unwrap();
    let status = probe(&left_name(), "idle")
        .status_with_stdout(file)
        .unwrap();
    assert!(status.success());
    assert!(std::fs::read(path).unwrap().starts_with(START));
}

#[cfg(unix)]
#[test]
fn shell_setup_preserves_configuration_and_proves_exec() {
    let scratch = Scratch::new().unwrap();
    let marker = scratch.path().join("marker");
    let setup = "echo setup >> \"$MTP_MARKER\"";
    let output = probe(&left_name(), "marker")
        .shell_setup("/bin/bash", setup)
        .env("MTP_MARKER", &marker)
        .output()
        .unwrap();
    assert!(output.status.success());
    assert_eq!(std::fs::read_to_string(&marker).unwrap(), "setup\nleft\n");
    let directory = scratch.path().join("work");
    std::fs::create_dir(&directory).unwrap();
    let output = probe(&left_name(), "cwd")
        .current_dir(&directory)
        .shell_setup("/bin/bash", "true")
        .env("MTP_VARS", "SHELL_EDIT")
        .env("SHELL_EDIT", "late")
        .output()
        .unwrap();
    let shown = described(&output.stdout);
    assert_eq!(shown["SHELL_EDIT"], hex("late"));
    assert_eq!(
        Path::new(&shown["cwd"]).canonicalize().unwrap(),
        directory.canonicalize().unwrap()
    );
    for setup in ["exit 0", "echo setup-diagnostic >&2; exit 3"] {
        let error = probe(&left_name(), "marker")
            .shell_setup("/bin/bash", setup)
            .output()
            .unwrap_err();
        assert!(error.to_string().contains(&left_name()), "{error}");
        assert_eq!(
            error.to_string().contains("setup-diagnostic"),
            setup.contains("diagnostic")
        );
    }
}

#[test]
fn adapter_output_is_checked_without_replacing_the_invoker() {
    let name = left_name();
    let calls = std::cell::RefCell::new(Vec::new());
    let output = plain(&name)
        .skip(LITERAL)
        .output_with(|arguments| {
            calls.borrow_mut().push(arguments.to_vec());
            Command::new(std::env::current_exe().unwrap())
                .args(arguments)
                .env("MTP_BEHAVIOR", "bytes")
                .env("MTP_END", "exit7")
                .output()
        })
        .unwrap();
    let calls = calls.into_inner();
    assert_eq!(
        calls,
        [[
            "--exact",
            &name,
            "--nocapture",
            "--ignored",
            "--skip",
            LITERAL
        ]
        .map(String::from)]
    );
    assert_eq!(output.status.code(), Some(7));
    assert!(contains(&output.stdout, b"out\0\xff"));
    let silent = plain(&name).output_with(|_| Ok::<_, std::io::Error>(fake(b"hello", b"")));
    assert!(silent.is_err());
}

#[test]
fn adapter_route_does_not_discard_os_configuration() {
    let name = left_name();
    let configured: Vec<(&str, TestProcess)> = vec![
        ("env", plain(&name).env("A", "b")),
        ("envs", plain(&name).envs([("A", "b")])),
        ("env_remove", plain(&name).env_remove("A")),
        ("env_clear", plain(&name).env_clear()),
        ("current_dir", plain(&name).current_dir(".")),
        ("stdin", plain(&name).stdin(Stdio::null())),
        ("stdout", plain(&name).stdout(Stdio::piped())),
        ("stderr", plain(&name).stderr(Stdio::null())),
        #[cfg(unix)]
        ("shell_setup", plain(&name).shell_setup("/bin/bash", "true")),
    ];
    for (setting, process) in configured {
        let invoked = std::cell::Cell::new(false);
        let result = process.output_with(|_| {
            invoked.set(true);
            Ok::<_, std::io::Error>(fake(START, b""))
        });
        assert!(result.is_err() && !invoked.get(), "{setting}");
    }
}

/// Variables the environment probe reports.
const VARS: &str = "LLVM_PROFILE_FILE,MTP_AMBIENT,MTP_SET,MTP_OTHER,PATH";

/// Report `process`'s view of [`VARS`].
fn view(process: TestProcess) -> BTreeMap<String, String> {
    let process = process.env("MTP_BEHAVIOR", "env").env("MTP_VARS", VARS);
    described(&process.output().unwrap().stdout)
}

/// Check each ordered edit against the ambient values the raw outer child supplied.
fn check_edits(profile: Option<&str>) {
    let name = left_name();
    let before: Vec<_> = std::env::vars_os().collect();
    let inherited = view(plain(&name));
    assert_eq!(inherited["MTP_AMBIENT"], hex("outer"));
    assert_eq!(inherited["PATH"], hex("outer-path"));
    let kept = profile.map_or_else(|| "-".to_owned(), hex);
    assert_eq!(inherited["LLVM_PROFILE_FILE"], kept);
    let cleared = view(plain(&name).env_clear());
    assert_eq!(cleared["PATH"], "-");
    assert_eq!(
        (
            cleared["MTP_AMBIENT"].as_str(),
            cleared["LLVM_PROFILE_FILE"].as_str()
        ),
        ("-", kept.as_str())
    );
    let overridden = view(plain(&name).env_clear().env("LLVM_PROFILE_FILE", "x%p"));
    assert_eq!(overridden["LLVM_PROFILE_FILE"], hex("x%p"));
    let removed = view(plain(&name).env_clear().env_remove("LLVM_PROFILE_FILE"));
    assert_eq!(removed["LLVM_PROFILE_FILE"], "-");
    let reset = view(plain(&name).env_remove("MTP_SET").env("MTP_SET", "2"));
    assert_eq!(reset["MTP_SET"], hex("2"));
    let unset = view(plain(&name).env("MTP_SET", "1").env_remove("MTP_SET"));
    assert_eq!(unset["MTP_SET"], "-");
    let late = view(
        plain(&name)
            .env("MTP_SET", "1")
            .env_clear()
            .env("MTP_OTHER", "3"),
    );
    assert_eq!(
        (late["MTP_SET"].as_str(), late["MTP_OTHER"].as_str()),
        ("-", hex("3").as_str())
    );
    assert_eq!(before, std::env::vars_os().collect::<Vec<_>>());
}

#[test]
fn environment_edits_keep_order_profile_bytes_and_parent_state() {
    if is_outer() {
        check_edits(std::env::var("LLVM_PROFILE_FILE").ok().as_deref());
        return;
    }
    let scratch = Scratch::new().unwrap();
    let profile = format!("{}/prof-%p-%m.profraw", scratch.path().display());
    for ambient in [Some(profile.as_str()), None] {
        let mut command = outer("environment_edits_keep_order_profile_bytes_and_parent_state");
        command
            .env("MTP_AMBIENT", "outer")
            .env("PATH", "outer-path");
        match ambient {
            Some(profile) => command.env("LLVM_PROFILE_FILE", profile),
            None => command.env_remove("LLVM_PROFILE_FILE"),
        };
        let output = command.output().unwrap();
        assert!(
            output.status.success(),
            "{}{}",
            text(&output.stdout),
            text(&output.stderr)
        );
    }
}

#[test]
fn startup_failures_name_selection_and_keep_diagnostics() {
    let this =
        || TestProcess::current("startup_failures_name_selection_and_keep_diagnostics").unwrap();
    let name = this().name().to_owned();
    let missing = Path::new("/definitely/not/a/directory");
    for error in [
        this().current_dir(missing).output().unwrap_err(),
        this().current_dir(missing).spawn().unwrap_err(),
    ] {
        assert!(error.to_string().starts_with(&name), "{error}");
        assert!(std::error::Error::source(&error).is_some());
    }
    let bad = this()
        .output_with(|_| Ok::<_, std::io::Error>(fake(b"diag-out", b"diag-err")))
        .unwrap_err()
        .to_string();
    assert!(
        bad.starts_with(&name) && bad.contains("diag-out") && bad.contains("diag-err"),
        "{bad}"
    );
    let failure = this()
        .output_with(|_| Err::<Output, _>(std::io::Error::other("invoker failed")))
        .unwrap_err();
    assert!(failure.to_string().contains("invoker failed"));
    #[cfg(unix)]
    {
        let scratch = Scratch::new().unwrap();
        let pid_file = scratch.path().join("pid");
        let setup = format!(
            "echo $$ > '{}'; printf 'invalid startup\\n'; exit 0",
            pid_file.display()
        );
        let error = this().shell_setup("/bin/bash", setup).output().unwrap_err();
        assert!(
            error.to_string().contains("did not start exactly"),
            "{error}"
        );
        assert_reaped(&pid_file);
    }
}

#[test]
fn alone_runs_the_test_once_in_a_fresh_one_test_process() {
    let name = "alone_runs_the_test_once_in_a_fresh_one_test_process";
    assert!(run_alone("some_other_test").is_err());
    if let Some(path) = std::env::var_os("MTP_ALONE_COUNT") {
        let mut log = std::fs::OpenOptions::new().append(true).open(path).unwrap();
        writeln!(log, "{}", std::process::id()).unwrap();
        if run_alone(name).unwrap() {
            return;
        }
        let arguments: Vec<_> = std::env::args().skip(1).collect();
        assert_eq!(
            arguments,
            ["--exact", &format!("checked_runs::{name}"), "--nocapture"]
        );
        return;
    }
    let scratch = Scratch::new().unwrap();
    let path = scratch.path().join("launches");
    std::fs::write(&path, "").unwrap();
    let output = TestProcess::current(name)
        .unwrap()
        .env("MTP_ALONE_COUNT", &path)
        .output()
        .unwrap();
    assert!(output.status.success(), "{}", text(&output.stdout));
    let log = std::fs::read_to_string(path).unwrap();
    let pids: Vec<_> = log.lines().collect();
    assert_eq!(pids.len(), 2, "one parent and one isolated child: {log}");
    assert_ne!(pids[0], pids[1]);
    println!(
        "ISOLATION-LAUNCH-COUNT parent={} child={} body-entries={}",
        pids[0],
        pids[1],
        pids.len()
    );
}

#[test]
fn alone_reports_the_failing_body_with_its_streams() {
    if std::env::var_os("MTP_ALONE_FAILS").is_none() {
        return;
    }
    if run_alone("alone_reports_the_failing_body_with_its_streams").unwrap() {
        return;
    }
    panic!("the isolated body failed");
}

#[test]
fn alone_passing_body_writes_a_diagnostic() {
    if std::env::var_os("MTP_ALONE_DIAGNOSES").is_none() {
        return;
    }
    if run_alone("alone_passing_body_writes_a_diagnostic").unwrap() {
        return;
    }
    println!("ALONE-BODY-STDOUT");
    eprintln!("ALONE-BODY-DIAGNOSTIC");
}

#[test]
fn alone_success_forwards_only_the_body_diagnostics() {
    let output = TestProcess::named("checked_runs::alone_passing_body_writes_a_diagnostic")
        .unwrap()
        .nocapture()
        .env("MTP_ALONE_DIAGNOSES", "1")
        .output()
        .unwrap();
    assert!(output.status.success(), "{}", text(&output.stdout));
    let stderr = text(&output.stderr);
    assert!(
        stderr.contains("ALONE-BODY-DIAGNOSTIC")
            && !stderr.contains("ALONE-BODY-STDOUT")
            && !stderr.contains("running 1 test"),
        "{stderr}"
    );
}

#[test]
fn alone_failure_names_the_test_and_keeps_the_body_output() {
    let name = "checked_runs::alone_reports_the_failing_body_with_its_streams";
    let output = TestProcess::named(name)
        .unwrap()
        .env("MTP_ALONE_FAILS", "1")
        .output()
        .unwrap();
    assert!(!output.status.success());
    let reported = text(&output.stdout);
    assert!(
        reported.contains("the isolated run failed")
            && reported.contains("the isolated body failed"),
        "{reported}"
    );
}

#[test]
fn harness_modes_keep_existing_output_presentation() {
    let name = left_name();
    let tree = |process: TestProcess| process.env("MTP_BEHAVIOR", "argv").output().unwrap();
    let pretty = TestProcess::named(&name)
        .unwrap()
        .ignored()
        .env("MTP_BEHAVIOR", "print");
    let pretty = text(&pretty.output().unwrap().stdout);
    assert!(
        pretty.contains(&format!("test {name} ... ok")) && !pretty.contains("printed left"),
        "{pretty}"
    );
    let loud = text(&probe(&name, "print").output().unwrap().stdout);
    assert!(loud.contains("printed left"), "{loud}");
    let terse = TestProcess::named(&name)
        .unwrap()
        .ignored()
        .quiet()
        .single_threaded();
    let terse = text(&terse.output().unwrap().stdout);
    assert!(
        !terse.contains("... ok") && terse.contains("test result: ok. 1 passed;"),
        "{terse}"
    );
    let flags = tree(probe(&name, "argv").quiet().single_threaded());
    let args: Vec<_> = text(&flags.stdout)
        .lines()
        .filter_map(|l| l.strip_prefix("arg=").map(str::to_owned))
        .collect();
    let quoted = |text: &str| format!("{text:?}");
    let expected = [
        "--exact",
        &name,
        "--nocapture",
        "--test-threads=1",
        "--quiet",
        "--ignored",
    ]
    .map(quoted);
    assert_eq!(args, expected);
}

/// Values with no text spelling in common: empty, spaced, non-ASCII and, on Unix, non-UTF-8.
fn native_values() -> Vec<std::ffi::OsString> {
    #[cfg_attr(not(unix), allow(unused_mut))]
    let mut values: Vec<std::ffi::OsString> = ["", "a b", "é☃"].map(Into::into).into();
    #[cfg(unix)]
    values.push(
        <std::ffi::OsStr as std::os::unix::ffi::OsStrExt>::from_bytes(b"\xff\xfe!").to_owned(),
    );
    values
}

/// Run an environment probe over the capture, live and file routes and return each printout.
fn route_outputs(configure: impl Fn(TestProcess) -> TestProcess) -> Vec<String> {
    let name = left_name();
    let scratch = Scratch::new().unwrap();
    let captured = configure(plain(&name).env("MTP_BEHAVIOR", "cwd"))
        .output()
        .unwrap();
    let live = configure(plain(&name).env("MTP_BEHAVIOR", "cwd"))
        .spawn()
        .unwrap();
    let live = live.wait_with_output().unwrap();
    let file = std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(true)
        .open(scratch.path().join("out"))
        .unwrap();
    let process = configure(plain(&name).env("MTP_BEHAVIOR", "cwd"));
    process.status_with_stdout(file).unwrap();
    let from_file = std::fs::read(scratch.path().join("out")).unwrap();
    #[cfg_attr(not(unix), allow(unused_mut))]
    let mut outputs = vec![text(&captured.stdout), text(&live.stdout), text(&from_file)];
    #[cfg(unix)]
    outputs.push(text(
        &configure(
            plain(&name)
                .env("MTP_BEHAVIOR", "cwd")
                .shell_setup("/bin/bash", "true"),
        )
        .output()
        .unwrap()
        .stdout,
    ));
    outputs
}

#[test]
fn native_os_values_and_working_directory_reach_each_transport() {
    for value in native_values() {
        let expected = format!("MTP_VALUE={}\n", crate::hex(&crate::raw_bytes(&value)));
        for printed in route_outputs(|process| {
            process
                .env("MTP_VARS", "MTP_VALUE")
                .env("MTP_VALUE", &value)
        }) {
            assert!(
                printed.contains(&expected),
                "{expected:?} not in {printed:?}"
            );
        }
    }
    let scratch = Scratch::new().unwrap();
    let directory = scratch.path().join("cwd");
    std::fs::create_dir(&directory).unwrap();
    let canonical = directory.canonicalize().unwrap();
    let here = std::env::current_dir().unwrap();
    for printed in route_outputs(|process| process.current_dir(&directory)) {
        let line = printed
            .lines()
            .find_map(|line| line.strip_prefix("cwd="))
            .unwrap();
        assert_eq!(Path::new(line).canonicalize().unwrap(), canonical);
    }
    for printed in
        route_outputs(|process| process.env("MTP_VARS", "MTP_VALUE").env("MTP_VALUE", "1"))
    {
        let line = printed
            .lines()
            .find_map(|line| line.strip_prefix("cwd="))
            .unwrap();
        assert_eq!(Path::new(line), here);
    }
}

#[test]
fn checked_child_waits_keep_exit_and_raw_streams() {
    let name = left_name();
    let mut exiting = probe(&name, "exit").spawn().unwrap();
    assert!(exiting.wait().unwrap().success());
    let failing = probe(&name, "bytes").env("MTP_END", "exit7");
    let mut failing = failing.stdin(Stdio::piped()).spawn().unwrap();
    assert_eq!(failing.wait().unwrap().code(), Some(7));
    let mut child = probe(&name, "bytes")
        .stdin(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let mut partial = [0; 5];
    child
        .stdout
        .as_mut()
        .unwrap()
        .read_exact(&mut partial)
        .unwrap();
    assert_eq!(partial, START[..5]);
    let output = child.wait_with_output().unwrap();
    assert!(
        output.stdout.starts_with(&START[5..]),
        "{:?}",
        text(&output.stdout)
    );
    assert!(contains(&output.stdout, b"out\0\xff") && contains(&output.stderr, b"err\0\xfe"));
    assert!(output.status.success());
}

#[test]
fn argument_invoker_keeps_exact_selection_and_caller_result() {
    let selected =
        TestProcess::current("argument_invoker_keeps_exact_selection_and_caller_result").unwrap();
    let name = selected.name().to_owned();
    let arguments = selected
        .nocapture()
        .invoke_with(|arguments| Ok::<_, std::io::Error>(arguments.to_vec()))
        .unwrap();
    assert_eq!(arguments, ["--exact", &name, "--nocapture"]);
    let mut invoked = false;
    let result = TestProcess::current("argument_invoker_keeps_exact_selection_and_caller_result")
        .unwrap()
        .env("UNUSED", "value")
        .invoke_with(|_| {
            invoked = true;
            Ok::<_, std::io::Error>(())
        });
    assert!(result.is_err());
    assert!(!invoked);
}

/// Variable that marks a shell-setup child and names the parent's disposable layout.
#[cfg(unix)]
const SHELL_ROOT: &str = "MTP_SHELL_ROOT";

/// Run this test once in a child whose `command` the parent prepared over `scratch`, and hand
/// the child's own `body` the disposable root. The parent asserts the child passed.
#[cfg(unix)]
fn in_shell_child(command: impl FnOnce(&Scratch) -> Command, body: impl FnOnce(&Path)) {
    let Some(root) = std::env::var_os(SHELL_ROOT).map(std::path::PathBuf::from) else {
        let scratch = Scratch::new().unwrap();
        let output = command(&scratch)
            .env(SHELL_ROOT, scratch.path())
            .output()
            .unwrap();
        assert!(output.status.success(), "{}", text(&output.stderr));
        return;
    };
    body(&root);
}

#[cfg(unix)]
#[test]
fn shell_setup_refuses_a_relative_interpreter() {
    let scratch = Scratch::new().unwrap();
    let marker = scratch.path().join("marker");
    let error = plain(&left_name())
        .env("MTP_BEHAVIOR", "marker")
        .env("MTP_MARKER", &marker)
        .shell_setup("bash", format!("echo ran > '{}'", marker.display()))
        .output()
        .unwrap_err();
    assert!(error.to_string().contains("not absolute"), "{error}");
    assert!(
        !marker.exists(),
        "the setup ran before the path was refused"
    );
}

#[cfg(unix)]
#[test]
fn shell_setup_reports_a_supplied_interpreter_that_cannot_run() {
    let scratch = Scratch::new().unwrap();
    let program = scratch.path().join("bash");
    std::fs::write(&program, "not a program\n").unwrap();
    let error = plain(&left_name())
        .shell_setup(&program, "true")
        .output()
        .unwrap_err();
    assert!(error.to_string().starts_with(&left_name()), "{error}");
    assert!(std::error::Error::source(&error).is_some());
}

#[cfg(unix)]
#[test]
fn shell_setup_uses_the_supplied_interpreter_when_the_parent_path_is_absent() {
    let leaf = "shell_setup_uses_the_supplied_interpreter_when_the_parent_path_is_absent";
    in_shell_child(
        |_| {
            let mut command = outer(leaf);
            command.env_remove("PATH");
            command
        },
        |root| {
            let empty = root.join("empty-path");
            std::fs::create_dir(&empty).unwrap();
            let marker = root.join("marker");
            plain(&left_name())
                .env("MTP_BEHAVIOR", "marker")
                .env("MTP_MARKER", &marker)
                .env("PATH", &empty)
                .shell_setup("/bin/bash", "echo setup >> \"$MTP_MARKER\"")
                .output()
                .unwrap();
            assert_eq!(std::fs::read_to_string(&marker).unwrap(), "setup\nleft\n");
        },
    );
}
