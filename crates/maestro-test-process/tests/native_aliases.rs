//! Native aliases receive backend operands without harness parsing or output.

use std::ffi::OsString;
#[cfg(unix)]
use std::io::{BufRead, BufReader};
use std::io::{Read, Write};
use std::net::TcpListener;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use maestro_test_directory::Scratch;
use maestro_test_process::credentials::{self, Scenario};
use maestro_test_process::install_executable;

/// The explicit artifact supplied by repository preparation, never a target-directory search.
fn artifact() -> PathBuf {
    std::env::var_os("MAESTRO_NATIVE_FIXTURE")
        .map(PathBuf::from)
        .expect("run just prepare-tests, then run these tests with cargo nextest")
}

/// One length-prefixed operating-system value reported by the native child.
fn field(input: &mut impl Read) -> Vec<u8> {
    let mut length = [0; 8];
    input.read_exact(&mut length).unwrap();
    let mut bytes = vec![0; usize::try_from(u64::from_le_bytes(length)).unwrap()];
    input.read_exact(&mut bytes).unwrap();
    bytes
}

/// Install one alias with its finite observation record.
fn alias(root: &Path, name: &str, port: u16, end: &str) -> PathBuf {
    let path = root.join(format!("{name}{}", std::env::consts::EXE_SUFFIX));
    install_executable(&artifact(), &path).unwrap();
    let mut record = path.as_os_str().to_owned();
    record.push(".fixture");
    std::fs::write(record, format!("observe\n{port}\n{end}\n")).unwrap();
    path
}

/// Literal backend operands, including the platform's native non-text representation.
fn operands() -> Vec<OsString> {
    let operands: Vec<OsString> = ["--", "host", "", "a b", "'\"", "$(touch sentinel);*", "é☃"]
        .map(OsString::from)
        .into();
    #[cfg(unix)]
    let operands: Vec<_> = operands
        .into_iter()
        .chain([std::os::unix::ffi::OsStringExt::from_vec(
            b"\xff\xfe!".to_vec(),
        )])
        .collect();
    #[cfg(windows)]
    let operands: Vec<_> = operands
        .into_iter()
        .chain(
            [<OsString as std::os::windows::ffi::OsStringExt>::from_wide(
                &[0xd800, 0x21],
            )],
        )
        .collect();
    operands
}

/// Native PATH execution preserves operands, process identity, environment and binary streams.
#[test]
fn path_aliases_keep_native_operands_pid_environment_and_raw_streams() {
    let root = Scratch::new().unwrap();
    let directory = root.path().join("space é");
    std::fs::create_dir(&directory).unwrap();
    let value = OsString::from("value ' \" $; é");
    #[cfg(unix)]
    let value: OsString =
        std::os::unix::ffi::OsStringExt::from_vec([value.as_encoded_bytes(), b"\xff"].concat());
    let operands = operands();
    for name in ["ssh", "rg", "fd"] {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        alias(
            root.path(),
            name,
            listener.local_addr().unwrap().port(),
            "7",
        );
        let mut child = Command::new(name)
            .args(&operands)
            .current_dir(&directory)
            .env("PATH", root.path())
            .env("MTP_VALUE", &value)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        let mut report = listener.accept().unwrap().0;
        assert_eq!(field(&mut report), child.id().to_string().as_bytes());
        let cwd = String::from_utf8(field(&mut report)).unwrap();
        assert_eq!(
            Path::new(&cwd).canonicalize().unwrap(),
            directory.canonicalize().unwrap()
        );
        assert_eq!(field(&mut report), value.as_encoded_bytes());
        for operand in &operands {
            assert_eq!(field(&mut report), operand.as_encoded_bytes());
        }
        assert_eq!(report.read(&mut [0]).unwrap(), 0);
        let mut stderr = child.stderr.take().unwrap();
        let mut error_bytes = [0; 6];
        stderr.read_exact(&mut error_bytes).unwrap();
        assert_eq!(&error_bytes, b"err\0\xfe!");
        let bytes = b"input\0\xff\xfe\n";
        child.stdin.take().unwrap().write_all(bytes).unwrap();
        let output = child.wait_with_output().unwrap();
        assert_eq!(output.status.code(), Some(7));
        assert_eq!(output.stdout, bytes);
        assert!(!directory.join("sentinel").exists());
    }
}

/// A copied preparation example reports its cache without exporting components or writing logs.
#[test]
fn author_cache_query_has_no_preparation_effects() {
    let root = Scratch::new().unwrap();
    let alias = root.path().join(format!(
        "prepare_author_component{}",
        std::env::consts::EXE_SUFFIX
    ));
    install_executable(&artifact(), &alias).unwrap();
    std::fs::write(
        root.path().join("author-record"),
        "tooling-author\n{\"scenario\":\"cached\",\"engine\":\"fixture\"}\n",
    )
    .unwrap();
    let cache = root.path().join("private cache");
    let environment = root.path().join("nextest-env");

    let output = Command::new(alias)
        .arg("--cache-directory")
        .current_dir(root.path())
        .env("MAESTRO_NATIVE_RECORD", root.path().join("author-record"))
        .env("XDG_CACHE_HOME", &cache)
        .env("NEXTEST_ENV", &environment)
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        serde_json::from_slice::<PathBuf>(&output.stdout).unwrap(),
        cache.join("wasmtime")
    );
    assert!(!environment.exists());
    assert!(!cache.exists());
    assert!(!root.path().join("seen").exists());
}

/// Invalid selectors and records fail before fixture or installation effects.
#[test]
fn invalid_native_requests_leave_the_effect_sentinel_absent() {
    let root = Scratch::new().unwrap();
    let sentinel = root.path().join("effect");
    for text in [
        "unknown\n1\n0\n",
        "observe\n0\n0\n",
        "observe\nno-port\n0\n",
        "observe\n1\n256\n",
        "observe\n1\n0\nextra\n",
    ] {
        let record = root.path().join("record");
        std::fs::write(&record, text).unwrap();
        let output = Command::new(artifact())
            .args(["--maestro-observe"])
            .arg(record)
            .env("MTP_EFFECT", &sentinel)
            .current_dir(root.path())
            .output()
            .unwrap();
        assert!(!output.status.success(), "accepted {text:?}");
        assert!(output.stdout.is_empty());
        assert!(!sentinel.exists());
    }
    for args in [
        vec!["unknown"],
        vec!["--maestro-install"],
        vec!["--maestro-install", "one", "two"],
    ] {
        let output = Command::new(artifact())
            .args(args)
            .env("MTP_EFFECT", &sentinel)
            .current_dir(root.path())
            .output()
            .unwrap();
        assert!(!output.status.success());
        assert!(!sentinel.exists());
    }
    assert!(!root.path().join("one").exists());
    assert!(install_executable(&root.path().join("missing"), &sentinel).is_err());
    assert!(install_executable(&artifact(), &root.path().join("missing/alias")).is_err());
    assert!(!sentinel.exists());
}

/// Malformed clipboard records fail before any byte reaches a stream.
#[test]
fn malformed_clipboard_records_fail_before_output() {
    let root = Scratch::new().unwrap();
    let record = root.path().join("record");
    let observe = |text: &str| {
        std::fs::write(&record, text).unwrap();
        Command::new(artifact())
            .arg("--maestro-observe")
            .arg(&record)
            .output()
            .unwrap()
    };
    let filled = observe("clipboard\n{\"scenario\":\"fill\",\"byte\":7,\"count\":3}");
    assert!(filled.status.success());
    assert_eq!(filled.stdout, [7, 7, 7]);
    for text in [
        "clipboard\n",
        "clipboard\n{\"scenario\":\"unknown\"}",
        "clipboard\n{\"scenario\":\"echo\",\"code\":256}",
        "clipboard\n{\"scenario\":\"echo\",\"code\":0,\"unknown\":true}",
        "clipboard\n{\"scenario\":\"echo\",\"code\":0}",
        "clipboard\n{\"scenario\":\"fill\",\"byte\":7,\"count\":-1}",
        "clipboard\nnot json",
    ] {
        let output = observe(text);
        assert!(!output.status.success(), "accepted {text:?}");
        assert!(output.stdout.is_empty(), "wrote bytes for {text:?}");
    }
}

/// Malformed search records fail before operand logs or injection sentinels are written.
#[cfg(unix)]
#[test]
fn malformed_search_records_have_no_partial_effects() {
    use serde_json::json;
    let root = Scratch::new().unwrap();
    let sentinel = root.path().join("effect");
    let response = json!({"scenario":"response", "args":sentinel,
        "stdout":[], "stderr":[], "code":0, "signal":null});
    let mut invalid = vec![
        json!({"scenario":"marker", "path":sentinel,
            "text":"executed", "copy_input":false, "unknown":true}),
        json!({"scenario":"chosen-shell", "unknown":true}),
    ];
    for (field, value) in [
        ("stdout", json!([256])),
        ("code", json!(256)),
        ("signal", json!("unknown")),
        ("scenario", json!("unknown")),
    ] {
        let mut record = response.clone();
        record[field] = value;
        invalid.push(record);
    }
    for record in invalid {
        let path = root.path().join("record");
        std::fs::write(&path, format!("search\n{record}")).unwrap();
        let output = Command::new(artifact())
            .arg("--maestro-observe")
            .arg(path)
            .output()
            .unwrap();
        assert!(!output.status.success(), "accepted {record}");
        assert!(output.stdout.is_empty());
        assert!(!sentinel.exists(), "partial effects for {record}");
    }
}

/// Malformed command records fail before operand logs or appends are written.
#[cfg(unix)]
#[test]
fn malformed_command_records_have_no_partial_effects() {
    use serde_json::json;
    let root = Scratch::new().unwrap();
    let sentinel = root.path().join("effect");
    let output = json!({
        "scenario": "output",
        "stdout": [],
        "stderr": [],
        "code": 0,
        "append": {"path": sentinel, "cwd": false, "bytes": []}
    });
    let mut invalid = vec![
        json!({"scenario": "unknown"}),
        json!({"scenario": "package", "unknown": true}),
        json!({"scenario": "labelled", "unknown": true}),
    ];
    for (field, value) in [
        ("code", json!(256)),
        ("signal", json!("SIGKILL")),
        ("stdout", json!([256])),
    ] {
        let mut record = output.clone();
        record[field] = value;
        invalid.push(record);
    }
    let mut unknown = output;
    unknown["unknown"] = json!(true);
    invalid.push(unknown);
    for record in invalid {
        let path = root.path().join("record");
        std::fs::write(&path, format!("command\n{record}")).unwrap();
        // The operands and both variables admit an unmodified run, so only a
        // rejected record keeps the append, the package log and both streams empty.
        let run = Command::new(artifact())
            .arg("--maestro-observe")
            .arg(path)
            .args(["--quiet", "root"])
            .env("MAESTRO_COMMAND_LOG", &sentinel)
            .env("MAESTRO_NPM_ROOT", root.path())
            .output()
            .unwrap();
        assert!(!run.status.success(), "accepted {record}");
        assert!(run.stdout.is_empty());
        assert!(!sentinel.exists(), "partial effects for {record}");
    }
}

/// Concurrent installations leave launchable copies with independently mutable ages.
#[test]
fn parallel_installs_launch_independent_copies_while_siblings_are_live() {
    let root = Scratch::new().unwrap();
    let source = artifact();
    let source_time = std::fs::metadata(&source).unwrap().modified().unwrap();
    let barrier = std::sync::Barrier::new(4);
    std::thread::scope(|scope| {
        for number in 0..4 {
            let root = root.path();
            let barrier = &barrier;
            scope.spawn(move || {
                barrier.wait();
                let listener = TcpListener::bind("127.0.0.1:0").unwrap();
                let path = alias(
                    root,
                    &format!("copy-{number}"),
                    listener.local_addr().unwrap().port(),
                    "0",
                );
                let mut child = Command::new(&path)
                    .stdin(Stdio::piped())
                    .stdout(Stdio::piped())
                    .stderr(Stdio::piped())
                    .spawn()
                    .unwrap();
                let mut report = listener.accept().unwrap().0;
                assert_eq!(field(&mut report), child.id().to_string().as_bytes());
                std::io::copy(&mut report, &mut std::io::sink()).unwrap();
                drop(report);
                let second = root.join(format!("other-{number}{}", std::env::consts::EXE_SUFFIX));
                install_executable(&artifact(), &second).unwrap();
                child.stdin.take().unwrap().write_all(b"\0\xff").unwrap();
                let output = child.wait_with_output().unwrap();
                assert!(output.status.success());
                assert_eq!(output.stdout, b"\0\xff");
                assert_eq!(
                    std::fs::read(&second).unwrap(),
                    std::fs::read(artifact()).unwrap()
                );
                let old = std::time::UNIX_EPOCH + std::time::Duration::from_secs(1);
                std::fs::File::options()
                    .write(true)
                    .open(&second)
                    .unwrap()
                    .set_modified(old)
                    .unwrap();
                assert_eq!(std::fs::metadata(&second).unwrap().modified().unwrap(), old);
            });
        }
    });
    assert_eq!(
        std::fs::metadata(source).unwrap().modified().unwrap(),
        source_time
    );
}

/// The native child handle signals the same process that reported readiness.
#[cfg(unix)]
#[test]
fn native_handle_stops_the_observed_child_by_signal() {
    use std::os::unix::process::ExitStatusExt;
    let root = Scratch::new().unwrap();
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let path = alias(
        root.path(),
        "signal",
        listener.local_addr().unwrap().port(),
        "0",
    );
    let mut child = Command::new(path)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let mut report = listener.accept().unwrap().0;
    assert_eq!(field(&mut report), child.id().to_string().as_bytes());
    std::io::copy(&mut report, &mut std::io::sink()).unwrap();
    child.kill().unwrap();
    assert_eq!(child.wait_with_output().unwrap().status.signal(), Some(9));
}

/// Forwarded metadata replaces the Cargo fixture without changing its native identity.
#[cfg(unix)]
#[test]
fn cargo_metadata_forwarding_keeps_the_native_child_pid() {
    let root = Scratch::new().unwrap();
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let observer = alias(
        root.path(),
        "metadata-observer",
        listener.local_addr().unwrap().port(),
        "0",
    );
    let cargo = root.path().join("cargo");
    install_executable(&artifact(), &cargo).unwrap();
    std::fs::write(
        root.path().join("cargo.fixture"),
        format!("tooling-cargo\n{}\nunused-template\n", observer.display()),
    )
    .unwrap();
    let child = Command::new(cargo)
        .arg("metadata")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let pid = child.id();
    let mut report = listener.accept().unwrap().0;
    let reported_pid = field(&mut report);
    std::io::copy(&mut report, &mut std::io::sink()).unwrap();
    let output = child.wait_with_output().unwrap();
    assert!(output.status.success());
    assert_eq!(output.stderr, b"err\0\xfe!");
    assert_eq!(reported_pid, pid.to_string().as_bytes());
}

/// RPC family inputs fail before protocol bytes or launch markers can escape.
#[cfg(unix)]
#[test]
fn rpc_peer_inputs_fail_before_protocol_output_or_marker_creation() {
    let root = Scratch::new().unwrap();
    let record = root.path().join("record");
    let marker = root.path().join("marker");
    for (text, answers, records, replies) in [
        ("rpc-peer\nunknown\n", "{}", "[]", "[]"),
        ("rpc-peer\n", "{}", "[]", "[]"),
        ("rpc-peer\nLaunch\nextra\n", "{}", "[]", "[]"),
        ("rpc-peer\nServe\n", "[]", "[]", "[]"),
        ("rpc-peer\nBurst\n", "{}", "{}", "[]"),
        ("rpc-peer\nReplay\n", "{}", "[]", "[null]"),
        ("rpc-peer\nTick\n", "{}", "[]", "[]"),
    ] {
        std::fs::write(&record, text).unwrap();
        let output = Command::new(artifact())
            .arg("--maestro-observe")
            .arg(&record)
            .env("PEER_MARKER", &marker)
            .env("PEER_ANSWERS", answers)
            .env("PEER_RECORDS", records)
            .env("PEER_REPLIES", replies)
            .env("PEER_PORT", "0")
            .output()
            .unwrap();
        assert!(!output.status.success(), "accepted {text:?}");
        assert!(
            output.stdout.is_empty(),
            "emitted protocol output for {text:?}"
        );
        assert!(!marker.exists(), "wrote a launch marker for {text:?}");
    }
}

/// Idle peers distinguish TERM before EOF from plain EOF without waiting for a signal.
#[cfg(unix)]
#[test]
fn rpc_idle_peer_observes_term_and_eof_independently() {
    use std::io::{BufRead, BufReader};
    let root = Scratch::new().unwrap();
    let path = root.path().join("idle");
    install_executable(&artifact(), &path).unwrap();
    std::fs::write(
        root.path().join("idle.fixture"),
        "rpc-peer\nIdleInterrupt\n",
    )
    .unwrap();
    for terminate in [false, true] {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let marker = root.path().join(format!("marker-{terminate}"));
        let mut child = Command::new(&path)
            .env(
                "PEER_PORT",
                listener.local_addr().unwrap().port().to_string(),
            )
            .env("PEER_MARKER", &marker)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        let mut report = BufReader::new(listener.accept().unwrap().0);
        let mut line = String::new();
        if terminate {
            rustix::process::kill_process(
                rustix::process::Pid::from_raw(i32::try_from(child.id()).unwrap()).unwrap(),
                rustix::process::Signal::TERM,
            )
            .unwrap();
        }
        drop(child.stdin.take());
        if terminate {
            report.read_line(&mut line).unwrap();
            assert_eq!(line, "terminated\n");
            assert_eq!(std::fs::read_to_string(&marker).unwrap(), "terminated");
            line.clear();
        }
        report.read_line(&mut line).unwrap();
        assert_eq!(line, "input closed\n");
        line.clear();
        assert_eq!(report.read_line(&mut line).unwrap(), 0);
        assert!(child.wait_with_output().unwrap().status.success());
        assert_eq!(marker.exists(), terminate);
    }
}

/// A Git stand-in reports the operands, directory and status the product supplied.
#[cfg(unix)]
#[test]
fn git_stand_in_reports_operands_directory_and_status() {
    let root = Scratch::new().unwrap();
    let directory = root.path().join("repo");
    std::fs::create_dir(&directory).unwrap();
    let alias = root.path().join("git");
    maestro_test_process::git_command::install(
        &alias,
        maestro_test_process::git_command::Scenario::Reported,
    )
    .unwrap();
    let output = Command::new(&alias)
        .args([
            "--no-optional-locks",
            "symbolic-ref",
            "--quiet",
            "--short",
            "HEAD",
        ])
        .current_dir(&directory)
        .env("GIT_EXIT", "1")
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(1));
    let reported = String::from_utf8(output.stdout).unwrap();
    let mut lines = reported.lines();
    assert_eq!(
        lines.next(),
        Some("--no-optional-locks symbolic-ref --quiet --short HEAD")
    );
    assert_eq!(
        Path::new(lines.next().unwrap()).canonicalize().unwrap(),
        directory.canonicalize().unwrap()
    );
    assert_eq!(lines.next(), None);
}

/// The retaining descendant announces the direct child's identifier, not its own.
#[cfg(unix)]
#[test]
fn git_stand_in_reports_the_direct_child_identifier() {
    let root = Scratch::new().unwrap();
    let alias = root.path().join("git");
    let holder = root.path().join("git-holder");
    maestro_test_process::git_command::install(
        &alias,
        maestro_test_process::git_command::Scenario::Exiting,
    )
    .unwrap();
    maestro_test_process::git_command::install(
        &holder,
        maestro_test_process::git_command::Scenario::Retaining,
    )
    .unwrap();
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    std::fs::write(
        root.path().join("holder-port"),
        listener.local_addr().unwrap().port().to_string(),
    )
    .unwrap();
    let mut child = Command::new(&alias)
        .current_dir(root.path())
        .env("GIT_HOLDER", &holder)
        .stdout(Stdio::null())
        .spawn()
        .unwrap();
    let direct = child.id().to_string();
    let mut control = listener.accept().unwrap().0;
    let mut announced = String::new();
    BufReader::new(&control).read_line(&mut announced).unwrap();
    control.write_all(b"!").unwrap();
    assert!(child.wait().unwrap().success());
    assert_eq!(announced.trim(), direct);
}

/// Git stand-in records fail before the alias writes output or reads its inputs.
#[cfg(unix)]
#[test]
fn git_stand_in_refuses_incomplete_records_before_output() {
    let root = Scratch::new().unwrap();
    let record = root.path().join("record");
    for text in ["git\n", "git\nObserved\n", "git\nRetaining\nextra\n"] {
        std::fs::write(&record, text).unwrap();
        let output = Command::new(artifact())
            .arg("--maestro-observe")
            .arg(&record)
            .env_remove("GIT_LOG")
            .output()
            .unwrap();
        assert!(!output.status.success(), "accepted {text:?}");
        assert!(output.stdout.is_empty(), "wrote output for {text:?}");
    }
}

/// An installed alias of the credential scenarios, with its record already written.
fn credential_alias(root: &Path, name: &str, scenario: Scenario) -> PathBuf {
    let path = root.join(format!("{name}{}", std::env::consts::EXE_SUFFIX));
    credentials::install(&path, scenario).unwrap();
    path
}

/// The runs an installed credential process counted beside itself.
fn counted_runs(root: &Path) -> usize {
    std::fs::read_to_string(root.join("process-count"))
        .unwrap_or_default()
        .lines()
        .count()
}

/// A complete credential document names the controlled key, secret and token.
fn assert_credential_document(variant: &str, stdout: &[u8]) {
    let document: serde_json::Value = serde_json::from_slice(stdout).unwrap();
    assert_eq!(document["Version"], 1, "{variant}: version");
    assert_eq!(document["AccessKeyId"], "process-key", "{variant}: key");
    assert_eq!(
        document["SessionToken"], "process-token",
        "{variant}: token"
    );
    assert_eq!(
        document["SecretAccessKey"], "controlled-secret",
        "{variant}: secret"
    );
    assert!(document.get("Expiration").is_none(), "{variant}: expiry");
}

/// Each unusable document carries exactly one defect the reader must refuse.
fn assert_unusable_documents(process: &Path) {
    let answer = |variant: &str| Command::new(process).arg(variant).output().unwrap();
    assert_eq!(answer("invalid_json").stdout, b"broken", "unusable text");
    let wrong: serde_json::Value = serde_json::from_slice(&answer("wrong_version").stdout).unwrap();
    assert_eq!(wrong["Version"], 2, "refused version");
    let missing: serde_json::Value = serde_json::from_slice(&answer("missing_key").stdout).unwrap();
    assert!(missing.get("SecretAccessKey").is_none(), "missing secret");
    let expired: serde_json::Value = serde_json::from_slice(&answer("expired").stdout).unwrap();
    assert_eq!(
        expired["Expiration"], "2000-01-01T00:00:00Z",
        "expired document"
    );
}

/// An AWS credential process answers every variant and counts each run.
#[test]
fn credential_process_answers_variants_and_counts_runs() {
    let root = Scratch::new().unwrap();
    let process = credential_alias(root.path(), "process", Scenario::AwsProcess);
    for (variant, status) in [("once", 7), ("once", 0), ("ok", 0), ("fail", 7)] {
        let output = Command::new(&process).arg(variant).output().unwrap();
        assert_eq!(output.status.code(), Some(status), "{variant}: status");
        assert!(output.stderr.is_empty(), "{variant}: diagnostics");
        if status == 0 {
            assert_credential_document(variant, &output.stdout);
        } else {
            assert!(output.stdout.is_empty(), "{variant}: failing output");
        }
    }
    assert_unusable_documents(&process);
    assert_eq!(counted_runs(root.path()), 8, "counted runs");
}

/// A Google executable source answers one success document and takes no operand.
#[test]
fn google_executable_source_answers_one_success_document() {
    let root = Scratch::new().unwrap();
    let source = credential_alias(root.path(), "source", Scenario::GoogleExecutable);
    let output = Command::new(&source).output().unwrap();
    assert!(output.status.success(), "{output:?}");
    let document: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(document["version"], 1);
    assert_eq!(document["success"], true);
    assert_eq!(document["id_token"], "subject-exec");
    assert_eq!(document["expiration_time"], 4_102_444_800_u64);
    assert!(!root.path().join("process-count").exists(), "counted a run");
    assert!(
        !Command::new(&source)
            .arg("extra")
            .output()
            .unwrap()
            .status
            .success()
    );
}

/// Records and variants outside the compiled set fail before any output or count.
#[test]
fn credential_records_and_variants_fail_before_output() {
    let root = Scratch::new().unwrap();
    for text in [
        "credentials\nunknown\n",
        "credentials\n",
        "credentials\nAwsProcess\nextra\n",
    ] {
        let record = root.path().join("record");
        std::fs::write(&record, text).unwrap();
        let output = Command::new(artifact())
            .arg("--maestro-observe")
            .arg(&record)
            .output()
            .unwrap();
        assert!(!output.status.success(), "accepted {text:?}");
        assert!(output.stdout.is_empty(), "answered for {text:?}");
    }
    let process = credential_alias(root.path(), "process", Scenario::AwsProcess);
    for operands in [vec!["unknown"], vec!["ok", "ok"]] {
        let output = Command::new(&process).args(&operands).output().unwrap();
        assert!(!output.status.success(), "accepted {operands:?}");
        assert!(output.stdout.is_empty(), "answered for {operands:?}");
    }
    assert_eq!(counted_runs(root.path()), 0, "counted a rejected run");
}
