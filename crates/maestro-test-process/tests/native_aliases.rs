//! Native aliases receive backend operands without harness parsing or output.

use std::ffi::OsString;
use std::io::{Read, Write};
use std::net::TcpListener;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use maestro_test_directory::Scratch;
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
