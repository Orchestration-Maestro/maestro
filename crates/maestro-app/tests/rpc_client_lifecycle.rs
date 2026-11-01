//! Launching, observing and stopping the RPC client's child.
#![cfg(unix)]
use crate::rpc_peer::{Peer, SERVE, events, hold_clock, next_of};
use crate::support::rpc_connection::poll_once;
use crate::support::session_harness::{on_paused_task_set, on_task_set};
use maestro_app::{RpcClient, RpcClientError, RpcClientOptions};
use maestro_process::child_process::process_exists;
use serde_json::{Value, json};
use std::{
    io::{BufRead, Read},
    path::Path,
    pin::pin,
    task::Poll,
    time::Duration,
};

/// Reports the arguments, directory and environment, then waits for its input to end.
const REPORT: &str = r"
open(os.environ['PEER_MARKER'], 'a').write('started\n')
emit({'type': 'peer_ready', 'argv': sys.argv[1:], 'cwd': os.getcwd(),
      'path': os.environ.get('PATH'), 'home': os.environ.get('HOME')})
sys.stdin.read()
";

/// Changes one launch option.
type Configure = fn(&mut RpcClientOptions);

/// Starts `client`, returns its first `peer_ready` event and stops it.
async fn ready(client: &RpcClient) -> Value {
    let mut events = events(client);
    client.start().await.unwrap();
    let ready = next_of(&mut events, "peer_ready").await;
    client.stop().await.unwrap();
    ready
}

#[test]
fn client_arguments_keep_literal_order_and_child_context() {
    let peer = Peer::new();
    let marker = peer.scratch.join("marker");
    let here = std::env::current_dir().unwrap();
    let cases: [(Configure, &[&str]); 8] = [
        (|_| {}, &[]),
        (|o| o.provider = Some(String::new()), &[]),
        (|o| o.model = Some(String::new()), &[]),
        (
            |o| o.provider = Some("provider".into()),
            &["--provider", "provider"],
        ),
        (|o| o.model = Some("model".into()), &["--model", "model"]),
        (
            |o| {
                o.provider = Some("provider".into());
                o.model = Some("model".into());
                o.args = vec!["space arg".into(), ";literal".into(), "$HOME".into()];
            },
            &[
                "--provider",
                "provider",
                "--model",
                "model",
                "space arg",
                ";literal",
                "$HOME",
            ],
        ),
        (|o| o.args = Vec::new(), &[]),
        (
            |o| o.args = vec!["--model".into(), "late".into()],
            &["--model", "late"],
        ),
    ];
    on_task_set(|_| async {
        for (index, (configure, extra)) in cases.into_iter().enumerate() {
            let mut options = peer.options(REPORT);
            options.env.insert("PEER_MARKER".into(), marker.clone());
            configure(&mut options);
            let client = RpcClient::new(options);
            let started = std::fs::read_to_string(&marker).unwrap_or_default();
            assert_eq!(
                started.lines().count(),
                index,
                "construction launches nothing"
            );
            let ready = ready(&client).await;
            let mut argv = vec!["--mode", "rpc"];
            argv.extend_from_slice(extra);
            assert_eq!(ready["argv"], json!(argv));
            assert_eq!(ready["cwd"], json!(here.to_str().unwrap()));
            assert_eq!(ready["path"], json!(std::env::var("PATH").unwrap()));
        }
        child_context(&peer, &marker).await;
    });
}

/// A selected directory and an environment overlay reach the child, beside the inherited
/// environment.
async fn child_context(peer: &Peer, marker: &str) {
    let mut options = peer.options(REPORT);
    options.env.insert("PEER_MARKER".into(), marker.to_owned());
    options.env.insert("HOME".into(), "override".into());
    options.cwd = Some(peer.scratch.join(""));
    let ready = ready(&RpcClient::new(options)).await;
    assert_eq!(ready["argv"], json!(["--mode", "rpc"]));
    assert_eq!(ready["cwd"], json!(peer.scratch.0.path().to_str().unwrap()));
    assert_eq!(ready["home"], json!("override"));
    assert_eq!(ready["path"], json!(std::env::var("PATH").unwrap()));
}

#[test]
fn relative_entry_uses_the_selected_child_directory() {
    let peer = Peer::new();
    let dist = peer.scratch.join("dist");
    std::fs::create_dir(&dist).unwrap();
    crate::native::install_executable(Path::new(&peer.entry), &Path::new(&dist).join("maestro"));
    let here = std::env::current_dir().unwrap();
    let up = "../".repeat(here.components().count() - 1);
    let from_here = format!("{up}{}", peer.entry.trim_start_matches('/'));
    let launch = |cli_path: Option<&str>, cwd: Option<String>| {
        let mut options =
            peer.options("emit({'type': 'peer_ready', 'entry': sys.argv[0]})\nsys.stdin.read()");
        options.cli_path = cli_path.map(str::to_owned);
        options.cwd = cwd;
        RpcClient::new(options)
    };
    on_task_set(|_| async {
        let scratch = Some(peer.scratch.join(""));
        for (cli_path, cwd, entry) in [
            (None, scratch.clone(), dist.clone() + "/maestro"),
            (Some("peer"), scratch.clone(), peer.entry.clone()),
            (
                Some("./dist/maestro"),
                scratch.clone(),
                dist.clone() + "/maestro",
            ),
            (Some(from_here.as_str()), None, peer.entry.clone()),
            (
                Some(peer.entry.as_str()),
                Some(here.to_string_lossy().into_owned()),
                peer.entry.clone(),
            ),
        ] {
            let ready = ready(&launch(cli_path, cwd)).await;
            let launched = std::fs::canonicalize(ready["entry"].as_str().unwrap()).unwrap();
            assert_eq!(launched, std::fs::canonicalize(&entry).unwrap());
        }
        for (cli_path, cwd) in [
            (None, None),
            (Some("python3"), scratch.clone()),
            (Some(peer.entry.as_str()), Some(peer.scratch.join("absent"))),
        ] {
            let error = launch(cli_path, cwd).start().await.unwrap_err();
            let RpcClientError::Io(error) = error else {
                panic!("a native launch failure, not {error}");
            };
            assert_eq!(error.kind(), std::io::ErrorKind::NotFound);
        }
    });
}

#[test]
fn startup_reports_duplicate_and_early_exit() {
    let peer = Peer::new();
    on_task_set(|_| async {
        let client = RpcClient::new(peer.options(SERVE));
        let error = client.abort().await.unwrap_err();
        assert!(matches!(error, RpcClientError::NotStarted));
        assert_eq!(error.to_string(), "Client not started");
        client.stop().await.unwrap();
        client.stop().await.unwrap();
        client.start().await.unwrap();
        let error = client.start().await.unwrap_err();
        assert!(matches!(error, RpcClientError::AlreadyStarted));
        assert_eq!(error.to_string(), "Client already started");
        client.stop().await.unwrap();
        client.stop().await.unwrap();

        let failing =
            RpcClient::new(peer.options("sys.stderr.write('boot failed\\n')\nsys.exit(7)"));
        for runs in 1..=2 {
            let error = failing.start().await.unwrap_err();
            let stderr = "boot failed\n".repeat(runs);
            let message = format!("Agent process exited immediately with code 7. Stderr: {stderr}");
            assert_eq!(
                error.to_string(),
                message,
                "the diagnostic text outlives each child"
            );
            assert!(matches!(
                &error,
                RpcClientError::ProcessExit { code: Some(7), .. }
            ));
            assert!(matches!(
                failing.start().await,
                Err(RpcClientError::AlreadyStarted)
            ));
            failing.stop().await.unwrap();
            failing.stop().await.unwrap();
        }
        assert_eq!(failing.get_stderr(), "boot failed\nboot failed\n");

        let killed = RpcClient::new(peer.options("os.kill(os.getpid(), signal.SIGKILL)"));
        let error = killed.start().await.unwrap_err();
        assert!(matches!(
            &error,
            RpcClientError::ProcessExit { code: None, .. }
        ));
        assert_eq!(
            error.to_string(),
            "Agent process exited immediately with code null. Stderr: "
        );
        killed.stop().await.unwrap();
    });
}

#[test]
fn startup_observes_the_hundred_millisecond_boundary() {
    let peer = Peer::new();
    let script = "sys.stderr.write('booting\\n'); sys.stderr.flush()\nemit({'type': 'peer_ready'})\nsys.stdin.read()";
    on_paused_task_set(|_| async {
        let client = RpcClient::new(peer.options(script));
        let (release, hold) = hold_clock();
        let mut events = events(&client);
        let mut start = pin!(client.start());
        assert!(poll_once(start.as_mut()).is_pending());
        next_of(&mut events, "peer_ready").await;
        while client.get_stderr().is_empty() {
            tokio::task::yield_now().await;
        }
        assert_eq!(client.get_stderr(), "booting\n");
        tokio::time::advance(Duration::from_millis(99)).await;
        assert!(poll_once(start.as_mut()).is_pending());
        tokio::time::advance(Duration::from_millis(1)).await;
        assert!(matches!(poll_once(start.as_mut()), Poll::Ready(Ok(()))));
        release.send(()).unwrap();
        hold.await.unwrap();
        client.stop().await.unwrap();
    });
}

/// More diagnostic text than a pipe holds, so an unread standard error blocks its writer.
const FILL: usize = 1 << 20;

/// Runs a client whose child writes more diagnostic text than this process's unread
/// standard error holds, then reports the retained text and waits for the parent.
fn forwarding_child() {
    let peer = Peer::new();
    let script = format!(
        r"
signal.signal(signal.SIGTERM, lambda *_: (os.write(2, b'terminating\xc3'), os.write(2, b'\xa9\n')))
os.write(2, b'boot\xff\n')
sys.stderr.buffer.write(b'x' * {FILL}); sys.stderr.flush()
emit({{'type': 'peer_ready'}})
for line in commands(): respond(json.loads(line))
"
    );
    on_task_set(|_| async {
        let client = RpcClient::new(peer.options(&script));
        let mut events = events(&client);
        client.start().await.unwrap();
        next_of(&mut events, "peer_ready").await;
        client.abort().await.unwrap();
        client.stop().await.unwrap();
        let retained = client.get_stderr().replace(&"x".repeat(FILL), "<fill>");
        println!("PROBE {}", json!(retained));
    });
    // The parent lets this process end once it has read the forwarded text.
    let _ = std::io::stdin().read_line(&mut String::new());
}

#[test]
fn stderr_is_forwarded_and_retained_during_start_and_stop() {
    if maestro_test_process::selection().is_child() {
        forwarding_child();
        return;
    }
    let mut child = maestro_test_process::TestProcess::current(
        "stderr_is_forwarded_and_retained_during_start_and_stop",
    )
    .unwrap()
    .nocapture()
    .stdin(std::process::Stdio::piped())
    .stderr(std::process::Stdio::piped())
    .case("*")
    .spawn()
    .unwrap();
    let mut stdout = std::io::BufReader::new(child.stdout.take().unwrap());
    let mut stderr = child.stderr.take().unwrap();
    // The stop ends once the forwarded text is written, so it is read meanwhile.
    let reading = std::thread::spawn(move || {
        let mut seen = Vec::new();
        while !seen.ends_with(b"terminating\xc3\xa9\n") {
            let mut chunk = [0; 8192];
            let read = stderr.read(&mut chunk).unwrap();
            assert!(read > 0, "the forwarded text arrives before the child ends");
            seen.extend_from_slice(&chunk[..read]);
        }
        seen
    });
    let probe = loop {
        let mut line = String::new();
        assert!(
            stdout.read_line(&mut line).unwrap() > 0,
            "the child reports"
        );
        if let Some(probe) = line.strip_prefix("PROBE ") {
            break probe.trim_end().to_owned();
        }
    };
    assert_eq!(
        probe,
        json!("boot\u{fffd}\n<fill>terminating\u{e9}\n").to_string()
    );
    let forwarded = [
        &b"boot\xff\n"[..],
        &b"x".repeat(FILL),
        b"terminating\xc3\xa9\n",
    ]
    .concat();
    let seen = reading.join().unwrap();
    assert!(seen.ends_with(&forwarded));
    drop(child.stdin.take());
    assert!(child.wait().unwrap().success());
}

/// Runs a client whose child writes more diagnostic text than this process's unread
/// standard error holds, then stops it and reports, once the child is reaped, whether the
/// stop still waits for that text to be written.
fn delivering_child() {
    let peer = Peer::new();
    let script = format!(
        r"
sys.stderr.buffer.write(b'x' * {FILL} + b'end\n'); sys.stderr.flush()
emit({{'type': 'peer_ready', 'pid': os.getpid()}})
while True: time.sleep(1)
"
    );
    on_task_set(|_| async {
        let client = RpcClient::new(peer.options(&script));
        let mut events = events(&client);
        client.start().await.unwrap();
        let ready = next_of(&mut events, "peer_ready").await;
        let pid = u32::try_from(ready["pid"].as_u64().unwrap()).unwrap();
        while !client.get_stderr().ends_with("end\n") {
            tokio::task::yield_now().await;
        }
        let mut stop = pin!(client.stop());
        assert!(poll_once(stop.as_mut()).is_pending());
        while process_exists(pid) {
            tokio::task::yield_now().await;
        }
        if poll_once(stop.as_mut()).is_ready() {
            println!("VERDICT stopped");
        } else {
            println!("VERDICT waiting");
            stop.await.unwrap();
        }
    });
}

#[test]
fn stop_ends_after_every_captured_diagnostic_byte_is_written() {
    if maestro_test_process::selection().is_child() {
        delivering_child();
        return;
    }
    let mut child = maestro_test_process::TestProcess::current(
        "stop_ends_after_every_captured_diagnostic_byte_is_written",
    )
    .unwrap()
    .nocapture()
    .stderr(std::process::Stdio::piped())
    .case("*")
    .spawn()
    .unwrap();
    let mut stdout = std::io::BufReader::new(child.stdout.take().unwrap());
    let verdict = loop {
        let mut line = String::new();
        assert!(
            stdout.read_line(&mut line).unwrap() > 0,
            "the child reports"
        );
        if let Some(verdict) = line.strip_prefix("VERDICT ") {
            break verdict.trim_end().to_owned();
        }
    };
    assert_eq!(
        verdict, "waiting",
        "the stop waits until the captured text is written"
    );
    let mut forwarded = Vec::new();
    child
        .stderr
        .take()
        .unwrap()
        .read_to_end(&mut forwarded)
        .unwrap();
    let expected = [&b"x".repeat(FILL)[..], b"end\n"].concat();
    assert!(
        forwarded
            .windows(expected.len())
            .any(|window| window == expected)
    );
    assert!(child.wait().unwrap().success());
}

#[test]
fn stderr_keeps_a_character_split_across_reads() {
    let peer = Peer::new();
    let script = r"
os.write(2, b'a\xc3')
emit({'type': 'peer_ready'})
for line in commands():
    os.write(2, b'\xa9\n')
    respond(json.loads(line))
";
    on_task_set(|_| async {
        let client = RpcClient::new(peer.options(script));
        let mut events = events(&client);
        client.start().await.unwrap();
        next_of(&mut events, "peer_ready").await;
        while client.get_stderr().is_empty() {
            tokio::task::yield_now().await;
        }
        assert_eq!(
            client.get_stderr(),
            "a",
            "a partial character waits for its end"
        );
        client.abort().await.unwrap();
        while !client.get_stderr().ends_with('\n') {
            tokio::task::yield_now().await;
        }
        assert_eq!(client.get_stderr(), "a\u{e9}\n");
        client.stop().await.unwrap();
    });
}

#[test]
fn stop_detaches_stdout_and_keeps_the_grace_boundary() {
    let peer = Peer::new();
    let ticking =
        "emit({'type': 'peer_ready'})\nwhile True:\n    emit({'type': 'tick'}); time.sleep(0.001)";
    on_task_set(|_| async {
        let client = RpcClient::new(peer.options(ticking));
        let mut events = events(&client);
        client.start().await.unwrap();
        next_of(&mut events, "tick").await;
        let started = std::time::Instant::now();
        let mut stop = pin!(client.stop());
        assert!(poll_once(stop.as_mut()).is_pending());
        while events.try_recv().is_ok() {}
        stop.await.unwrap();
        assert!(
            started.elapsed() < Duration::from_millis(1000),
            "an ordinary child is not killed"
        );
        assert!(events.try_recv().is_err(), "stdout is detached at stop");
    });
    let resisting = r"
signal.signal(signal.SIGTERM, lambda *_: os.write(2, b'terminating\n'))
emit({'type': 'peer_ready', 'pid': os.getpid()})
while True: time.sleep(1)
";
    on_paused_task_set(|_| async {
        let client = RpcClient::new(peer.options(resisting));
        let mut events = events(&client);
        client.start().await.unwrap();
        let pid = u32::try_from(
            next_of(&mut events, "peer_ready").await["pid"]
                .as_u64()
                .unwrap(),
        )
        .unwrap();
        let (release, hold) = hold_clock();
        let mut stop = pin!(client.stop());
        assert!(poll_once(stop.as_mut()).is_pending());
        while !client.get_stderr().contains("terminating") {
            tokio::task::yield_now().await;
        }
        tokio::time::advance(Duration::from_millis(999)).await;
        assert!(poll_once(stop.as_mut()).is_pending());
        assert!(process_exists(pid));
        tokio::time::advance(Duration::from_millis(1)).await;
        stop.await.unwrap();
        assert!(!process_exists(pid));
        assert!(
            events.try_recv().is_err(),
            "nothing is delivered after stop"
        );
        release.send(()).unwrap();
        hold.await.unwrap();
    });
}

#[test]
fn restart_keeps_ids_stderr_and_registered_listeners() {
    let peer = Peer::new();
    let script = r"
os.write(2, b'child\n')
emit({'type': 'peer_ready'})
for line in commands():
    command = json.loads(line)
    emit({'type': 'wire_capture', 'raw': line})
    if command['type'] != 'get_state': respond(command)
";
    on_task_set(|_| async {
        let client = RpcClient::new(peer.options(script));
        let mut events = events(&client);
        client.start().await.unwrap();
        next_of(&mut events, "peer_ready").await;
        client.abort().await.unwrap();
        assert_eq!(
            next_of(&mut events, "wire_capture").await["raw"],
            "{\"type\":\"abort\",\"id\":\"req_1\"}\n"
        );
        let mut pending = pin!(client.get_state());
        assert!(poll_once(pending.as_mut()).is_pending());
        next_of(&mut events, "wire_capture").await;
        client.stop().await.unwrap();
        assert!(matches!(pending.await, Err(RpcClientError::Stopped)));

        client.start().await.unwrap();
        next_of(&mut events, "peer_ready").await;
        client.abort().await.unwrap();
        assert_eq!(
            next_of(&mut events, "wire_capture").await["raw"],
            "{\"type\":\"abort\",\"id\":\"req_3\"}\n"
        );
        client.stop().await.unwrap();
        assert_eq!(client.get_stderr(), "child\nchild\n");
    });
}
