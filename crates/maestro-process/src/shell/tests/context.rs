//! Child-directory executable selection through the native launch boundary.
use super::super::Launch;
use std::path::{Path, PathBuf};

#[test]
fn launch_uses_complete_child_context() {
    in_directory_context(
        "launch_uses_complete_child_context",
        "parent",
        assert_context,
    );
}

#[test]
#[cfg(windows)]
fn launch_honors_current_directory_search_policy() {
    in_directory_context(
        "launch_honors_current_directory_search_policy",
        "chosen",
        assert_lookup_policy,
    );
}

/// Reuses the same executable fixtures in a checked child with a chosen parent directory.
fn in_directory_context(name: &str, parent: &str, run: fn(&Path)) {
    const CHILD: &str = "MAESTRO_LAUNCH_CONTEXT_CHILD";
    if let Some(root) = std::env::var_os(CHILD) {
        run(&PathBuf::from(root));
        return;
    }
    let root = std::env::temp_dir().join(format!(
        "maestro-launch-context-{}-{name}",
        std::process::id()
    ));
    for (directory, text) in [("parent", "parent"), ("chosen", "chosen")] {
        let tools = root.join(directory).join("tools");
        std::fs::create_dir_all(&tools).unwrap();
        write_wrapper(&tools.join(wrapper_name()), text);
        if directory == "chosen" {
            write_wrapper(&tools.join(only_name()), text);
        }
    }
    let output = maestro_test_process::TestProcess::current(name)
        .unwrap()
        .env(CHILD, &root)
        .env("PATHEXT", ".CMD")
        .env("PATH", root.join("parent").join("tools"))
        .env_remove("NoDefaultCurrentDirectoryInExePath")
        .current_dir(root.join(parent))
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

/// A present skip-current-directory policy changes only bare-name lookup.
#[cfg(windows)]
fn assert_lookup_policy(root: &Path) {
    let chosen = root.join("chosen");
    let parent = root.join("parent").join("tools");
    write_wrapper(&chosen.join("lookup.CMD"), "chosen");
    write_wrapper(&parent.join("lookup.CMD"), "parent");
    let mut observed = Vec::new();
    for flag in [None, Some(""), Some("1")] {
        let mut environment = vec![("PATH", parent.to_str().unwrap()), ("PATHEXT", ".CMD")];
        if let Some(flag) = flag {
            environment.push(("NoDefaultCurrentDirectoryInExePath", flag));
        }
        println!("SEARCH-POLICY flag={flag:?}");
        observed.push(run_wrapper(Path::new("lookup.CMD"), &chosen, &environment));
        assert_wrapper(
            Path::new("tools/wrapper.CMD"),
            &chosen,
            &environment,
            "chosen",
        );
        assert_wrapper(
            &chosen.join("tools/wrapper.CMD"),
            &chosen,
            &environment,
            "chosen",
        );
        let Some(std::path::Component::Prefix(prefix)) = chosen.components().next() else {
            panic!("missing native drive prefix")
        };
        let drive_relative = PathBuf::from(format!(
            "{}tools\\wrapper.CMD",
            prefix.as_os_str().to_str().unwrap()
        ));
        let root_relative = chosen
            .strip_prefix(prefix.as_os_str())
            .unwrap()
            .join("tools/wrapper.CMD");
        assert_wrapper(&drive_relative, &chosen, &environment, "chosen");
        assert_wrapper(&root_relative, &chosen, &environment, "chosen");
    }
    assert_eq!(observed, ["chosen", "parent", "parent"]);
    assert_empty_search_paths(&chosen);
    assert_native_image_policy(&chosen, &parent);
}

/// Empty PATH members cannot reintroduce the default cwd search; an authored dot can.
#[cfg(windows)]
fn assert_empty_search_paths(chosen: &Path) {
    for path in ["", ";;;"] {
        let error = Launch {
            executable: "lookup.CMD".into(),
            args: vec![],
            verbatim: false,
        }
        .with_context(
            [("PATH", path), ("NoDefaultCurrentDirectoryInExePath", "1")],
            Some(chosen),
        )
        .output()
        .unwrap_err();
        assert_eq!(error.kind(), std::io::ErrorKind::NotFound);
    }
    assert_wrapper(
        Path::new("lookup.CMD"),
        chosen,
        &[("PATH", "."), ("NoDefaultCurrentDirectoryInExePath", "1")],
        "chosen",
    );
}

/// Direct executable images obey the same policy as batch wrappers.
#[cfg(windows)]
fn assert_native_image_policy(chosen: &Path, parent: &Path) {
    let image = std::env::current_exe().unwrap();
    for directory in [chosen, parent] {
        std::fs::copy(&image, directory.join("lookup.exe")).unwrap();
    }
    let chosen_image = std::fs::canonicalize(chosen.join("lookup.exe")).unwrap();
    let parent_image = std::fs::canonicalize(parent.join("lookup.exe")).unwrap();
    for flag in [None, Some(""), Some("1")] {
        let mut env = vec![("PATH", parent.to_str().unwrap())];
        if let Some(flag) = flag {
            env.push(("NoDefaultCurrentDirectoryInExePath", flag));
        }
        let actual = loaded_image("lookup.exe", chosen, &env);
        assert_eq!(
            &actual,
            if flag.is_some() {
                &parent_image
            } else {
                &chosen_image
            }
        );
    }
    let env = [("PATH", "."), ("NoDefaultCurrentDirectoryInExePath", "1")];
    for program in [
        Path::new("lookup.exe"),
        Path::new("./lookup.exe"),
        chosen_image.as_path(),
    ] {
        assert_eq!(
            loaded_image(program.to_str().unwrap(), chosen, &env),
            chosen_image
        );
    }
}

/// Observes the module loaded by the operating system through a checked test child.
#[cfg(windows)]
fn loaded_image(program: &str, cwd: &Path, environment: &[(&str, &str)]) -> PathBuf {
    let mut environment = environment.to_vec();
    environment.push(("MAESTRO_LOADED_IMAGE_PROBE", "1"));
    let output = maestro_test_process::TestProcess::named(
        maestro_test_process::test_name(loaded_image_probe).unwrap(),
    )
    .unwrap()
    .nocapture()
    .output_with(|args| {
        Launch {
            executable: program.into(),
            args: args.into(),
            verbatim: false,
        }
        .with_context(environment.iter().copied(), Some(cwd))
        .output()
    })
    .unwrap();
    assert!(output.status.success(), "{output:?}");
    let text = String::from_utf8(output.stdout).unwrap();
    let path = text
        .lines()
        .find_map(|line| line.split_once("LOADED-IMAGE ").map(|(_, path)| path))
        .unwrap();
    std::fs::canonicalize(path).unwrap()
}

/// The checked child's only effect is reporting its loaded executable image.
#[test]
#[cfg(windows)]
fn loaded_image_probe() {
    if std::env::var_os("MAESTRO_LOADED_IMAGE_PROBE").is_some() {
        println!(
            "LOADED-IMAGE {}",
            std::env::current_exe().unwrap().display()
        );
    }
}

/// Observes which fixture actually ran, not the command builder's declared operands.
fn assert_wrapper(program: &Path, cwd: &Path, environment: &[(&str, &str)], expected: &str) {
    assert_eq!(
        run_wrapper(program, cwd, environment),
        expected,
        "{program:?}"
    );
}

/// Runs one fixture and returns its independently authored output.
fn run_wrapper(program: &Path, cwd: &Path, environment: &[(&str, &str)]) -> String {
    let output = Launch {
        executable: program.to_str().unwrap().into(),
        args: vec![],
        verbatim: false,
    }
    .with_context(environment.iter().copied(), Some(cwd))
    .output()
    .unwrap();
    assert!(output.status.success(), "{output:?}");
    String::from_utf8(output.stdout).unwrap().trim().to_owned()
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
