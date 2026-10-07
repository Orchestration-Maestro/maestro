#![cfg(not(target_arch = "wasm32"))]
mod support;
use maestro_settings::{
    FileSettingsStorage, InMemorySettingsStorage, SettingsManager, SettingsScope, SettingsStorage,
};
use std::{
    io::{BufRead, BufReader, Write},
    process::{Command, Stdio},
};
use support::{Scheduler, Scratch};

fn wait_for_signal(root: &std::path::Path) {
    println!("READY");
    std::io::stdout().flush().unwrap();
    while !root.join("continue").exists() {
        std::thread::yield_now();
    }
}
struct QueuedStorage(std::path::PathBuf, std::sync::atomic::AtomicBool);
impl SettingsStorage for QueuedStorage {
    fn with_lock(
        &self,
        scope: SettingsScope,
        operation: &mut dyn FnMut(Option<&str>) -> Result<Option<String>, support::Error>,
    ) -> Result<(), support::Error> {
        let path = if scope == SettingsScope::Global {
            self.0.join("settings.json")
        } else {
            self.0.join(".maestro/settings.json")
        };
        let text = std::fs::read_to_string(&path)?;
        if self.1.load(std::sync::atomic::Ordering::SeqCst) {
            wait_for_signal(&self.0);
        }
        if let Some(next) = operation(Some(&text))? {
            std::fs::write(path, next)?;
        }
        Ok(())
    }
}
fn child_mode(name: &str) -> bool {
    if std::env::var("MAESTRO_TERMINATION_CASE").as_deref() != Ok(name) {
        return false;
    }
    let root = std::path::PathBuf::from(std::env::var_os("MAESTRO_TERMINATION_ROOT").unwrap());
    let storage = FileSettingsStorage::new(&root, &root, ".maestro");
    if name.starts_with("termination_during_queued_") {
        let storage = std::sync::Arc::new(QueuedStorage(root.clone(), false.into()));
        let scheduler = Scheduler::default();
        let manager = SettingsManager::from_storage(storage.clone(), scheduler.spawn());
        match name {
            "termination_during_queued_scalar_write" => manager.set_theme("written".into()),
            "termination_during_queued_nested_write" => {
                manager.set_compaction_enabled(false).unwrap()
            }
            "termination_during_queued_combined_write" => {
                manager.set_default_model_and_provider("provider".into(), "model".into())
            }
            "termination_during_queued_number_write" => manager.set_editor_padding_x(2.0),
            "termination_during_queued_composite_write" => {
                manager.set_enabled_models(vec!["model".to_owned()].into())
            }
            "termination_during_queued_project_write" => {
                manager.set_project_extension_paths(vec!["path".to_owned()].into())
            }
            _ => unreachable!(),
        }
        storage.1.store(true, std::sync::atomic::Ordering::SeqCst);
        scheduler.drive();
        panic!("signal was not re-raised");
    }
    if name == "termination_during_memory_callback_finishes_operation" {
        InMemorySettingsStorage::new()
            .with_lock(SettingsScope::Global, &mut |_| {
                wait_for_signal(&root);
                std::fs::write(root.join("settings.json"), "written")?;
                Ok(Some("written".into()))
            })
            .unwrap();
        panic!("signal was not re-raised");
    }

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
    let scope = if name == "termination_during_missing_project_callback_finishes_write_and_releases"
    {
        SettingsScope::Project
    } else {
        SettingsScope::Global
    };
    storage
        .with_lock(scope, &mut |_| {
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
            wait_for_signal(&root);
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
    if name != "termination_during_missing_file_callback_finishes_write_and_releases" {
        std::fs::write(&file, "before").unwrap();
    }
    std::fs::create_dir(scratch.root.join(".maestro")).unwrap();
    std::fs::write(scratch.root.join(".maestro/settings.json"), "before").unwrap();
    if name == "termination_during_missing_project_callback_finishes_write_and_releases" {
        std::fs::remove_file(scratch.root.join(".maestro/settings.json")).unwrap();
        std::fs::remove_dir(scratch.root.join(".maestro")).unwrap();
    }
    if name.starts_with("termination_during_queued_") {
        std::fs::write(&file, "{}").unwrap();
        std::fs::write(scratch.root.join(".maestro/settings.json"), "{}").unwrap();
    }
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
    if name.starts_with("termination_during_queued_") {
        let (path, expected) = match name {
            "termination_during_queued_scalar_write" => {
                (file, serde_json::json!({"theme":"written"}))
            }
            "termination_during_queued_nested_write" => {
                (file, serde_json::json!({"compaction":{"enabled":false}}))
            }
            "termination_during_queued_combined_write" => (
                file,
                serde_json::json!({"defaultProvider":"provider","defaultModel":"model"}),
            ),
            "termination_during_queued_number_write" => {
                (file, serde_json::json!({"editorPaddingX":2}))
            }
            "termination_during_queued_composite_write" => {
                (file, serde_json::json!({"enabledModels":["model"]}))
            }
            "termination_during_queued_project_write" => (
                scratch.root.join(".maestro/settings.json"),
                serde_json::json!({"extensions":["path"]}),
            ),
            _ => unreachable!(),
        };
        assert_eq!(
            serde_json::from_str::<serde_json::Value>(&std::fs::read_to_string(path).unwrap())
                .unwrap(),
            expected
        );
    } else {
        assert_eq!(
            std::fs::read_to_string(file).unwrap(),
            if matches!(
                name,
                "termination_during_locked_callback_finishes_write_and_releases"
                    | "termination_during_missing_file_callback_finishes_write_and_releases"
                    | "termination_during_memory_callback_finishes_operation"
            ) {
                "written"
            } else {
                "before"
            }
        );
        if name == "termination_during_missing_project_callback_finishes_write_and_releases" {
            assert_eq!(
                std::fs::read_to_string(scratch.root.join(".maestro/settings.json")).unwrap(),
                "written"
            );
        }
    }
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

#[cfg(unix)]
#[test]
fn termination_during_missing_file_callback_finishes_write_and_releases() {
    subprocess(
        "termination_during_missing_file_callback_finishes_write_and_releases",
        true,
    );
}

#[cfg(unix)]
#[test]
fn termination_during_missing_project_callback_finishes_write_and_releases() {
    subprocess(
        "termination_during_missing_project_callback_finishes_write_and_releases",
        true,
    );
}

#[cfg(unix)]
#[test]
fn termination_during_memory_callback_finishes_operation() {
    subprocess(
        "termination_during_memory_callback_finishes_operation",
        true,
    );
}

#[cfg(unix)]
#[test]
fn termination_during_queued_scalar_write() {
    subprocess("termination_during_queued_scalar_write", true);
}

#[cfg(unix)]
#[test]
fn termination_during_queued_nested_write() {
    subprocess("termination_during_queued_nested_write", true);
}

#[cfg(unix)]
#[test]
fn termination_during_queued_combined_write() {
    subprocess("termination_during_queued_combined_write", true);
}

#[cfg(unix)]
#[test]
fn termination_during_queued_number_write() {
    subprocess("termination_during_queued_number_write", true);
}

#[cfg(unix)]
#[test]
fn termination_during_queued_composite_write() {
    subprocess("termination_during_queued_composite_write", true);
}

#[cfg(unix)]
#[test]
fn termination_during_queued_project_write() {
    subprocess("termination_during_queued_project_write", true);
}
