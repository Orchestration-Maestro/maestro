//! Checked-child proof through the existing test-process owner.
#![cfg(unix)]
use maestro_test_directory::Scratch;
use maestro_test_process::TestProcess;
use std::{fs, io};

#[test]
fn chosen_temporary_parent_is_canonical_and_errors_stay_native() {
    if maestro_test_process::selection().is_child() {
        let case = ["linked", "missing", "file"]
            .into_iter()
            .find(|case| maestro_test_process::selection().runs_case(case))
            .expect("selected directory case");
        match case {
            "linked" => {
                let root = Scratch::new().unwrap();
                assert_eq!(
                    root.path().as_os_str(),
                    fs::canonicalize(root.path()).unwrap().as_os_str()
                );
                let file = root.path().join("payload");
                fs::write(&file, "kept").unwrap();
                assert_eq!(
                    file.as_os_str(),
                    fs::canonicalize(&file).unwrap().as_os_str()
                );
                assert_eq!(fs::read_to_string(file).unwrap(), "kept");
            }
            "missing" => {
                assert_eq!(Scratch::new().unwrap_err().kind(), io::ErrorKind::NotFound);
                assert!(!std::env::temp_dir().exists());
            }
            "file" => {
                let parent = std::env::temp_dir();
                let expected = fs::create_dir(parent.join("probe")).unwrap_err().kind();
                assert_eq!(Scratch::new().unwrap_err().kind(), expected);
                assert_eq!(fs::read_to_string(parent).unwrap(), "unchanged");
            }
            _ => panic!("unexpected child"),
        }
        return;
    }
    let parent = Scratch::new().unwrap();
    let target = parent.path().join("target");
    let linked = parent.path().join("linked");
    fs::create_dir(&target).unwrap();
    std::os::unix::fs::symlink(&target, &linked).unwrap();
    let missing = parent.path().join("missing");
    let file = parent.path().join("file");
    fs::write(&file, "unchanged").unwrap();
    for (case, temporary) in [("linked", linked), ("missing", missing), ("file", file)] {
        let output =
            TestProcess::current("chosen_temporary_parent_is_canonical_and_errors_stay_native")
                .unwrap()
                .case(case)
                .temp_dir(temporary)
                .output()
                .unwrap();
        assert!(
            output.status.success(),
            "{case}: {} {}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    }
}
