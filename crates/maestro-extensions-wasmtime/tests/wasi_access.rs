//! Real component environment and standard-interface access in checked isolated hosts.
#![cfg(test)]
use super::wasi_support::{COMPONENT, Harness, run};
use serde_json::json;
use std::{collections::BTreeMap, path::Path};

/// An explicitly controlled, nonsecret environment variable.
const SENTINEL: &str = "MAESTRO_WASI_SENTINEL";

#[test]
fn component_environment_keeps_inherited_values() {
    let component = std::env::var(COMPONENT).unwrap();
    let cases = [
        None,
        Some(""),
        Some("ordinary"),
        Some(" space Ω # % ! "),
        Some("\u{feff}"),
        Some("\u{85}"),
    ];
    let selected = maestro_test_process::run_per_environment(
        "component_environment_keeps_inherited_values",
        cases,
        |value| {
            let mut variables = BTreeMap::from([(COMPONENT, component.as_str())]);
            if let Some(value) = value {
                variables.insert(SENTINEL, value);
            }
            variables
        },
    )
    .unwrap();
    for supplied in selected.into_iter().flatten() {
        run(async {
            let host = Harness::load(Path::new(&component)).await;
            let observed = host.request(json!({"op":"environment"})).await;
            assert_eq!(observed["p2"][0], json!(supplied), "p2 {supplied:?}");
            assert_eq!(
                observed["p3"][0],
                json!(supplied.into_iter().collect::<Vec<_>>()),
                "p3 {supplied:?}"
            );
        });
        assert_eq!(std::env::var(SENTINEL).ok().as_deref(), supplied);
    }
}

#[test]
fn component_host_location_wins_over_inherited_environment() {
    let component = std::env::var(COMPONENT).unwrap();
    let key = if cfg!(windows) {
        "maestro_extension_path"
    } else {
        "MAESTRO_EXTENSION_PATH"
    };
    let selected = maestro_test_process::run_per_environment(
        "component_host_location_wins_over_inherited_environment",
        [None, Some(""), Some("/wrong/index.wasm")],
        |supplied| {
            let mut vars =
                BTreeMap::from([(COMPONENT, component.as_str()), (SENTINEL, "unrelated Ω")]);
            if let Some(value) = supplied {
                vars.insert(key, value);
            }
            vars
        },
    )
    .unwrap();
    for supplied in selected.into_iter().flatten() {
        let scratch = maestro_test_directory::Scratch::new().unwrap();
        let paths = ["first # Ω.wasm", "second.wasm", "relocated.wasm"]
            .map(|name| scratch.path().join(name));
        for path in &paths[..2] {
            std::fs::copy(&component, path).unwrap();
        }
        run(async {
            let earlier = Harness::load(&paths[0]).await;
            let later = Harness::load(&paths[1]).await;
            for (host, expected) in [
                (&earlier, &paths[0]),
                (&later, &paths[1]),
                (&earlier, &paths[0]),
            ] {
                let value = host.request(json!({"op":"environment"})).await;
                assert_eq!(value["p2"][1], json!(expected), "p2 {supplied:?}");
                assert_eq!(
                    value["p3"][1],
                    json!([expected]),
                    "p3 unique reserved key {supplied:?}"
                );
                assert_eq!(value["p2"][0], "unrelated Ω");
                assert_eq!(value["p3"][0], json!(["unrelated Ω"]));
            }
            std::fs::rename(&paths[1], &paths[2]).unwrap();
            let relocated = Harness::load(&paths[2]).await;
            let value = relocated.request(json!({"op":"environment"})).await;
            assert_eq!(value["p2"][1], json!(paths[2]), "p2 relocation");
            assert_eq!(value["p3"][1], json!([paths[2]]), "p3 relocation");
        });
        assert_eq!(std::env::var(key).ok().as_deref(), supplied);
    }
}

#[cfg(unix)]
#[test]
fn component_environment_keeps_native_string_conversion() {
    use std::{ffi::OsString, os::unix::ffi::OsStringExt};
    let component = std::env::var_os(COMPONENT).unwrap();
    let selected = maestro_test_process::run_per_environment(
        "component_environment_keeps_native_string_conversion",
        [()],
        |()| {
            BTreeMap::from([
                (OsString::from(COMPONENT), component.clone()),
                (
                    OsString::from(SENTINEL),
                    OsString::from_vec(vec![255, 65, 195, 40]),
                ),
                (
                    OsString::from("MAESTRO_WASI_NEIGHBOR"),
                    OsString::from("valid Ω"),
                ),
            ])
        },
    )
    .unwrap();
    for () in selected.into_iter().flatten() {
        run(async {
            let host = Harness::load(Path::new(&component)).await;
            let observed = host.request(json!({"op":"environment"})).await;
            assert_eq!(observed["p2"][0], "�A�(", "p2 native conversion");
            assert_eq!(observed["p3"][0], json!(["�A�("]), "p3 native conversion");
            assert_eq!(observed["neighbor"], "valid Ω");
        });
    }
}

#[test]
fn component_standard_basics_work_beside_pending_native_io() {
    if maestro_test_process::selection().is_child() {
        run(mixed_host());
    } else {
        mixed_parent();
    }
}

/// Captures guest stdout before allowing the pending native peer to complete.
fn mixed_parent() {
    use maestro_test_process::TestProcess;
    use std::{
        io::{Read, Write},
        process::Stdio,
    };
    let mut child = TestProcess::current("component_standard_basics_work_beside_pending_native_io")
        .unwrap()
        .case("*")
        .quiet()
        .nocapture()
        .env(SENTINEL, "mixed Ω")
        .stdin(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let mut seen = vec![0; b"\nrunning 1 test\nMIXED\0\xff\x1b[32m".len()];
    child
        .stdout
        .as_mut()
        .unwrap()
        .read_exact(&mut seen)
        .unwrap();
    if seen != b"\nrunning 1 test\nMIXED\0\xff\x1b[32m" {
        let output = child.wait_with_output().unwrap();
        panic!(
            "standard calls failed before release: {seen:?} {} {}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    }
    child.stdin.as_mut().unwrap().write_all(b"R").unwrap();
    let output = child.wait_with_output().unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}

/// Keeps one real native read pending while another callback uses the standard library.
async fn mixed_host() {
    use futures_util::future::{Either, select};
    use std::io::Read;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    let scratch = maestro_test_directory::Scratch::new().unwrap();
    let path = scratch.path().join("mixed file Ω");
    std::fs::write(&path, b"original\0\xfe").unwrap();
    let host = Harness::load(Path::new(&std::env::var(COMPONENT).unwrap())).await;
    let pending = std::rc::Rc::new(tokio::sync::Notify::new());
    let ready = pending.clone();
    let _subscription = host.bus.controller.on(
        "pending",
        std::rc::Rc::new(move |value, _| {
            assert_eq!(value["pending"], true);
            ready.notify_one();
            Ok(None)
        }),
    );
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let request = json!({"op":"pending","port":listener.local_addr().unwrap().port()});
    let invocation = std::pin::pin!(host.request(request));
    let peer = std::pin::pin!(async {
        let (mut peer, _) = listener.accept().await.unwrap();
        let mut bytes = [0; 11];
        peer.read_exact(&mut bytes).await.unwrap();
        assert_eq!(&bytes, b"guest-ready");
        pending.notified().await;
        peer
    });
    let Either::Right((mut peer, invocation)) = select(invocation, peer).await else {
        panic!("native read did not remain pending");
    };
    assert_mixed_basics(&host, &path).await;
    let mut release = [0];
    std::io::stdin().read_exact(&mut release).unwrap();
    assert_eq!(&release, b"R");
    peer.write_all(b"native completion\0\xff").await.unwrap();
    peer.shutdown().await.unwrap();
    let completed = invocation.await;
    assert_eq!(completed["bytes"], json!(b"native completion\0\xff"));
    let healthy = host.request(json!({"op":"environment"})).await;
    assert_eq!(healthy["p2"][0], "mixed Ω");
}

/// Observes file, environment and instance identity before the native read is released.
async fn assert_mixed_basics(host: &Harness, path: &Path) {
    let basics = host.request(json!({"op":"basics","path":path})).await;
    assert_eq!(basics["before"], json!(b"original\0\xfe"));
    assert_eq!(basics["after"], json!(b"updated\0\xff"));
    assert_eq!(basics["env"]["p2"][0], "mixed Ω");
    assert_eq!(basics["env"]["p3"][0], json!(["mixed Ω"]));
    assert_eq!(basics["env"]["p2"][1], std::env::var(COMPONENT).unwrap());
}
