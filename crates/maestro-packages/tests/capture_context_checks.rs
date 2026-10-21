//! Child-directory selection through both asynchronous package operations.
#![cfg(test)]
#![cfg(not(target_arch = "wasm32"))]
use maestro_packages::{CommandCaptureOptions, NativePackageOperations, PackageOperations};
use std::{path::Path, rc::Rc};

#[test]
fn package_commands_resolve_in_child_directory() {
    in_directory_context(
        "package_commands_resolve_in_child_directory",
        "parent",
        assert_operations,
    );
}

#[test]
#[cfg(windows)]
fn capture_honors_current_directory_search_policy() {
    in_directory_context(
        "capture_honors_current_directory_search_policy",
        "chosen",
        assert_lookup_policy,
    );
}

/// Runs either operation witness with the same native wrapper fixtures.
fn in_directory_context(name: &str, parent: &str, run: fn(&Path)) {
    const CHILD: &str = "MAESTRO_PACKAGE_CONTEXT_CHILD";
    if let Some(root) = std::env::var_os(CHILD) {
        run(Path::new(&root));
        return;
    }
    let scratch = crate::scratch::Scratch::new().unwrap();
    for (directory, text, status) in [("parent", "parent", 7), ("chosen", "chosen", 0)] {
        let tools = scratch.path().join(directory).join("tools");
        std::fs::create_dir_all(&tools).unwrap();
        #[cfg(windows)]
        std::fs::write(
            tools.join("wrapper.cmd"),
            format!("@echo {text}\r\n@exit /b {status}\r\n"),
        )
        .unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let path = tools.join("wrapper");
            std::fs::write(
                &path,
                format!("#!/bin/sh\nprintf '%s\\n' '{text}'\nexit {status}\n"),
            )
            .unwrap();
            std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o700)).unwrap();
        }
    }
    let output = maestro_test_process::TestProcess::current(name)
        .unwrap()
        .env(CHILD, scratch.path())
        .env_remove("NoDefaultCurrentDirectoryInExePath")
        .current_dir(scratch.path().join(parent))
        .output()
        .unwrap();
    assert!(output.status.success(), "{output:?}");
}

/// Capture must return the child wrapper's bytes; inherited execution its exit code.
fn assert_operations(root: &Path) {
    let local = Rc::new(tokio::task::LocalSet::new());
    let operations = NativePackageOperations::new(Rc::new(|| false), &local);
    let cwd = root.join("chosen");
    let cwd = cwd.to_str().unwrap();
    let programs: &[&str] = if cfg!(windows) {
        &["tools\\wrapper.cmd", "./tools/wrapper.cmd"]
    } else {
        &["./tools/wrapper"]
    };
    let runtime = crate::runtime::runtime();
    for &program in programs {
        let result = runtime.block_on(operations.run_command_capture(
            program,
            &[],
            CommandCaptureOptions {
                cwd: Some(cwd),
                timeout: None,
                env: &[],
            },
        ));
        assert_eq!(result.unwrap(), "chosen");
        assert_eq!(
            runtime
                .block_on(operations.run_command(program, &[], Some(cwd)))
                .unwrap(),
            Some(0)
        );
    }
}

/// Capture overrides must honor all three states of the native search policy.
#[cfg(windows)]
fn assert_lookup_policy(root: &Path) {
    let chosen = root.join("chosen");
    let parent = root.join("parent").join("tools");
    std::fs::write(chosen.join("lookup.cmd"), "@echo chosen\r\n").unwrap();
    std::fs::write(parent.join("lookup.cmd"), "@echo parent\r\n").unwrap();
    let local = Rc::new(tokio::task::LocalSet::new());
    let operations = NativePackageOperations::new(Rc::new(|| false), &local);
    let runtime = crate::runtime::runtime();
    let mut observed = Vec::new();
    for flag in [None, Some(""), Some("1")] {
        let mut env = vec![("PATH", parent.to_str().unwrap()), ("PATHEXT", ".CMD")];
        if let Some(flag) = flag {
            env.push(("NoDefaultCurrentDirectoryInExePath", flag));
        }
        println!("CAPTURE-SEARCH-POLICY flag={flag:?}");
        let result = runtime.block_on(operations.run_command_capture(
            "lookup.cmd",
            &[],
            CommandCaptureOptions {
                cwd: Some(chosen.to_str().unwrap()),
                timeout: None,
                env: &env,
            },
        ));
        observed.push(result.unwrap());
    }
    assert_eq!(observed, ["chosen", "parent", "parent"]);
}
