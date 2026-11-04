//! Windows command lookup through the public capture environment.
#![cfg(test)]
#![cfg(windows)]

use maestro_packages::{CommandCaptureOptions, NativePackageOperations, PackageOperations};

/// Variable carrying the scratch directory a child fixture works in.
const PATH_ROOT: &str = "MAESTRO_CAPTURE_PATH_ROOT";

#[test]
fn capture_resolves_override_path_before_parent_decoys() {
    if let Some(directory) = std::env::var_os(PATH_ROOT) {
        let chosen = std::path::PathBuf::from(directory).join("chosen");
        let path = chosen.to_str().unwrap();
        let local = std::rc::Rc::new(tokio::task::LocalSet::new());
        let operations = NativePackageOperations::new(std::rc::Rc::new(|| false), &local);
        for (command, env, expected) in [
            (
                "shared",
                vec![("Path", path), ("pathext", ".BAT")],
                "chosen",
            ),
            ("only", vec![("PATH", path), ("PATHEXT", ".BAT")], "chosen"),
            ("shared", vec![], "parent"),
        ] {
            let result = crate::runtime::runtime().block_on(operations.run_command_capture(
                command,
                &[],
                CommandCaptureOptions {
                    cwd: None,
                    timeout: None,
                    env: &env,
                },
            ));
            assert_eq!(result.unwrap(), expected, "{command}");
        }
        return;
    }
    let scratch = crate::scratch::Scratch::new().unwrap();
    let parent = scratch.path().join("parent");
    let chosen = scratch.path().join("chosen");
    std::fs::create_dir(&parent).unwrap();
    std::fs::create_dir(&chosen).unwrap();
    std::fs::write(parent.join("shared.CMD"), "@echo parent\r\n").unwrap();
    for name in ["shared.BAT", "only.BAT"] {
        std::fs::write(chosen.join(name), "@echo chosen\r\n").unwrap();
    }
    let output = maestro_test_process::TestProcess::current(
        "capture_resolves_override_path_before_parent_decoys",
    )
    .unwrap()
    .case("native")
    .env(PATH_ROOT, scratch.path())
    .env("PATH", &parent)
    .env("PATHEXT", ".CMD")
    .output()
    .unwrap();
    assert!(output.status.success(), "{output:?}");
}
