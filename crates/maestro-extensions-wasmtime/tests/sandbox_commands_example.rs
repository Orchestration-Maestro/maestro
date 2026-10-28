//! Platform operands exercised without a namespace or internet access.
#![cfg(test)]
#![cfg(unix)]

use std::{
    ffi::OsStr,
    path::Path,
    process::{Command, Output},
};

#[path = "../../../examples/extensions/sandbox/platform_commands.rs"]
mod commands;

/// Runs the same literal script and argument boundary the component supplies to native I/O.
fn run(script: &str, argument: &OsStr) -> Output {
    Command::new("bash")
        .args(["-c", script, "--"])
        .arg(argument)
        .env_clear()
        .env("PATH", "/usr/bin:/bin")
        .env("LC_ALL", "C")
        .output()
        .unwrap()
}

#[test]
fn sandbox_command_globs_preserve_names_and_reject_non_unicode_matches() {
    use std::os::unix::ffi::OsStringExt;
    let scratch = maestro_test_directory::Scratch::new().unwrap();
    let root = scratch.path();
    std::fs::create_dir(root.join("nested")).unwrap();
    let names = [".hidden.pem", "a b.pem", "nested/key.pem", "λ\nz.pem"];
    for name in names {
        std::fs::write(root.join(name), "secret").unwrap();
    }
    std::fs::write(root.join("ignored.key"), "ignored").unwrap();
    let pattern = root.join("**/*.pem");
    let output = run(commands::GLOB, pattern.as_os_str());
    assert!(output.status.success(), "{output:?}");
    let mut actual = commands::decode_matches(&output.stdout).unwrap();
    actual.sort();
    let mut expected = names
        .map(|name| root.join(name).to_str().unwrap().to_owned())
        .to_vec();
    expected.sort();
    assert_eq!(actual, expected);
    std::fs::write(
        root.join(std::ffi::OsString::from_vec(b"bad-\xff.pem".to_vec())),
        "secret",
    )
    .unwrap();
    let output = run(commands::GLOB, pattern.as_os_str());
    assert!(output.status.success(), "{output:?}");
    assert!(commands::decode_matches(&output.stdout).is_err());
}

#[test]
fn sandbox_command_temporary_directory_is_private_and_rejects_files_or_foreign_owners() {
    use std::os::unix::fs::{MetadataExt, PermissionsExt};
    let scratch = maestro_test_directory::Scratch::new().unwrap();
    let path = scratch.path().join("space and newline\n");
    let output = run(commands::TEMPORARY, path.as_os_str());
    assert!(output.status.success(), "{output:?}");
    assert!(path.is_dir());
    assert_eq!(
        std::fs::metadata(&path).unwrap().permissions().mode() & 0o777,
        0o700
    );
    assert_eq!(
        output.stdout,
        std::fs::canonicalize(&path)
            .unwrap()
            .as_os_str()
            .as_encoded_bytes()
    );
    std::fs::write(path.join("kept"), "artifact").unwrap();
    assert!(run(commands::TEMPORARY, path.as_os_str()).status.success());
    assert_eq!(
        std::fs::read_to_string(path.join("kept")).unwrap(),
        "artifact"
    );
    let file = scratch.path().join("file");
    std::fs::write(&file, "unchanged").unwrap();
    assert!(!run(commands::TEMPORARY, file.as_os_str()).status.success());
    assert_eq!(std::fs::read_to_string(file).unwrap(), "unchanged");
    let foreign = Path::new("/");
    if std::fs::metadata(foreign).unwrap().uid() != std::fs::metadata(scratch.path()).unwrap().uid()
    {
        assert!(
            !run(commands::TEMPORARY, foreign.as_os_str())
                .status
                .success()
        );
    }
}
