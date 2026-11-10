//! The pinned search program resolver accepts only the supplied directories' exact programs.
use super::*;
use maestro_test_directory::Scratch;
use std::ffi::OsStr;
use std::path::Path;

/// Installs the native fixture as `fd` below `root`, printing `stdout` and ending with `code`.
fn fixture(root: &Path, stdout: &[u8], code: u8) -> std::io::Result<()> {
    let record = serde_json::json!({
        "scenario": "output",
        "stdout": stdout,
        "stderr": [],
        "code": code,
    });
    let entry = root.join(format!("fd{}", std::env::consts::EXE_SUFFIX));
    maestro_test_process::install_command(&entry, &record.to_string())
}

/// Empty and nonexistent search directories never fall back to ambient PATH.
#[test]
fn pinned_search_rejects_missing_executable() {
    let root = Scratch::new().unwrap();
    for search in [OsStr::new(""), root.path().as_os_str()] {
        assert_eq!(
            pinned_search_tool("absent-search-tool", "fd", "1.2.3", search),
            Err("absent-search-tool: no executable on PATH".into())
        );
    }
}

/// The supplied absolute directory wins regardless of ambient HOME or PATH.
#[test]
fn pinned_search_accepts_matching_identity_and_version() {
    let root = Scratch::new().unwrap();
    fixture(root.path(), b"fd 1.2.3\n", 0).unwrap();
    let path = root
        .path()
        .join(format!("fd{}", std::env::consts::EXE_SUFFIX));
    assert_eq!(
        pinned_search_tool("fd", "fd", "1.2.3", root.path().as_os_str()).unwrap(),
        path.to_str().unwrap()
    );
}

/// A matching version alone does not establish executable identity or success.
#[test]
fn pinned_search_rejects_failed_or_mismatched_probes() {
    let root = Scratch::new().unwrap();
    for (stdout, code) in [
        (&b"fd 1.2.3"[..], 1),
        (b"other 1.2.3", 0),
        (b"fd 0.0.0", 0),
        (b"", 0),
    ] {
        fixture(root.path(), stdout, code).unwrap();
        assert!(
            pinned_search_tool("fd", "fd", "1.2.3", root.path().as_os_str())
                .unwrap_err()
                .starts_with("fd: expected fd 1.2.3, got ")
        );
    }
}

/// Spawn errors remain distinct from version mismatches.
#[test]
fn pinned_search_reports_probe_spawn_failure() {
    let root = Scratch::new().unwrap();
    std::fs::write(
        root.path()
            .join(format!("fd{}", std::env::consts::EXE_SUFFIX)),
        "not executable",
    )
    .unwrap();
    assert!(
        pinned_search_tool("fd", "fd", "1.2.3", root.path().as_os_str())
            .unwrap_err()
            .contains("--version:")
    );
}

/// Relative candidates are refused rather than resolved against the current directory.
#[test]
fn pinned_search_rejects_relative_candidate() {
    let cwd = std::env::current_dir().unwrap();
    let root = Scratch::new_in(&cwd).unwrap();
    fixture(root.path(), b"fd 1.2.3", 0).unwrap();
    let relative = root.path().strip_prefix(&cwd).unwrap();
    assert!(
        pinned_search_tool("fd", "fd", "1.2.3", relative.as_os_str())
            .unwrap_err()
            .starts_with("fd: executable path is not absolute:")
    );
}

/// Invalid bytes from a successful probe cannot identify a pinned program.
#[test]
fn pinned_search_rejects_non_utf8_output() {
    let root = Scratch::new().unwrap();
    fixture(root.path(), b"\xff", 0).unwrap();
    assert!(
        pinned_search_tool("fd", "fd", "1.2.3", root.path().as_os_str())
            .unwrap_err()
            .starts_with("fd --version:")
    );
}

/// Nextest's text environment file cannot transport a non-UTF-8 executable path.
#[cfg(unix)]
#[test]
fn pinned_search_rejects_non_utf8_path() {
    use std::os::unix::ffi::OsStrExt;
    let root = Scratch::new().unwrap();
    let directory = root.path().join(OsStr::from_bytes(b"invalid-\xff"));
    let executable = directory.join(format!("fd{}", std::env::consts::EXE_SUFFIX));
    let rejection = format!("fd: executable path is not UTF-8: {}", executable.display());

    assert_eq!(executable_text("fd", &executable), Err(rejection.clone()));

    // Only a volume that stores the byte spelling can hold the fixture for the full resolution;
    // the conversion above proves the rejection on every host.
    if root
        .keeps_distinct(&[OsStr::from_bytes(b"invalid-\xff")])
        .unwrap()
    {
        std::fs::create_dir(&directory).unwrap();
        fixture(&directory, b"fd 1.2.3", 0).unwrap();
        assert_eq!(
            pinned_search_tool("fd", "fd", "1.2.3", directory.as_os_str()),
            Err(rejection)
        );
    }
}
