//! Native self-update probes never execute update verbs.
#![cfg(test)]
#![cfg(not(target_arch = "wasm32"))]

use maestro_app::configuration::config::{NativeSelfUpdateOperations, SelfUpdateOperations};
use maestro_test_directory::Scratch;

#[cfg(unix)]
#[test]
fn native_update_probes_capture_only_helper_output() {
    use maestro_app::configuration::config::installation::{Installation, PackageLayout};
    use maestro_app::configuration::config::{SelfUpdateContext, get_self_update_command};
    use std::{fs, os::unix::fs::symlink};
    let scratch = Scratch::new().unwrap();
    let root = scratch.path().join("global");
    let package = root.join("agent");
    fs::create_dir_all(&package).unwrap();
    let alias = scratch.path().join("alias");
    symlink(&root, &alias).unwrap();
    let helper = install_helper(&scratch);
    let installation = Installation {
        module_dir: "/source".into(),
        executable: "/runtime/npm/node".into(),
        layout: PackageLayout::Package,
    };
    let dir = package.to_str().unwrap();
    let operations = NativeSelfUpdateOperations;
    let context = SelfUpdateContext {
        installation: &installation,
        package_dir: dir,
        operations: &operations,
    };
    let configured = vec![
        helper.to_string_lossy().into_owned(),
        alias.to_string_lossy().into_owned(),
    ];
    let selected = get_self_update_command(&context, "agent", Some(&configured), Some("new-agent"))
        .unwrap()
        .unwrap();
    assert_eq!(selected.command, configured[0]);
    assert_eq!(
        selected.args,
        [configured[1].as_str(), "install", "-g", "new-agent"]
    );
    assert_eq!(
        selected.steps.unwrap()[0].args,
        [configured[1].as_str(), "uninstall", "-g", "agent"]
    );
    let failed = vec![configured[0].clone(), "fail".into()];
    let error = get_self_update_command(&context, "agent", Some(&failed), None).unwrap_err();
    assert_eq!(
        error.to_string(),
        format!("Failed to run {} fail root -g: query denied", configured[0])
    );
    let missing = vec![scratch.path().join("absent").to_string_lossy().into_owned()];
    let source_error = operations
        .run_command_sync(&missing[0], &["root".into(), "-g".into()])
        .unwrap_err();
    let native_reason = std::error::Error::source(&source_error)
        .unwrap()
        .to_string();
    let error = get_self_update_command(&context, "agent", Some(&missing), None).unwrap_err();
    assert_eq!(
        error.to_string(),
        format!("Failed to run {} root -g: {native_reason}", missing[0])
    );
    assert!(std::error::Error::source(&error).is_some());
}

#[cfg(unix)]
fn install_helper(scratch: &Scratch) -> std::path::PathBuf {
    use std::{fs::OpenOptions, io::Write};
    let source = scratch.path().join("query-source");
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&source)
        .unwrap();
    file.write_all(b"#!/bin/sh\nroot=$1\nshift\n[ \"$*\" = 'root -g' ] || exit 91\nif [ \"$root\" = fail ]; then printf ' query denied\\n' >&2; exit 7; fi\nprintf ' \\357\\273\\277%s\\n' \"$root\"\n").unwrap();
    drop(file);
    let target = scratch.path().join("query-helper");
    assert!(
        std::process::Command::new("install")
            .args(["-m", "0700"])
            .arg(&source)
            .arg(&target)
            .status()
            .unwrap()
            .success()
    );
    target
}

#[cfg(unix)]
#[test]
fn native_update_access_matches_system_decision() {
    use maestro_app::configuration::config::installation::{Installation, PackageLayout};
    use maestro_app::configuration::config::{
        SelfUpdateContext, get_self_update_command, get_self_update_unavailable_instruction,
    };
    use std::{fs, os::unix::fs::PermissionsExt};
    let scratch = Scratch::new().unwrap();
    let parent = scratch.path().join("node_modules");
    let package = parent.join("agent");
    fs::create_dir_all(&package).unwrap();
    let helper = install_helper(&scratch);
    let configured = vec![
        helper.to_string_lossy().into_owned(),
        parent.to_string_lossy().into_owned(),
    ];
    let installation = Installation {
        module_dir: "/source".into(),
        executable: "/runtime/npm/node".into(),
        layout: PackageLayout::Package,
    };
    let operations = NativeSelfUpdateOperations;
    let context = SelfUpdateContext {
        installation: &installation,
        package_dir: package.to_str().unwrap(),
        operations: &operations,
    };
    for (package_mode, parent_mode) in [(0o700, 0o700), (0o500, 0o700), (0o700, 0o500)] {
        fs::set_permissions(&package, fs::Permissions::from_mode(package_mode)).unwrap();
        fs::set_permissions(&parent, fs::Permissions::from_mode(parent_mode)).unwrap();
        let package_access = rustix::fs::access(&package, rustix::fs::Access::WRITE_OK).is_ok();
        let parent_access = rustix::fs::access(&parent, rustix::fs::Access::WRITE_OK).is_ok();
        assert_eq!(
            operations
                .check_write_access(package.to_str().unwrap())
                .is_ok(),
            package_access
        );
        assert_eq!(
            operations
                .check_write_access(parent.to_str().unwrap())
                .is_ok(),
            parent_access
        );
        let command = get_self_update_command(&context, "agent", Some(&configured), None).unwrap();
        assert_eq!(command.is_some(), package_access && parent_access);
        let guidance =
            get_self_update_unavailable_instruction(&context, "agent", Some(&configured), None)
                .unwrap();
        assert_eq!(
            guidance.contains("not writable"),
            !(package_access && parent_access)
        );
    }
    fs::set_permissions(&parent, fs::Permissions::from_mode(0o700)).unwrap();
    fs::set_permissions(&package, fs::Permissions::from_mode(0o700)).unwrap();
}

#[cfg(windows)]
#[test]
fn native_windows_update_access_matches_directory_rule() {
    use std::fs;
    let scratch = Scratch::new().unwrap();
    let operations = NativeSelfUpdateOperations;
    let dir = scratch.path().join("directory");
    fs::create_dir(&dir).unwrap();
    let file = scratch.path().join("file");
    drop(
        fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&file)
            .unwrap(),
    );
    for path in [&dir, &file] {
        assert!(
            operations
                .check_write_access(path.to_str().unwrap())
                .is_ok()
        );
        let original_permissions = fs::metadata(path).unwrap().permissions();
        let mut permissions = original_permissions.clone();
        permissions.set_readonly(true);
        fs::set_permissions(path, permissions).unwrap();
        assert_eq!(
            operations
                .check_write_access(path.to_str().unwrap())
                .is_ok(),
            path == &dir
        );
        fs::set_permissions(path, original_permissions).unwrap();
    }
    assert!(
        operations
            .check_write_access(scratch.path().join("absent").to_str().unwrap())
            .is_err()
    );
    windows_selection_uses_native_access(&scratch);
    assert_eq!(fs::read(&file).unwrap(), b"");
    assert_eq!(fs::read_dir(&dir).unwrap().count(), 0);
}

#[cfg(windows)]
fn windows_selection_uses_native_access(scratch: &Scratch) {
    use maestro_app::configuration::config::installation::{Installation, PackageLayout};
    use maestro_app::configuration::config::{SelfUpdateContext, get_self_update_command};
    let package = scratch.path().join("directory").join("agent");
    std::fs::create_dir(&package).unwrap();
    let operations = WindowsQuery {
        root: scratch
            .path()
            .join("directory")
            .to_string_lossy()
            .into_owned(),
    };
    let installation = Installation {
        module_dir: r"C:\source".into(),
        executable: r"C:\runtime\npm\node.exe".into(),
        layout: PackageLayout::Package,
    };
    let context = SelfUpdateContext {
        installation: &installation,
        package_dir: package.to_str().unwrap(),
        operations: &operations,
    };
    let configured = vec!["query-only".into()];
    assert!(
        get_self_update_command(&context, "agent", Some(&configured), None)
            .unwrap()
            .is_some()
    );
    let original_permissions = std::fs::metadata(&package).unwrap().permissions();
    let mut permissions = original_permissions.clone();
    permissions.set_readonly(true);
    std::fs::set_permissions(&package, permissions).unwrap();
    assert!(
        get_self_update_command(&context, "agent", Some(&configured), None)
            .unwrap()
            .is_some()
    );
    std::fs::set_permissions(&package, original_permissions).unwrap();
    std::fs::remove_dir(&package).unwrap();
}

#[cfg(windows)]
struct WindowsQuery {
    root: String,
}
#[cfg(windows)]
impl SelfUpdateOperations for WindowsQuery {
    fn is_windows(&self) -> bool {
        true
    }
    fn home_dir(&self) -> std::io::Result<String> {
        unreachable!()
    }
    fn check_write_access(&self, path: &str) -> std::io::Result<()> {
        NativeSelfUpdateOperations.check_write_access(path)
    }
    fn run_command_sync(
        &self,
        command: &str,
        args: &[String],
    ) -> Result<std::process::Output, maestro_process::SpawnSyncError> {
        use std::os::windows::process::ExitStatusExt;
        assert_eq!(command, "query-only");
        assert_eq!(args, ["root", "-g"]);
        Ok(std::process::Output {
            status: std::process::ExitStatus::from_raw(0),
            stdout: self.root.as_bytes().to_vec(),
            stderr: Vec::new(),
        })
    }
}
