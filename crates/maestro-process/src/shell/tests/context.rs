//! Child-directory executable selection through the native launch boundary.
use super::super::Launch;
use std::path::{Path, PathBuf};

#[test]
fn launch_uses_complete_child_context() {
    const CHILD: &str = "MAESTRO_LAUNCH_CONTEXT_CHILD";
    if let Some(root) = std::env::var_os(CHILD) {
        assert_context(&PathBuf::from(root));
        return;
    }
    let root = std::env::temp_dir().join(format!("maestro-launch-context-{}", std::process::id()));
    for (directory, text) in [("parent", "parent"), ("chosen", "chosen")] {
        let tools = root.join(directory).join("tools");
        std::fs::create_dir_all(&tools).unwrap();
        write_wrapper(&tools.join(wrapper_name()), text);
    }
    let output = maestro_test_process::TestProcess::current("launch_uses_complete_child_context")
        .unwrap()
        .env(CHILD, &root)
        .env("PATHEXT", ".CMD")
        .current_dir(root.join("parent"))
        .output()
        .unwrap();
    std::fs::remove_dir_all(root).unwrap();
    assert!(output.status.success(), "{output:?}");
}

/// Native executable name used in both directories.
fn wrapper_name() -> &'static str {
    if cfg!(windows) {
        "wrapper.CMD"
    } else {
        "wrapper"
    }
}

/// Creates a small nonrecursive native executable.
fn write_wrapper(path: &Path, text: &str) {
    #[cfg(windows)]
    std::fs::write(path, format!("@echo {text}\r\n")).unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::write(path, format!("#!/bin/sh\nprintf '%s\\n' '{text}'\n")).unwrap();
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o700)).unwrap();
    }
}

/// Rejects resolving in the parent before cwd, including relative PATH entries.
fn assert_context(root: &Path) {
    let chosen = root.join("chosen");
    let relative = Path::new("tools").join(wrapper_name());
    for program in [
        relative,
        chosen.join("tools").join(wrapper_name()),
        PathBuf::from("wrapper"),
    ] {
        let mut command = Launch {
            executable: program.to_str().unwrap().into(),
            args: vec![],
            verbatim: false,
        }
        .command_with_env([("PATH", "tools"), ("PATHEXT", ".CMD")])
        .unwrap();
        let output = command.current_dir(&chosen).output().unwrap();
        assert!(output.status.success(), "{output:?}");
        assert_eq!(
            String::from_utf8(output.stdout).unwrap().trim(),
            "chosen",
            "{program:?}"
        );
    }
}
