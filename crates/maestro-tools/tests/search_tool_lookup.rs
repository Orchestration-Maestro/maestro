//! Native discovery, PATH probe ordering and call-time offline advice.
#![cfg(test)]
#![cfg(unix)]

use crate::read_support;

use crate::runtime::run;
use maestro_tools::{SearchTool, ToolsManager, ToolsManagerOutput};
use read_support::{Fixture, Scratch, counting_fetch, isolated_os};
use std::{
    path::Path,
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
};
use tokio::io::AsyncReadExt;

/// Install a shell probe through a child, with no inherited writable executable descriptor.
fn script(root: &Path, name: &str, code: i32) {
    let source = root.join(format!("{name}.source"));
    let body = format!(
        "#!/bin/sh\nprintf '%s %s\\n' '{name}' \"$*\" >> '{}'\n[ \"$#\" = 1 ] && [ \"$1\" = --version ] || exit 99\nexit {code}\n",
        root.join("probes.log").display()
    );
    std::fs::write(&source, body).unwrap();
    assert!(
        std::process::Command::new("install")
            .args(["-m", "0700"])
            .arg(source)
            .arg(root.join(name))
            .status()
            .unwrap()
            .success()
    );
}

/// Completed probes recorded by the child; absence is distinguished from empty text.
fn probe_log(root: &Path) -> Option<String> {
    match std::fs::read_to_string(root.join("probes.log")) {
        Ok(text) => Some(text),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => None,
        Err(error) => panic!("probe log read failed: {error}"),
    }
}

/// Run one child with the supplied scripts and offline flag.
fn child(test: &str, mode: &str, root: &Path, offline: &str) -> bool {
    isolated_os(
        test,
        mode,
        &[
            ("PATH", Some(root.as_os_str())),
            ("MAESTRO_OFFLINE", Some(std::ffi::OsStr::new(offline))),
            ("MAESTRO_SEARCH_ROOT", Some(root.as_os_str())),
        ],
    )
}

/// The scripts directory captured by the parent.
fn child_root() -> std::path::PathBuf {
    std::env::var_os("MAESTRO_SEARCH_ROOT").unwrap().into()
}

#[test]
fn binary_directory_entry_wins_before_any_probe() {
    run(async {
        if !maestro_test_process::selection().is_child() {
            for mode in ["entries", "dangling"] {
                let scripts = Scratch::new().unwrap();
                probe_scripts(scripts.path(), mode);
                assert!(!child(
                    "binary_directory_entry_wins_before_any_probe",
                    mode,
                    scripts.path(),
                    "1"
                ));
            }
            return;
        }
        let root = child_root();
        let calls = Arc::new(AtomicUsize::new(0));
        let bin = Scratch::new().unwrap();
        let manager = ToolsManager::new(format!("{}/./", bin.text()), Some(counting_fetch(&calls)));
        let candidate = bin.path().join("fd");
        let mut output = Vec::new();
        if maestro_test_process::selection().runs_case("dangling") {
            std::os::unix::fs::symlink("absent", &candidate).unwrap();
            assert_eq!(manager.get_tool_path(SearchTool::Fd).await, None);
            assert_eq!(
                manager
                    .ensure_tool(
                        SearchTool::Fd,
                        Some(ToolsManagerOutput {
                            stdout: &mut output,
                            color: false
                        })
                    )
                    .await,
                None
            );
            assert_eq!(
                output,
                b"fd not found. Offline mode enabled, skipping download.\n"
            );
        } else {
            existing_entries(&manager, &bin).await;
        }
        assert_eq!(probe_log(&root), None);
        assert_eq!(calls.load(Ordering::SeqCst), 0);
    });
}

/// Existing entries return the joined spelling without advice.
async fn existing_entries(manager: &ToolsManager, bin: &Scratch) {
    let candidate = bin.path().join("fd");
    let mut output = Vec::new();
    for kind in ["file", "directory", "symlink"] {
        match kind {
            "file" => {
                use std::os::unix::fs::PermissionsExt;
                std::fs::write(&candidate, "not executable").unwrap();
                std::fs::set_permissions(&candidate, std::fs::Permissions::from_mode(0o644))
                    .unwrap();
            }
            "directory" => std::fs::create_dir(&candidate).unwrap(),
            "symlink" => {
                bin.file("target", b"target");
                std::os::unix::fs::symlink("target", &candidate).unwrap();
            }
            _ => unreachable!(),
        }
        let expected = Some(candidate.to_str().unwrap().to_owned());
        assert_eq!(manager.get_tool_path(SearchTool::Fd).await, expected);
        assert_eq!(
            manager
                .ensure_tool(
                    SearchTool::Fd,
                    Some(ToolsManagerOutput {
                        stdout: &mut output,
                        color: false
                    })
                )
                .await,
            expected
        );
        assert!(output.is_empty());
        if kind == "directory" {
            std::fs::remove_dir(&candidate).unwrap();
        } else {
            std::fs::remove_file(&candidate).unwrap();
        }
    }
}

#[test]
fn path_probes_try_aliases_in_order_whatever_the_exit_status() {
    run(async {
        if !maestro_test_process::selection().is_child() {
            for mode in ["none", "alias", "failed-fd", "fd", "rg"] {
                let scripts = Scratch::new().unwrap();
                probe_scripts(scripts.path(), mode);
                assert!(!child(
                    "path_probes_try_aliases_in_order_whatever_the_exit_status",
                    mode,
                    scripts.path(),
                    "0"
                ));
            }
            return;
        }
        let mode = ["none", "alias", "failed-fd", "fd", "rg"]
            .into_iter()
            .find(|mode| maestro_test_process::selection().runs_case(mode))
            .expect("selected probe mode");
        let root = child_root();
        let bin = Scratch::new().unwrap();
        let calls = Arc::new(AtomicUsize::new(0));
        let manager = ToolsManager::new(bin.text(), Some(counting_fetch(&calls)));
        let expected = match mode {
            "alias" => Some("fdfind"),
            "fd" | "failed-fd" => Some("fd"),
            _ => None,
        };
        assert_eq!(
            manager.get_tool_path(SearchTool::Fd).await.as_deref(),
            expected
        );
        assert_eq!(
            probe_log(&root),
            expected.map(|name| format!("{name} --version\n"))
        );
        if expected.is_some() {
            std::fs::remove_file(root.join("probes.log")).unwrap();
        }
        let expected = (mode == "rg").then_some("rg");
        assert_eq!(
            manager.get_tool_path(SearchTool::Rg).await.as_deref(),
            expected
        );
        assert_eq!(
            probe_log(&root),
            expected.map(|name| format!("{name} --version\n"))
        );
        assert_eq!(calls.load(Ordering::SeqCst), 0);
        if mode == "alias" {
            let selected = ToolsManager::new(bin.text(), Some(counting_fetch(&calls)))
                .with_offline(Some(true));
            assert_eq!(
                selected.ensure_tool(SearchTool::Fd, None).await.as_deref(),
                Some("fdfind"),
                "an offline selection still uses the program a probe finds"
            );
            assert_eq!(probe_log(&root), Some("fdfind --version\n".to_owned()));
            assert_eq!(calls.load(Ordering::SeqCst), 0);
        }
    });
}

#[test]
fn offline_flag_stops_a_missing_install_with_advice() {
    run(async {
        if !maestro_test_process::selection().is_child() {
            let scripts = Scratch::new().unwrap();
            assert!(!child(
                "offline_flag_stops_a_missing_install_with_advice",
                "plain",
                scripts.path(),
                "YeS"
            ));
            assert!(!child(
                "offline_flag_stops_a_missing_install_with_advice",
                "styled",
                scripts.path(),
                "1"
            ));
            return;
        }
        let root = child_root();
        let calls = Arc::new(AtomicUsize::new(0));
        let bin = root.join("not-created");
        let manager = ToolsManager::new(
            bin.to_str().unwrap().to_owned(),
            Some(counting_fetch(&calls)),
        );
        let color = maestro_test_process::selection().runs_case("styled");
        for (tool, name) in [(SearchTool::Fd, "fd"), (SearchTool::Rg, "ripgrep")] {
            let mut output = Vec::new();
            assert_eq!(
                manager
                    .ensure_tool(
                        tool,
                        Some(ToolsManagerOutput {
                            stdout: &mut output,
                            color
                        })
                    )
                    .await,
                None
            );
            let line = format!("{name} not found. Offline mode enabled, skipping download.");
            let expected = if color {
                format!("\x1b[33m{line}\x1b[39m\n")
            } else {
                format!("{line}\n")
            };
            assert_eq!(String::from_utf8(output).unwrap(), expected);
            assert_eq!(manager.ensure_tool(tool, None).await, None);
        }
        assert!(!bin.exists());
        assert_eq!(calls.load(Ordering::SeqCst), 0);
    });
}

/// Configure one ordered probe scenario.
fn probe_scripts(root: &Path, mode: &str) {
    match mode {
        "alias" => script(root, "fdfind", 0),
        "failed-fd" | "fd" | "entries" => {
            script(root, "fd", if mode == "fd" { 0 } else { 7 });
            script(root, "fdfind", 0);
        }
        "rg" => script(root, "rg", 9),
        _ => {}
    }
}

#[test]
fn dropping_discovery_kills_the_running_probe() {
    run(async {
        if !maestro_test_process::selection().is_child() {
            let scripts = Scratch::new().unwrap();
            let source = scripts.file("probe.source", b"#!/usr/bin/python3\nimport os, socket\ns = socket.socket(socket.AF_UNIX)\ns.connect(os.environ['MAESTRO_SEARCH_ROOT'] + '/probe.sock')\ns.sendall(b'ready')\ns.recv(1)\n");
            assert!(
                std::process::Command::new("install")
                    .args(["-m", "0700", &source])
                    .arg(scripts.path().join("fd"))
                    .status()
                    .unwrap()
                    .success()
            );
            assert!(!child(
                "dropping_discovery_kills_the_running_probe",
                "drop",
                scripts.path(),
                "1"
            ));
            return;
        }
        let root = child_root();
        let listener = tokio::net::UnixListener::bind(root.join("probe.sock")).unwrap();
        let bin = Scratch::new().unwrap();
        let manager = ToolsManager::new(bin.text(), None);
        let mut discovery = Box::pin(manager.get_tool_path(SearchTool::Fd));
        let (mut socket, _) = tokio::select! {
            () = async { discovery.as_mut().await; } => panic!("probe exited before readiness"),
            connection = listener.accept() => connection.unwrap(),
        };
        let mut ready = [0; 5];
        socket.read_exact(&mut ready).await.unwrap();
        assert_eq!(&ready, b"ready");
        // Drop the future itself, not merely a pinned reference to it.
        drop(discovery);
        assert_eq!(
            socket.read(&mut [0]).await.unwrap(),
            0,
            "dropped probe must close its socket"
        );
    });
}
