//! Native byte resources through the compiled guest and shipped composition root.
#![cfg(target_os = "linux")]
use crate::support::{Root, rpc::Rpc};
use serde_json::{Value, json};
mod faults;
mod file_trigger;
mod files;
mod http;
mod reload;
mod watch;

/// Loads the compiled native-operation component through normal startup.
fn loaded(root: &Root) -> Rpc {
    connected(root, "http://127.0.0.1:1/v1")
}

/// Loads the same component with a controlled provider for active-run cancellation.
fn connected(root: &Root, base: &str) -> Rpc {
    root.models(base);
    let component = std::env::var("MAESTRO_NATIVE_IO_COMPONENT").unwrap();
    Rpc::command(&mut root.command(&[
        "--mode",
        "rpc",
        "--provider",
        "startup",
        "--model",
        "controlled",
        "--extension",
        &component,
    ]))
}

/// Obtains the actual guest operation's reply through the existing RPC UI adapter.
fn answer(rpc: &mut Rpc, data: &Value) -> Value {
    command_answer(rpc, "native-child", data)
}

/// Receives a named component command through the normal RPC UI adapter.
fn command_answer(rpc: &mut Rpc, command: &str, data: &Value) -> Value {
    let start = rpc.events.len();
    rpc.request(json!({"type":"prompt", "message":format!("/{command} {data}")}));
    rpc.events[start..]
        .iter()
        .find_map(|event| {
            event["title"]
                .as_str()
                .and_then(|title| title.strip_prefix("NATIVE-IO:"))
                .map(|text| serde_json::from_str(text).unwrap())
        })
        .expect("guest native-operation reply")
}

/// Requires successful admission while retaining JSON null separately from errors.
fn call(rpc: &mut Rpc, data: &Value) -> Value {
    let reply = answer(rpc, data);
    assert!(reply.get("error").is_none(), "{reply}");
    reply["value"].clone()
}

/// Accumulates a prefix without assuming native chunk boundaries.
fn prefix(rpc: &mut Rpc, stderr: bool, length: usize) -> Vec<u8> {
    let mut bytes = Vec::new();
    while bytes.len() < length {
        let chunk = call(rpc, &json!({"op":"read", "stderr":stderr}));
        assert!(!chunk.is_null(), "unexpected EOF before prefix");
        bytes.extend(serde_json::from_value::<Vec<u8>>(chunk).unwrap());
    }
    bytes
}

/// Reads the selected descriptor to its own EOF.
fn drain(rpc: &mut Rpc, stderr: bool) -> Vec<u8> {
    let mut bytes = Vec::new();
    loop {
        let chunk = call(rpc, &json!({"op":"read", "stderr":stderr}));
        if chunk.is_null() {
            return bytes;
        }
        bytes.extend(serde_json::from_value::<Vec<u8>>(chunk).unwrap());
    }
}

#[test]
fn native_child_streams_bytes_while_input_is_pending() {
    let root = Root::new();
    let mut rpc = loaded(&root);
    let producer = "import os; os.write(1,b'\\x00\\xff\\xe2'); os.write(2,b'error\\x00'); release=os.read(0,1); os.write(1,b'\\x82\\xac'+release); os.write(2,b'last'); tail=b''\nwhile True:\n b=os.read(0,4096)\n if not b: break\n tail+=b\nos.write(1,tail)";
    let pid = call(
        &mut rpc,
        &json!({"op":"spawn","command":"python3","args":["-c",producer]}),
    )
    .as_u64()
    .unwrap();
    assert!(pid > 0);
    assert_eq!(prefix(&mut rpc, false, 3), [0, 255, 226]);
    assert_eq!(prefix(&mut rpc, true, 6), b"error\0");
    call(&mut rpc, &json!({"op":"write", "bytes":[1]}));
    assert_eq!(prefix(&mut rpc, false, 3), [130, 172, 1]);
    assert_eq!(prefix(&mut rpc, true, 4), b"last");
    call(&mut rpc, &json!({"op":"write", "bytes":[0,255,2]}));
    call(&mut rpc, &json!({"op":"end"}));
    assert_eq!(drain(&mut rpc, false), [0, 255, 2]);
    assert!(drain(&mut rpc, true).is_empty());
    assert_eq!(
        call(&mut rpc, &json!({"op":"wait"})),
        json!({"code":0,"signal":null})
    );
    rpc.finish();
}

/// Starts one producer on a private socket and reads its own PID acknowledgement.
fn ready(listener: &std::net::TcpListener) -> (std::net::TcpStream, u64) {
    use std::io::Read;
    let (mut socket, _) = listener.accept().unwrap();
    let mut pid = [0; 8];
    socket.read_exact(&mut pid).unwrap();
    (socket, u64::from_be_bytes(pid))
}
/// Releases only this acknowledged producer.
fn release(mut socket: std::net::TcpStream) {
    use std::io::Write;
    socket.write_all(&[1]).unwrap();
}
/// Decodes an observed child environment without normalizing values.
fn environment(bytes: &[u8]) -> std::collections::BTreeMap<String, String> {
    bytes
        .split(|byte| *byte == 0)
        .filter(|entry| !entry.is_empty())
        .map(|entry| {
            let (key, value) = std::str::from_utf8(entry).unwrap().split_once('=').unwrap();
            (key.into(), value.into())
        })
        .collect()
}

#[test]
fn native_child_keeps_literal_arguments_and_cwd() {
    let root = Root::new();
    let outside = root.path("outside Ω");
    std::fs::create_dir(&outside).unwrap();
    let mut rpc = loaded(&root);
    let operands = ["space value", "$HOME; `literal`", "line\nbreak", "Ω雪", ""];
    for cwd in [None, Some(outside.to_str().unwrap())] {
        let mut argv = vec![
            "-c",
            "import os,sys,json; print(json.dumps([os.getcwd(),sys.argv[1:]],ensure_ascii=False))",
        ];
        argv.extend(operands);
        call(
            &mut rpc,
            &json!({"op":"spawn","command":"python3","args":argv,"cwd":cwd}),
        );
        let bytes = drain(&mut rpc, false);
        let value: Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(
            value,
            json!([
                cwd.map_or_else(|| root.path("project"), std::path::PathBuf::from),
                operands
            ])
        );
        assert!(drain(&mut rpc, true).is_empty());
        assert_eq!(call(&mut rpc, &json!({"op":"wait"}))["code"], 0);
    }
    rpc.finish();
}

#[test]
fn native_child_keeps_environment_selection() {
    let root = Root::new();
    root.models("http://127.0.0.1:1/v1");
    let component = std::env::var("MAESTRO_NATIVE_IO_COMPONENT").unwrap();
    let mut command = root.command(&[
        "--mode",
        "rpc",
        "--provider",
        "startup",
        "--model",
        "controlled",
        "--extension",
        &component,
    ]);
    command
        .env("UNCHANGED", "inherited Ω")
        .env("DELETE_ME", "not in snapshot");
    let mut rpc = Rpc::command(&mut command);
    call(
        &mut rpc,
        &json!({"op":"spawn","command":"/usr/bin/env","args":["-0"]}),
    );
    let inherited = environment(&drain(&mut rpc, false));
    assert_eq!(inherited["UNCHANGED"], "inherited Ω");
    assert_eq!(inherited["DELETE_ME"], "not in snapshot");
    call(&mut rpc, &json!({"op":"wait"}));
    for env in [
        json!([]),
        json!([["UNCHANGED", "inherited Ω"], ["OVERLAY", "replacement"]]),
    ] {
        call(
            &mut rpc,
            &json!({"op":"spawn","command":"/usr/bin/env","args":["-0"],"env":env}),
        );
        let actual = environment(&drain(&mut rpc, false));
        let expected: std::collections::BTreeMap<String, String> =
            serde_json::from_value::<Vec<(String, String)>>(env)
                .unwrap()
                .into_iter()
                .collect();
        assert_eq!(actual, expected);
        call(&mut rpc, &json!({"op":"wait"}));
    }
    rpc.finish();
}

#[test]
fn native_child_keeps_descriptor_selection() {
    let root = Root::new();
    let mut rpc = loaded(&root);
    let mut selections = vec![["pipe"; 3], ["ignore", "inherit", "ignore"]];
    for fd in 0..3 {
        for choice in ["ignore", "inherit"] {
            let mut selected = ["pipe"; 3];
            selected[fd] = choice;
            selections.push(selected);
        }
    }
    let producer = "import os,socket,sys; s=socket.create_connection(('127.0.0.1',int(sys.argv[1]))); s.sendall(os.getpid().to_bytes(8,'big')); s.recv(1)";
    for selected in selections {
        let listener = std::net::TcpListener::bind(("127.0.0.1", 0)).unwrap();
        let pid = call(&mut rpc, &json!({"op":"spawn","command":"python3","args":["-c",producer,listener.local_addr().unwrap().port().to_string()],"stdio":selected})).as_u64().unwrap();
        let (socket, announced) = ready(&listener);
        assert_eq!(announced, pid);
        assert_eq!(
            call(&mut rpc, &json!({"op":"descriptors"})),
            json!(selected.map(|selection| selection == "pipe"))
        );
        for (fd, selection) in selected.into_iter().enumerate() {
            let child = std::fs::read_link(format!("/proc/{pid}/fd/{fd}")).unwrap();
            let parent = std::fs::read_link(format!("/proc/{}/fd/{fd}", rpc.pid())).unwrap();
            match selection {
                "inherit" => assert_eq!(child, parent),
                "ignore" => assert_eq!(child, std::path::Path::new("/dev/null")),
                _ => assert_ne!(child, parent),
            }
        }
        release(socket);
        assert_eq!(call(&mut rpc, &json!({"op":"wait"}))["code"], 0);
    }
    rpc.finish();
}

#[test]
fn native_child_writes_all_bytes_before_eof() {
    let root = Root::new();
    let mut rpc = loaded(&root);
    call(
        &mut rpc,
        &json!({"op":"spawn","command":"python3","args":["-c","import sys; data=sys.stdin.buffer.read(); sys.stdout.buffer.write(data)"]}),
    );
    let bytes: Vec<u8> = (0..131_089)
        .map(|value| u8::try_from(value % 256).unwrap())
        .collect();
    call(&mut rpc, &json!({"op":"write","bytes":[]}));
    call(&mut rpc, &json!({"op":"write","bytes":bytes}));
    call(&mut rpc, &json!({"op":"write","bytes":[255,0,226,130,172]}));
    call(&mut rpc, &json!({"op":"end"}));
    let mut expected = bytes;
    expected.extend([255, 0, 226, 130, 172]);
    assert_eq!(drain(&mut rpc, false), expected);
    assert_eq!(call(&mut rpc, &json!({"op":"wait"}))["code"], 0);
    rpc.finish();
}

#[test]
fn native_child_completion_keeps_output_close_order() {
    let root = Root::new();
    let mut rpc = loaded(&root);
    let listener = std::net::TcpListener::bind(("127.0.0.1", 0)).unwrap();
    let producer = "import os,socket,sys\nr,w=os.pipe()\nif os.fork()==0:\n os.close(w)\n while os.read(r,1): pass\n s=socket.create_connection(('127.0.0.1',int(sys.argv[1])))\n s.sendall(os.getpid().to_bytes(8,'big')); s.recv(1)\n os.write(1,b'late\\x00\\xff'); os.write(2,b'late-error'); os._exit(0)\nos.close(r); os.write(1,b'early'); os.write(2,b'early-error'); os._exit(7)";
    let pid = call(&mut rpc,&json!({"op":"spawn","command":"python3","args":["-c",producer,listener.local_addr().unwrap().port().to_string()]})).as_u64().unwrap();
    let (socket, descendant) = ready(&listener);
    assert_ne!(descendant, pid);
    assert_eq!(prefix(&mut rpc, false, 5), b"early");
    assert_eq!(prefix(&mut rpc, true, 11), b"early-error");
    assert_eq!(call(&mut rpc, &json!({"op":"wait_start"})), true);
    release(socket);
    assert_eq!(
        call(&mut rpc, &json!({"op":"wait_finish"})),
        json!({"code":7,"signal":null})
    );
    assert_eq!(drain(&mut rpc, false), b"late\0\xff");
    assert_eq!(drain(&mut rpc, true), b"late-error");
    rpc.finish();
}

#[test]
fn native_child_keeps_exit_and_signal_distinct() {
    let root = Root::new();
    let mut rpc = loaded(&root);
    for code in [0, 7] {
        call(
            &mut rpc,
            &json!({"op":"spawn","command":"python3","args":["-c",format!("import sys; sys.exit({code})")]}),
        );
        assert_eq!(
            call(&mut rpc, &json!({"op":"wait"})),
            json!({"code":code,"signal":null})
        );
    }
    call(
        &mut rpc,
        &json!({"op":"spawn","command":"python3","args":["-c","import os; os.write(1,b'ready'); os.read(0,1)"]}),
    );
    assert_eq!(prefix(&mut rpc, false, 5), b"ready");
    assert_eq!(call(&mut rpc, &json!({"op":"wait_start"})), true);
    assert_eq!(call(&mut rpc, &json!({"op":"kill"})), true);
    assert_eq!(
        call(&mut rpc, &json!({"op":"wait_finish"})),
        json!({"code":null,"signal":"SIGTERM"})
    );
    rpc.finish();
}

#[test]
fn native_child_kill_controls_the_retained_child() {
    let root = Root::new();
    let mut rpc = loaded(&root);
    call(
        &mut rpc,
        &json!({"op":"spawn","command":"python3","args":["-c","import os; os.write(1,b'ready'); os.read(0,1)"]}),
    );
    assert_eq!(prefix(&mut rpc, false, 5), b"ready");
    assert_eq!(
        call(&mut rpc, &json!({"op":"kill","signal":"SIGKILL"})),
        true
    );
    let status = call(&mut rpc, &json!({"op":"wait"}));
    assert_eq!(status, json!({"code":null,"signal":"SIGKILL"}));
    assert_eq!(call(&mut rpc, &json!({"op":"kill"})), false);
    assert_eq!(call(&mut rpc, &json!({"op":"wait"})), status);
    rpc.finish();
}

#[test]
fn native_child_group_signal_keeps_direct_fallback() {
    let root = Root::new();
    let mut rpc = loaded(&root);
    let producer = "import os,json,socket,sys\nif sys.argv[1]=='1' and os.fork()==0:\n s=socket.create_connection(('127.0.0.1',int(sys.argv[2]))); s.sendall(os.getpid().to_bytes(8,'big')+os.getpgrp().to_bytes(8,'big')); s.recv(1); os._exit(0)\nos.write(1,(json.dumps([os.getpid(),os.getpgrp(),os.getsid(0)])+'\\n').encode()); os.read(0,1)";
    for detached in [true, false] {
        let listener = std::net::TcpListener::bind(("127.0.0.1", 0)).unwrap();
        let pid = call(&mut rpc,&json!({"op":"spawn","command":"python3","args":["-c",producer,if detached { "1" } else { "0" },listener.local_addr().unwrap().port().to_string()],"detached":detached})).as_u64().unwrap();
        let mut line = Vec::new();
        while !line.ends_with(b"\n") {
            line.extend(prefix(&mut rpc, false, 1));
        }
        let identity: Vec<u64> = serde_json::from_slice(&line).unwrap();
        assert_eq!(identity[0], pid);
        let group = -i32::try_from(pid).unwrap();
        if detached {
            use std::io::Read;
            let (mut socket, descendant) = ready(&listener);
            assert_ne!(descendant, pid);
            let mut inherited = [0; 8];
            socket.read_exact(&mut inherited).unwrap();
            assert_eq!(u64::from_be_bytes(inherited), pid);
            assert_eq!(identity, [pid, pid, pid]);
            call(
                &mut rpc,
                &json!({"op":"signal_pid","pid":group,"signal":"SIGKILL"}),
            );
            assert_eq!(socket.read(&mut [0]).unwrap(), 0);
        } else {
            assert_ne!(identity[1], pid);
            assert!(
                answer(
                    &mut rpc,
                    &json!({"op":"signal_pid","pid":group,"signal":"SIGKILL"})
                )
                .get("error")
                .is_some()
            );
            assert_eq!(
                call(&mut rpc, &json!({"op":"kill","signal":"SIGKILL"})),
                true
            );
        }
        assert_eq!(
            call(&mut rpc, &json!({"op":"wait"})),
            json!({"code":null,"signal":"SIGKILL"})
        );
    }
    rpc.finish();
}

#[test]
fn native_child_errors_remain_catchable() {
    let root = Root::new();
    let mut rpc = loaded(&root);
    for request in [
        json!({"op":"spawn","command":root.path("missing"),"args":[]}),
        json!({"op":"spawn","command":"python3","args":[],"cwd":root.path("missing")}),
    ] {
        assert!(answer(&mut rpc, &request)["error"].is_string());
    }
    call(
        &mut rpc,
        &json!({"op":"spawn","command":"python3","args":["-c","import os,sys; os.write(1,b'ready'); os.write(1,sys.stdin.buffer.read())"]}),
    );
    assert_eq!(prefix(&mut rpc, false, 5), b"ready");
    assert!(answer(&mut rpc, &json!({"op":"kill","signal":"invalid-signal"}))["error"].is_string());
    call(&mut rpc, &json!({"op":"write","bytes":[0,255]}));
    call(&mut rpc, &json!({"op":"end"}));
    assert!(answer(&mut rpc, &json!({"op":"write","bytes":[1]}))["error"].is_string());
    assert_eq!(drain(&mut rpc, false), [0, 255]);
    assert_eq!(call(&mut rpc, &json!({"op":"wait"}))["code"], 0);
    rpc.finish();
}

#[test]
fn native_child_handles_outlive_the_callback() {
    let root = Root::new();
    let mut rpc = loaded(&root);
    call(
        &mut rpc,
        &json!({"op":"spawn","command":"python3","args":["-c","import os,sys; os.write(1,b'ready'); os.write(1,sys.stdin.buffer.read()); os.write(2,b'complete')"]}),
    );
    assert_eq!(prefix(&mut rpc, false, 5), b"ready");
    call(&mut rpc, &json!({"op":"save_pipes"}));
    call(&mut rpc, &json!({"op":"drop"}));
    call(&mut rpc, &json!({"op":"write","bytes":[0,255,226,130,172]}));
    call(&mut rpc, &json!({"op":"end"}));
    assert_eq!(drain(&mut rpc, false), [0, 255, 226, 130, 172]);
    assert_eq!(drain(&mut rpc, true), b"complete");
    call(&mut rpc, &json!({"op":"drop_pipes"}));
    rpc.finish();
}

/// A controlled provider that acknowledges the real attempt and holds its response headers.
struct HeldProvider {
    /// Loopback endpoint for the ordinary native model owner.
    base: String,
    /// The exact request admitted after the guest's agent-start handler returned.
    request: std::sync::mpsc::Receiver<Value>,
    /// Releases the held server transaction during teardown.
    release: std::sync::mpsc::Sender<()>,
    /// Teardown also wakes an accept if startup failed before the attempt.
    stopped: std::sync::Arc<std::sync::atomic::AtomicBool>,
    /// Listener used only for the teardown wake-up.
    address: std::net::SocketAddr,
    /// Original serving thread, joined exactly once.
    worker: Option<std::thread::JoinHandle<()>>,
}
impl HeldProvider {
    /// Admits one controlled request without any readiness timer.
    fn new() -> Self {
        use std::sync::{
            Arc,
            atomic::{AtomicBool, Ordering},
            mpsc,
        };
        let listener = std::net::TcpListener::bind(("127.0.0.1", 0)).unwrap();
        let address = listener.local_addr().unwrap();
        let stopped = Arc::new(AtomicBool::new(false));
        let closing = Arc::clone(&stopped);
        let (sent, request) = mpsc::channel();
        let (release, released) = mpsc::channel();
        let worker = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            if closing.load(Ordering::Relaxed) {
                return;
            }
            sent.send(crate::support::provider::request(&mut stream))
                .unwrap();
            let _ = released.recv();
        });
        Self {
            base: format!("http://{address}/v1"),
            request,
            release,
            stopped,
            address,
            worker: Some(worker),
        }
    }
}
impl Drop for HeldProvider {
    fn drop(&mut self) {
        self.stopped
            .store(true, std::sync::atomic::Ordering::Relaxed);
        let _ = self.release.send(());
        drop(std::net::TcpStream::connect(self.address));
        self.worker.take().unwrap().join().unwrap();
    }
}

#[test]
fn native_child_abort_keeps_the_original_signal() {
    use std::io::Read;
    let root = Root::new();
    let provider = HeldProvider::new();
    root.models(&provider.base);
    let component = std::env::var("MAESTRO_NATIVE_IO_COMPONENT").unwrap();
    let mut rpc = Rpc::command(&mut root.command(&[
        "--mode",
        "rpc",
        "--provider",
        "startup",
        "--model",
        "controlled",
        "--extension",
        &component,
    ]));
    let listener = std::net::TcpListener::bind(("127.0.0.1", 0)).unwrap();
    let producer = "import os,socket,sys; s=socket.create_connection(('127.0.0.1',int(sys.argv[1]))); s.sendall(os.getpid().to_bytes(8,'big')); os.read(0,1)";
    call(
        &mut rpc,
        &json!({"op":"arm_abort","command":"python3","args":["-c",producer,listener.local_addr().unwrap().port().to_string()]}),
    );
    rpc.request(json!({"type":"prompt","message":"native-child-abort"}));
    let (mut socket, pid) = ready(&listener);
    assert!(pid > 0);
    assert_eq!(provider.request.recv().unwrap()["model"], "controlled");
    rpc.request(json!({"type":"abort"}));
    assert_eq!(socket.read(&mut [0]).unwrap(), 0);
    assert_eq!(call(&mut rpc, &json!({"op":"observer"})), true);
    assert_eq!(
        call(&mut rpc, &json!({"op":"wait"})),
        json!({"code":null,"signal":"SIGTERM"})
    );
    assert_eq!(call(&mut rpc, &json!({"op":"observer"})), false);
    rpc.finish();
}

#[test]
fn native_io_keeps_full_user_basics() {
    use std::{io::Write, process::Stdio};
    // The child reads to the end of its input, so no sibling fork may hold the pipe open.
    if maestro_test_process::run_alone("native_io_keeps_full_user_basics").unwrap() {
        return;
    }
    let root = Root::new();
    root.models("http://127.0.0.1:1/v1");
    let outside = root.path("outside Ω");
    let target = root.path("linked 雪");
    let link = root.path("project/relative link");
    std::fs::write(&outside, b"absolute\0\xff").unwrap();
    std::fs::write(&target, b"symlink\0\xfe").unwrap();
    std::os::unix::fs::symlink("../linked 雪", &link).unwrap();
    let component = std::env::var("MAESTRO_NATIVE_IO_COMPONENT").unwrap();
    let mut child = root
        .command(&[
            "--mode",
            "rpc",
            "--provider",
            "startup",
            "--model",
            "controlled",
            "--extension",
            &component,
        ])
        .env(
            "MAESTRO_NATIVE_IO_BASICS",
            json!([outside, "relative link"]).to_string(),
        )
        .env("NATIVE_IO_SENTINEL", "inherited Ω\n")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let input = b"input\0\xff\xe2\x82\xac";
    child.stdin.take().unwrap().write_all(input).unwrap();
    let output = child.wait_with_output().unwrap();
    assert!(output.status.success(), "{output:?}");
    assert_eq!(
        output.stdout,
        b"inherited \xce\xa9\nabsolute\0\xffsymlink\0\xfeOUT[input\0\xff\xe2\x82\xac]"
    );
    assert_eq!(output.stderr, b"ERR[\xac\x82\xe2\xff\0tupni]");
    for path in [&outside, &target, &link] {
        assert_eq!(std::fs::read(path).unwrap(), b"changed\0\xff\xe9\x9b\xaa");
    }
    assert!(std::fs::symlink_metadata(link).unwrap().is_symlink());
}

#[test]
fn native_io_adapters_keep_the_same_author_contract() {
    let root = Root::new();
    let mut rpc = loaded(&root);
    call(
        &mut rpc,
        &json!({"op":"spawn","command":"python3","args":["-c","import os; os._exit(0)"]}),
    );
    assert_eq!(call(&mut rpc, &json!({"op":"aliases"})), true);
    assert_eq!(call(&mut rpc, &json!({"op":"wait"}))["code"], 0);
    http::adapter_contract(&mut rpc);
    watch::adapter_contract(&root, &mut rpc);
    rpc.finish();
}
