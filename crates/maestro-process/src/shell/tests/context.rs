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
        if directory == "chosen" {
            write_wrapper(&tools.join(only_name()), text);
            std::fs::create_dir(tools.join("nonfile.CMD")).unwrap();
        }
    }
    let output = maestro_test_process::TestProcess::current("launch_uses_complete_child_context")
        .unwrap()
        .env(CHILD, &root)
        .env("PATHEXT", ".CMD")
        .env("PATH", root.join("parent").join("tools"))
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

/// Wrapper present only in the child directory.
fn only_name() -> &'static str {
    if cfg!(windows) { "only.CMD" } else { "only" }
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
    let programs = vec![
        relative,
        chosen.join("tools").join(wrapper_name()),
        PathBuf::from("wrapper"),
        Path::new("./tools").join(wrapper_name()),
        Path::new("tools").join(only_name()),
        PathBuf::from("only"),
    ];
    for program in programs {
        assert_wrapper(
            &program,
            &chosen,
            &[("PATH", "tools"), ("PATHEXT", ".CMD")],
            "chosen",
        );
    }
    assert_wrapper(
        Path::new("tools").join(wrapper_name()).as_path(),
        Path::new("../chosen"),
        &[("PATH", "tools")],
        "chosen",
    );
    #[cfg(windows)]
    assert_windows_paths(&chosen);
    assert_wrapper(Path::new("wrapper"), &chosen, &[], "parent");
    let error = Launch {
        executable: "wrapper".into(),
        args: vec![],
        verbatim: false,
    }
    .with_context(std::iter::empty::<(&str, &str)>(), Some(&chosen))
    .clear_environment()
    .output()
    .unwrap_err();
    assert_eq!(error.kind(), std::io::ErrorKind::NotFound);
}

/// Covers the child's drive, root-relative paths and canonical directory spelling.
#[cfg(windows)]
fn assert_windows_paths(chosen: &Path) {
    let Some(std::path::Component::Prefix(prefix)) = chosen.components().next() else {
        panic!("the native temporary directory must have a Windows prefix");
    };
    let drive_relative = PathBuf::from(format!(
        "{}tools\\{}",
        prefix.as_os_str().to_str().unwrap(),
        wrapper_name()
    ));
    let root_relative = chosen
        .strip_prefix(prefix.as_os_str())
        .unwrap()
        .join("tools")
        .join(wrapper_name());
    for program in [drive_relative.as_path(), root_relative.as_path()] {
        assert_wrapper(program, chosen, &[("PATHEXT", ".CMD")], "chosen");
    }
    assert_wrapper(
        &drive_relative,
        &std::fs::canonicalize(chosen).unwrap(),
        &[("PATHEXT", ".CMD")],
        "chosen",
    );
}

/// Observes which fixture actually ran, not the command builder's declared operands.
fn assert_wrapper(program: &Path, cwd: &Path, environment: &[(&str, &str)], expected: &str) {
    let output = Launch {
        executable: program.to_str().unwrap().into(),
        args: vec![],
        verbatim: false,
    }
    .with_context(environment.iter().copied(), Some(cwd))
    .output()
    .unwrap();
    assert!(output.status.success(), "{output:?}");
    assert_eq!(
        String::from_utf8(output.stdout).unwrap().trim(),
        expected,
        "{program:?}"
    );
}

#[test]
fn launch_rejects_missing_files_and_directories() {
    let root = std::env::temp_dir().join(format!("maestro-launch-missing-{}", std::process::id()));
    std::fs::create_dir(&root).unwrap();
    std::fs::create_dir(root.join("nonfile.CMD")).unwrap();
    for program in ["./missing", "./nonfile.CMD"] {
        let error = Launch {
            executable: program.into(),
            args: vec![],
            verbatim: false,
        }
        .with_context([("PATH", "")], Some(&root))
        .output()
        .unwrap_err();
        assert!(matches!(
            error.kind(),
            std::io::ErrorKind::NotFound | std::io::ErrorKind::PermissionDenied
        ));
    }
    std::fs::remove_dir_all(root).unwrap();
}
