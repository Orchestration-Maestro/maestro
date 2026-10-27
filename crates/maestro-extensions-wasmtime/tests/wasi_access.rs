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

#[cfg(unix)]
#[test]
fn component_standard_basics_work_beside_pending_native_io() {
    if maestro_test_process::selection().is_child() {
        run(mixed_host());
    } else {
        mixed_parent();
    }
}

/// Captures guest stdout before allowing the pending native peer to complete.
#[cfg(unix)]
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
#[cfg(unix)]
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
    assert_eq!(completed["calls"], 1);
    let healthy = host.request(json!({"op":"environment"})).await;
    assert_eq!(healthy["p2"][0], "mixed Ω");
    assert_eq!(healthy["calls"], 3);
}

/// Observes file, environment and instance identity before the native read is released.
#[cfg(unix)]
async fn assert_mixed_basics(host: &Harness, path: &Path) {
    let basics = host.request(json!({"op":"basics","path":path})).await;
    assert_eq!(basics["calls"], 2);
    assert_eq!(basics["before"], json!(b"original\0\xfe"));
    assert_eq!(basics["after"], json!(b"updated\0\xff"));
    assert_eq!(basics["env"]["p2"][0], "mixed Ω");
    assert_eq!(basics["env"]["p3"][0], json!(["mixed Ω"]));
    assert_eq!(basics["env"]["p2"][1], std::env::var(COMPONENT).unwrap());
}

#[test]
fn component_inherits_separate_standard_streams() {
    let selected = maestro_test_process::selection();
    for generation in ["p2", "p3"] {
        for (index, input) in [b"".as_slice(), b"input\0\xff\x1b\xce\xa9".as_slice()]
            .into_iter()
            .enumerate()
        {
            let case = format!("{generation}-{index}");
            if !selected.is_child() {
                stdio_parent(&case, input);
            } else if selected.runs_case(&case) {
                run(stdio_host(generation, input));
            }
        }
    }
}

/// Checks actual captured pipes, including binary input and EOF, independently of notifications.
fn stdio_parent(case: &str, input: &[u8]) {
    use maestro_test_process::TestProcess;
    use std::{io::Write, process::Stdio};
    let mut child = TestProcess::current("component_inherits_separate_standard_streams")
        .unwrap()
        .case(case)
        .quiet()
        .nocapture()
        .stdin(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child.stdin.take().unwrap().write_all(input).unwrap();
    let output = child.wait_with_output().unwrap();
    assert!(
        output.status.success(),
        "{case}: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let body = output.stdout.strip_prefix(b"\nrunning 1 test\n").unwrap();
    let mut stdout = b"OUT[".to_vec();
    stdout.extend_from_slice(input);
    stdout.push(b']');
    let mut stderr = b"ERR[".to_vec();
    stderr.extend(input.iter().rev());
    stderr.push(b']');
    assert!(body.starts_with(&stdout), "{case}: {body:?}");
    assert!(
        body[stdout.len()..].starts_with(b".\ntest result: ok."),
        "{case}: {body:?}"
    );
    assert_eq!(output.stderr, stderr, "{case}");
}

/// Drives each standard generation's real descriptors in a checked isolated host.
async fn stdio_host(generation: &str, input: &[u8]) {
    let host = Harness::load(Path::new(&std::env::var(COMPONENT).unwrap())).await;
    let observed = host
        .request(json!({"op":"stdio","generation":generation}))
        .await;
    assert_eq!(observed["input"], json!(input), "{generation} EOF");
}

#[cfg(unix)]
#[test]
fn component_root_access_reaches_sibling_paths() {
    if maestro_test_process::run_alone("component_root_access_reaches_sibling_paths").unwrap() {
        return;
    }
    let original = std::env::current_dir().unwrap();
    let scratch = maestro_test_directory::Scratch::new().unwrap();
    std::fs::create_dir(scratch.path().join("cwd")).unwrap();
    std::fs::create_dir(scratch.path().join("sibling Ω space")).unwrap();
    std::os::unix::fs::symlink(
        "../sibling Ω space",
        scratch.path().join("cwd/relative-link"),
    )
    .unwrap();
    std::env::set_current_dir(scratch.path().join("cwd")).unwrap();
    let target = scratch.path().join("sibling Ω space/file Ω name");
    let paths = [
        target.clone(),
        "../sibling Ω space/file Ω name".into(),
        "relative-link/file Ω name".into(),
    ];
    run(root_paths(&paths, &target));
    std::env::set_current_dir(original).unwrap();
}

/// Each spelling independently writes and reads exact empty and binary file contents.
async fn root_paths(paths: &[std::path::PathBuf], target: &Path) {
    let host = Harness::load(Path::new(&std::env::var(COMPONENT).unwrap())).await;
    let cwd = host.request(json!({"op":"cwd"})).await;
    let native = std::env::current_dir().unwrap();
    let guest = maestro_extensions::helpers::guest_path(native.to_str().unwrap());
    assert_eq!(cwd["p2"]["path"], guest, "standard cwd: {cwd}");
    assert_eq!(cwd["p3"], guest, "asynchronous cwd: {cwd}");
    for generation in ["p2", "p3"] {
        for path in paths {
            for bytes in [b"".as_slice(), b"file\0\xff\x1b\xce\xa9".as_slice()] {
                let observed = host.request(json!({"op":"file","generation":generation,"path":path,"write":true,"bytes":bytes})).await;
                assert_eq!(
                    observed["bytes"],
                    json!(bytes),
                    "{generation} {path:?}: {observed}"
                );
                assert_eq!(
                    std::fs::read(target).unwrap(),
                    bytes,
                    "{generation} {path:?} native control"
                );
            }
        }
    }
}

/// The drive spelling of a native path, without the verbatim prefix a canonical path keeps.
#[cfg(windows)]
fn plain(path: &Path) -> String {
    let path = path.to_str().unwrap();
    path.strip_prefix(r"\\?\").unwrap_or(path).to_owned()
}

#[cfg(windows)]
#[test]
fn component_windows_roots_reach_drive_spellings() {
    if maestro_test_process::run_alone("component_windows_roots_reach_drive_spellings").unwrap() {
        return;
    }
    let original = std::env::current_dir().unwrap();
    let scratch = maestro_test_directory::Scratch::new().unwrap();
    std::fs::create_dir(scratch.path().join("cwd")).unwrap();
    std::fs::create_dir(scratch.path().join("sibling Ω space")).unwrap();
    std::env::set_current_dir(scratch.path().join("cwd")).unwrap();
    let target = scratch.path().join(r"sibling Ω space\file Ω name");
    let native = plain(&target);
    let mut paths: Vec<std::path::PathBuf> = vec![
        native.clone().into(),
        native.replace('\\', "/").into(),
        format!("{}{}", native[..1].to_ascii_lowercase(), &native[1..]).into(),
        format!(r"\\?\{native}").into(),
        r"..\sibling Ω space\file Ω name".into(),
        "../sibling Ω space/file Ω name".into(),
    ];
    // An account the host refuses symbolic links has no relative-link spelling to offer.
    match std::os::windows::fs::symlink_dir(
        r"..\sibling Ω space",
        scratch.path().join(r"cwd\relative-link"),
    ) {
        Ok(()) => paths.push(r"relative-link\file Ω name".into()),
        Err(error) => eprintln!("skipped the relative-link spelling: {error}"),
    }
    run(root_paths(&paths, &target));
    std::env::set_current_dir(original).unwrap();
}

#[cfg(windows)]
#[test]
fn component_windows_roots_reach_a_second_volume() {
    let first = maestro_test_directory::Scratch::new().unwrap();
    let drive = plain(first.path())[..1].to_ascii_uppercase();
    // A host with one writable volume has no second root to reach.
    let Some(scratch) = ('A'..='Z')
        .filter(|letter| letter.to_string() != drive)
        .find_map(|letter| {
            maestro_test_directory::Scratch::new_in(Path::new(&format!(r"{letter}:\"))).ok()
        })
    else {
        eprintln!("skipped: no writable volume besides {drive}:");
        return;
    };
    let target = scratch.path().join("file Ω name");
    let native = plain(&target);
    run(root_paths(
        &[native.clone().into(), native.replace('\\', "/").into()],
        &target,
    ));
}

#[cfg(windows)]
#[test]
fn component_windows_roots_reach_a_share_working_directory() {
    if maestro_test_process::run_alone("component_windows_roots_reach_a_share_working_directory")
        .unwrap()
    {
        return;
    }
    let original = std::env::current_dir().unwrap();
    let scratch = maestro_test_directory::Scratch::new().unwrap();
    std::fs::create_dir(scratch.path().join("cwd")).unwrap();
    std::fs::create_dir(scratch.path().join("sibling Ω space")).unwrap();
    let local = plain(scratch.path());
    let share = format!(r"\\localhost\{}${}", &local[..1], &local[2..]);
    // The administrative share is a host premise; without it no share is reachable.
    if let Err(error) = std::env::set_current_dir(format!(r"{share}\cwd")) {
        eprintln!("skipped: {share} is unavailable: {error}");
        return;
    }
    let target = scratch.path().join(r"sibling Ω space\file Ω name");
    let paths = [
        format!(r"{share}\sibling Ω space\file Ω name").into(),
        format!(r"\\?\UNC\{}\sibling Ω space\file Ω name", &share[2..]).into(),
        format!(
            r"\\LOCALHOST\{}${}\sibling Ω space\file Ω name",
            local[..1].to_ascii_lowercase(),
            &local[2..]
        )
        .into(),
        r"..\sibling Ω space\file Ω name".into(),
    ];
    run(root_paths(&paths, &target));
    std::env::set_current_dir(original).unwrap();
}

#[cfg(unix)]
#[test]
fn component_filesystem_failures_remain_os_failures() {
    if maestro_test_process::run_alone("component_filesystem_failures_remain_os_failures").unwrap()
    {
        return;
    }
    let scratch = maestro_test_directory::Scratch::new().unwrap();
    let healthy = scratch.path().join("healthy");
    let denied = scratch.path().join("denied");
    std::fs::write(&healthy, b"healthy\0\xff").unwrap();
    std::fs::write(&denied, b"denied bytes").unwrap();
    std::os::unix::fs::symlink("missing", scratch.path().join("dangling")).unwrap();
    let permission = maestro_test_directory::deny(&denied, 0).unwrap();
    let mut cases = vec![
        (scratch.path().join("missing"), "not-found"),
        (scratch.path().join("dangling"), "not-found"),
    ];
    if permission.is_some() {
        assert_eq!(
            std::fs::read(&denied).unwrap_err().kind(),
            std::io::ErrorKind::PermissionDenied
        );
        cases.push((denied.clone(), "permission-denied"));
    }
    run(file_failures(&cases, &healthy));
    if permission.is_some() {
        assert_eq!(
            std::fs::read(&denied).unwrap_err().kind(),
            std::io::ErrorKind::PermissionDenied
        );
    }
    unavailable_cwd(&scratch, &healthy, "removed");
    unavailable_cwd(&scratch, &healthy, "unreadable");
}

#[cfg(unix)]
#[test]
fn component_non_unicode_cwd_keeps_absolute_access() {
    use std::{ffi::OsString, os::unix::ffi::OsStringExt};
    if maestro_test_process::run_alone("component_non_unicode_cwd_keeps_absolute_access").unwrap() {
        return;
    }
    let original = std::env::current_dir().unwrap();
    let scratch = maestro_test_directory::Scratch::new().unwrap();
    let name = OsString::from_vec(vec![b'c', 255]);
    if !scratch.keeps_distinct(&[name.as_os_str()]).unwrap() {
        eprintln!("the working directory needs a host that stores the name bytes {name:?}");
        return;
    }
    let cwd = scratch.path().join(name);
    std::fs::create_dir(&cwd).unwrap();
    assert!(!Path::new(cwd.to_string_lossy().as_ref()).exists());
    let healthy = scratch.path().join("healthy");
    std::fs::write(&healthy, b"healthy\0\xff").unwrap();
    std::env::set_current_dir(&cwd).unwrap();
    assert!(std::env::current_dir().unwrap().to_str().is_none());
    run(async {
        let host = Harness::load(Path::new(&std::env::var(COMPONENT).unwrap())).await;
        let observed = host.request(json!({"op":"cwd"})).await;
        assert!(observed["p3"].is_null(), "{observed}");
        for generation in ["p2", "p3"] {
            let observed = host
                .request(json!({"op":"file","generation":generation,"path":healthy,"bytes":[]}))
                .await;
            assert_eq!(observed["bytes"], json!(b"healthy\0\xff"), "{generation}");
        }
        let healthy = host.request(json!({"op":"environment"})).await;
        assert_eq!(healthy["p2"][1], std::env::var(COMPONENT).unwrap());
    });
    std::env::set_current_dir(original).unwrap();
}

/// Each failed operation leaves both its native permissions and later callbacks healthy.
#[cfg(unix)]
async fn file_failures(cases: &[(std::path::PathBuf, &str)], healthy: &Path) {
    let host = Harness::load(Path::new(&std::env::var(COMPONENT).unwrap())).await;
    for generation in ["p2", "p3"] {
        for (path, error) in cases {
            let observed = host
                .request(json!({"op":"file","generation":generation,"path":path,"bytes":[]}))
                .await;
            assert_eq!(
                observed["error"], *error,
                "{generation} {path:?}: {observed}"
            );
            let observed = host
                .request(json!({"op":"file","generation":generation,"path":healthy,"bytes":[]}))
                .await;
            assert_eq!(
                observed["bytes"],
                json!(b"healthy\0\xff"),
                "{generation} healthy continuation"
            );
        }
    }
}

/// The host cannot observe cwd, but an otherwise accessible absolute artifact still loads.
#[cfg(unix)]
fn unavailable_cwd(scratch: &maestro_test_directory::Scratch, healthy: &Path, kind: &str) {
    let original = std::env::current_dir().unwrap();
    let permission = if kind == "removed" {
        scratch.enter_removed("removed").unwrap();
        None
    } else {
        let permission = scratch.enter_unreadable("unreadable").unwrap();
        if permission.is_none() {
            std::env::set_current_dir(&original).unwrap();
            return;
        }
        permission.map(|(_, restore)| restore)
    };
    assert!(
        std::env::current_dir().is_err(),
        "native {kind} cwd control"
    );
    run(async {
        let host = Harness::load(Path::new(&std::env::var(COMPONENT).unwrap())).await;
        let observed = host.request(json!({"op":"cwd"})).await;
        assert!(observed["p3"].is_null(), "{kind}: {observed}");
        for generation in ["p2", "p3"] {
            let observed = host
                .request(json!({"op":"file","generation":generation,"path":healthy,"bytes":[]}))
                .await;
            assert_eq!(
                observed["bytes"],
                json!(b"healthy\0\xff"),
                "{generation} {kind} absolute continuation"
            );
        }
    });
    drop(permission);
    std::env::set_current_dir(original).unwrap();
}

#[cfg(unix)]
#[test]
fn component_absolute_link_qualification() {
    if maestro_test_process::run_alone("component_absolute_link_qualification").unwrap() {
        return;
    }
    let original = std::env::current_dir().unwrap();
    let scratch = maestro_test_directory::Scratch::new().unwrap();
    let cwd = scratch.path().join("cwd");
    let outside = scratch.path().join("outside Ω");
    std::fs::create_dir(&cwd).unwrap();
    std::fs::create_dir(&outside).unwrap();
    let target = outside.join("bytes");
    std::os::unix::fs::symlink(&target, cwd.join("absolute-file")).unwrap();
    std::os::unix::fs::symlink(&outside, cwd.join("absolute-directory")).unwrap();
    std::os::unix::fs::symlink("../outside Ω/bytes", cwd.join("relative-file")).unwrap();
    std::os::unix::fs::symlink("../outside Ω", cwd.join("relative-directory")).unwrap();
    std::os::unix::fs::symlink(outside.join("missing"), cwd.join("dangling")).unwrap();
    let denied = outside.join("denied");
    std::fs::write(&denied, b"private").unwrap();
    let permission = maestro_test_directory::deny(&denied, 0).unwrap();
    std::env::set_current_dir(&cwd).unwrap();
    if permission.is_some() {
        assert_eq!(
            std::fs::read(&denied).unwrap_err().kind(),
            std::io::ErrorKind::PermissionDenied
        );
        run(qualify_links(&target, Some(&denied)));
    }
    run(qualify_links(&target, None));
    std::env::set_current_dir(original).unwrap();
}

/// Separates a runtime's absolute-link refusal from native permission denial.
#[cfg(unix)]
async fn qualify_links(target: &Path, denied: Option<&Path>) {
    let host = Harness::load(Path::new(&std::env::var(COMPONENT).unwrap())).await;
    for bytes in [
        b"first\0\xff".as_slice(),
        b"updated\0\xfe\xce\xa9".as_slice(),
    ] {
        std::fs::write(target, bytes).unwrap();
        for path in ["absolute-file", "absolute-directory/bytes"] {
            assert_eq!(std::fs::read(path).unwrap(), bytes, "native {path}");
        }
        let mut names: Vec<_> = std::fs::read_dir("absolute-directory")
            .unwrap()
            .map(|entry| entry.unwrap().file_name())
            .collect();
        names.sort();
        assert_eq!(names, ["bytes", "denied"]);
        assert_eq!(
            std::fs::read("dangling").unwrap_err().kind(),
            std::io::ErrorKind::NotFound
        );
        for generation in ["p2", "p3"] {
            for path in [
                target,
                Path::new("relative-file"),
                Path::new("relative-directory/bytes"),
            ] {
                let observed = host
                    .request(json!({"op":"file","generation":generation,"path":path,"bytes":[]}))
                    .await;
                assert_eq!(
                    observed["bytes"],
                    json!(bytes),
                    "{generation} {path:?}: {observed}"
                );
            }
            qualify_refusals(&host, generation, denied).await;
        }
    }
}

/// Checks the runtime's refusal and a genuinely unreadable native target separately.
#[cfg(unix)]
async fn qualify_refusals(host: &Harness, generation: &str, denied: Option<&Path>) {
    for path in ["absolute-file", "absolute-directory/bytes", "dangling"] {
        let observed = host
            .request(json!({"op":"file","generation":generation,"path":path,"bytes":[]}))
            .await;
        assert_eq!(
            observed["error"], "permission-denied",
            "{generation} {path}: {observed}"
        );
        if generation == "p3" {
            assert_eq!(
                observed["code"], "ErrorCode::NotPermitted",
                "{path}: {observed}"
            );
        }
    }
    let Some(denied) = denied else {
        return;
    };
    let observed = host
        .request(json!({"op":"file","generation":generation,"path":denied,"bytes":[]}))
        .await;
    assert_eq!(
        observed["error"], "permission-denied",
        "{generation}: {observed}"
    );
    if generation == "p3" {
        assert_eq!(
            observed["code"], "ErrorCode::Access",
            "OS denial: {observed}"
        );
    }
}
