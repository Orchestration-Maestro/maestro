#![cfg(not(target_arch = "wasm32"))]
mod support;
use maestro_settings::{FileSettingsStorage, SettingsScope, SettingsStorage};
use std::{
    io::{BufRead, BufReader, Write},
    process::{Command, Stdio},
};
use support::Scratch;

fn child_mode(name: &str) -> bool {
    if std::env::var("MAESTRO_TERMINATION_CASE").as_deref() != Ok(name) {
        return false;
    }
    let root = std::path::PathBuf::from(std::env::var_os("MAESTRO_TERMINATION_ROOT").unwrap());
    let storage = FileSettingsStorage::new(&root, &root, ".maestro");
    if name == "natural_exit_while_other_thread_holds_lease_removes_directory" {
        let (ready, wait) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            storage
                .with_lock(SettingsScope::Global, &mut |_| {
                    ready.send(()).unwrap();
                    loop {
                        std::thread::park();
                    }
                })
                .unwrap();
        });
        wait.recv().unwrap();
        return true;
    }
    if name == "termination_without_active_section_keeps_default_signal" {
        storage
            .with_lock(SettingsScope::Global, &mut |_| Ok(None))
            .unwrap();
        println!("READY");
        std::io::stdout().flush().unwrap();
        loop {
            std::thread::park();
        }
    }
    storage
        .with_lock(SettingsScope::Global, &mut |_| {
            if name == "process_exit_cleans_every_held_lease" {
                storage
                    .with_lock(SettingsScope::Project, &mut |_| std::process::exit(0))
                    .unwrap();
                unreachable!();
            }
            if name == "process_exit_inside_locked_callback_removes_lease" {
                std::process::exit(0);
            }
            if name == "process_exit_from_another_thread_removes_lease" {
                std::thread::spawn(|| std::process::exit(0)).join().unwrap();
                unreachable!();
            }
            println!("READY");
            std::io::stdout().flush().unwrap();
            while !root.join("continue").exists() {
                std::thread::yield_now();
            }
            Ok(Some("written".into()))
        })
        .unwrap();
    // Deferred signals must have terminated the process before this return.
    panic!("signal was not re-raised");
}
fn subprocess(name: &str, signal: bool) {
    if child_mode(name) {
        return;
    }
    let scratch = Scratch::new();
    let file = scratch.root.join("settings.json");
    std::fs::write(&file, "before").unwrap();
    std::fs::create_dir(scratch.root.join(".maestro")).unwrap();
    std::fs::write(scratch.root.join(".maestro/settings.json"), "before").unwrap();
    let mut child = Command::new(std::env::current_exe().unwrap())
        .args(["--exact", name, "--nocapture"])
        .env("MAESTRO_TERMINATION_CASE", name)
        .env("MAESTRO_TERMINATION_ROOT", &scratch.root)
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    if signal {
        let mut output = BufReader::new(child.stdout.take().unwrap());
        let mut line = String::new();
        loop {
            assert_ne!(
                output.read_line(&mut line).unwrap(),
                0,
                "child exited before READY"
            );
            if line.trim() == "READY" {
                break;
            }
            line.clear();
        }
        assert!(
            Command::new("kill")
                .args(["-TERM", &child.id().to_string()])
                .status()
                .unwrap()
                .success()
        );
        std::fs::write(scratch.root.join("continue"), "").unwrap();
    }
    let status = child.wait().unwrap();
    if signal {
        #[cfg(unix)]
        {
            use std::os::unix::process::ExitStatusExt;
            assert_eq!(status.signal(), Some(15));
        }
    } else {
        assert_eq!(status.code(), Some(0));
    }
    assert!(!scratch.root.join("settings.json.lock").exists());
    assert!(!scratch.root.join(".maestro/settings.json.lock").exists());
    assert_eq!(
        std::fs::read_to_string(file).unwrap(),
        if name == "termination_during_locked_callback_finishes_write_and_releases" {
            "written"
        } else {
            "before"
        }
    );
}
#[test]
fn process_exit_inside_locked_callback_removes_lease() {
    subprocess("process_exit_inside_locked_callback_removes_lease", false);
}
#[test]
fn process_exit_from_another_thread_removes_lease() {
    subprocess("process_exit_from_another_thread_removes_lease", false);
}
#[cfg(unix)]
#[test]
fn termination_during_locked_callback_finishes_write_and_releases() {
    subprocess(
        "termination_during_locked_callback_finishes_write_and_releases",
        true,
    );
}
#[cfg(unix)]
#[test]
fn termination_without_active_section_keeps_default_signal() {
    subprocess(
        "termination_without_active_section_keeps_default_signal",
        true,
    );
}

#[test]
fn natural_exit_while_other_thread_holds_lease_removes_directory() {
    subprocess(
        "natural_exit_while_other_thread_holds_lease_removes_directory",
        false,
    );
}
#[test]
fn process_exit_cleans_every_held_lease() {
    subprocess("process_exit_cleans_every_held_lease", false);
}
