//! Child-directory selection through both asynchronous package operations.
#![cfg(test)]
use maestro_packages::{CommandCaptureOptions, NativePackageOperations, PackageOperations};
use std::{path::Path, rc::Rc};

#[test]
fn package_commands_resolve_in_child_directory() {
    const CHILD: &str = "MAESTRO_PACKAGE_CONTEXT_CHILD";
    if let Some(root) = std::env::var_os(CHILD) {
        assert_operations(Path::new(&root));
        return;
    }
    let scratch = crate::scratch::Scratch::new().unwrap();
    for (directory, text, status) in [("parent", "parent", 7), ("chosen", "chosen", 0)] {
        let tools = scratch.0.join(directory).join("tools");
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
    let output =
        maestro_test_process::TestProcess::current("package_commands_resolve_in_child_directory")
            .unwrap()
            .env(CHILD, &scratch.0)
            .current_dir(scratch.0.join("parent"))
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
