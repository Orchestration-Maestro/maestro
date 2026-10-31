//! Native self-update probes never execute update verbs.
#![cfg(test)]
#![cfg(not(target_arch = "wasm32"))]

use maestro_app::configuration::config::SelfUpdateOperations;
#[cfg(unix)]
use maestro_app::configuration::config::{
    NativeSelfUpdateOperations, SelfUpdateCommand, SelfUpdateContext, get_self_update_command,
    get_self_update_unavailable_instruction,
};
use maestro_process::SpawnSyncError;
use maestro_test_directory::Scratch;
use std::process::Output;

/// Guidance for an installation no evidenced global root holds.
#[cfg(unix)]
const UNMANAGED: &str = "This installation is not managed by a global npm install. Update it with \
    the package manager, wrapper, or source checkout that provides it.";

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
    let alias = configured[1].as_str();
    let expect = |verb, package| [alias, verb, "-g", "--ignore-scripts", package];
    assert_eq!(selected.args, expect("install", "new-agent"));
    assert_eq!(
        selected.steps.unwrap()[0].args,
        [alias, "uninstall", "-g", "agent"]
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
    use std::fs;
    let scratch = Scratch::new().unwrap();
    let parent = scratch.path().join("node_modules");
    let package = parent.join("agent");
    fs::create_dir_all(&package).unwrap();
    let configured = vec![
        install_helper(&scratch).to_string_lossy().into_owned(),
        parent.to_string_lossy().into_owned(),
    ];
    let installation = Installation {
        module_dir: "/source".into(),
        executable: "/runtime/npm/node".into(),
        layout: PackageLayout::Package,
    };
    let context = SelfUpdateContext {
        installation: &installation,
        package_dir: package.to_str().unwrap(),
        operations: &NativeSelfUpdateOperations,
    };
    // A writable package and parent admit the command and keep the ordinary guidance.
    let (command, guidance) = observed(&context, &configured);
    let command = command.expect("a writable install selects an update");
    assert_eq!(guidance, UNMANAGED);
    let unwritable = format!(
        "This installation is managed by a global npm install, but the install path is not \
         writable. Update it yourself with: {}",
        command.display
    );
    // Each refused write withholds the command on its own, beside the other permitted one.
    for path in [&package, &parent] {
        let Some(refused) = maestro_test_directory::deny(path, 0o500).unwrap() else {
            // A privileged run is not refused the write, so it cannot state these rows.
            return;
        };
        let (command, guidance) = observed(&context, &configured);
        assert_eq!(command, None, "a refused write on {}", path.display());
        assert_eq!(guidance, unwritable);
        drop(refused);
    }
    // Ownership was observed before the query, so a package the query removes fails the
    // first access and reports the installation as unmanaged.
    let context = SelfUpdateContext {
        operations: &Removing { package: &package },
        ..context
    };
    let (command, guidance) = observed(&context, &configured);
    assert_eq!(command, None, "a package removed by its own query");
    assert_eq!(guidance, UNMANAGED);
}

#[cfg(unix)]
/// The command and guidance public selection reports for one context.
fn observed(
    context: &SelfUpdateContext<'_>,
    configured: &[String],
) -> (Option<SelfUpdateCommand>, String) {
    let command = get_self_update_command(context, "agent", Some(configured), None).unwrap();
    let guidance =
        get_self_update_unavailable_instruction(context, "agent", Some(configured), None).unwrap();
    (command, guidance)
}

#[cfg(unix)]
/// Removes the qualifying package while the query runs, after ownership was observed.
struct Removing<'a> {
    /// Package directory to remove.
    package: &'a std::path::Path,
}

#[cfg(unix)]
impl SelfUpdateOperations for Removing<'_> {
    fn is_windows(&self) -> bool {
        false
    }
    fn run_command_sync(&self, command: &str, args: &[String]) -> Result<Output, SpawnSyncError> {
        let output = NativeSelfUpdateOperations.run_command_sync(command, args)?;
        std::fs::remove_dir_all(self.package).unwrap();
        Ok(output)
    }
}

#[cfg(windows)]
#[test]
fn native_windows_update_access_matches_directory_rule() {
    use std::fs;
    let scratch = Scratch::new().unwrap();
    let directory = scratch.path().join("directory");
    fs::create_dir(&directory).unwrap();
    let package = directory.join("agent");
    fs::create_dir(&package).unwrap();
    let file = scratch.path().join("file");
    drop(
        fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&file)
            .unwrap(),
    );
    // A directory admits the command with or without its read-only attribute; a regular
    // file admits it only while that attribute is clear.
    for (path, admitted) in [(&package, [true, true]), (&file, [true, false])] {
        let attributes = fs::metadata(path).unwrap().permissions();
        for (state, read_only) in [false, true].into_iter().enumerate() {
            let mut attributes = attributes.clone();
            attributes.set_readonly(read_only);
            fs::set_permissions(path, attributes).unwrap();
            assert_eq!(
                selects(path),
                admitted[state],
                "{} read-only {read_only}",
                path.display()
            );
        }
        fs::set_permissions(path, attributes).unwrap();
    }
    // An absent package selects nothing.
    assert!(!selects(&scratch.path().join("absent")));
    // Qualification created or modified no probe file.
    assert_eq!(fs::read(&file).unwrap(), b"");
    assert_eq!(fs::read_dir(&directory).unwrap().count(), 1);
}

#[cfg(windows)]
/// Whether public selection offers the automatic command for `package`, where the query
/// reports the directory holding it as the global root.
fn selects(package: &std::path::Path) -> bool {
    use maestro_app::configuration::config::installation::{Installation, PackageLayout};
    use maestro_app::configuration::config::{SelfUpdateContext, get_self_update_command};
    let operations = WindowsQuery {
        root: package.parent().unwrap().to_string_lossy().into_owned(),
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
    get_self_update_command(&context, "agent", Some(&configured), None)
        .unwrap()
        .is_some()
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
    fn run_command_sync(&self, command: &str, args: &[String]) -> Result<Output, SpawnSyncError> {
        use std::os::windows::process::ExitStatusExt;
        assert_eq!(command, "query-only");
        assert_eq!(args, ["root", "-g"]);
        Ok(Output {
            status: std::process::ExitStatus::from_raw(0),
            stdout: self.root.as_bytes().to_vec(),
            stderr: Vec::new(),
        })
    }
}

#[cfg(windows)]
#[test]
fn windows_update_steps_receive_literal_arguments() {
    use crate::support::run_io;
    use maestro_app::configuration::config::NativeSelfUpdateOperations;
    use std::fs;
    let scratch = Scratch::new().unwrap();
    let report = scratch.path().join("arguments.txt");
    let shim = scratch.path().join("shim.cmd");
    // The shim reports the operands it received: `%~2` without its own quoting
    // and `%3` exactly as written, which tells an empty operand from none.
    let script = format!(
        "@echo off\r\n(echo second=\"[%~2]\"\r\necho marker=%3) > \"{}\"\r\n",
        report.display()
    );
    fs::write(&shim, script).unwrap();
    let command = shim.to_str().unwrap().to_owned();
    let args = ["--prefix".to_owned(), r"C:\R&D".to_owned(), String::new()];
    let status = run_io(NativeSelfUpdateOperations.run_command(&command, &args)).unwrap();
    assert!(status.success());
    assert_eq!(
        fs::read_to_string(&report)
            .unwrap()
            .lines()
            .collect::<Vec<_>>(),
        [r#"second="[C:\R&D]""#, "marker=\"\""]
    );
}
