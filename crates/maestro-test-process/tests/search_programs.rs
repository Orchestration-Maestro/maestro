//! Pinned search installations stay independent of the runner's search PATH.

use std::path::Path;

use maestro_test_directory::Scratch;
use maestro_test_process::{TestProcess, install_search_program, selection};

/// Missing exports and sources fail; a successful install keeps bytes and executable mode.
#[test]
fn search_install_preserves_the_supplied_program_and_reports_missing_inputs() {
    if selection().is_child() {
        return check_install();
    }
    let root = Scratch::new().unwrap();
    let source = root.path().join("source");
    std::fs::write(&source, b"pinned executable bytes").unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&source, std::fs::Permissions::from_mode(0o751)).unwrap();
    }
    for case in ["variable", "source", "success"] {
        let mut child = TestProcess::current(
            "search_install_preserves_the_supplied_program_and_reports_missing_inputs",
        )
        .unwrap()
        .case(case)
        .nocapture()
        .env("INSTALL_ROOT", root.path())
        .env("INSTALL_CASE", case)
        .env_remove("MAESTRO_TEST_FD");
        if case != "variable" {
            child = child.env(
                "MAESTRO_TEST_FD",
                if case == "source" {
                    root.path().join("missing")
                } else {
                    source.clone()
                },
            );
        }
        let output = child.output().unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
}

/// Exercise the public installer with the parent's isolated environment.
fn check_install() {
    let root = std::env::var_os("INSTALL_ROOT").unwrap();
    let root = Path::new(&root);
    let result = install_search_program("fd", root);
    match std::env::var("INSTALL_CASE").unwrap().as_str() {
        "variable" => assert!(result.unwrap_err().to_string().contains("MAESTRO_TEST_FD")),
        "source" => assert_eq!(result.unwrap_err().kind(), std::io::ErrorKind::NotFound),
        "success" => {
            let installed = result.unwrap();
            assert_eq!(
                installed,
                root.join(format!("fd{}", std::env::consts::EXE_SUFFIX))
            );
            assert_eq!(
                std::fs::read(&installed).unwrap(),
                b"pinned executable bytes"
            );
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                assert_eq!(
                    std::fs::metadata(installed).unwrap().permissions().mode() & 0o777,
                    0o751
                );
            }
        }
        _ => unreachable!(),
    }
}
