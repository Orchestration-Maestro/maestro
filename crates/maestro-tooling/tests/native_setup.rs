//! Native command staging through the same entry point nextest uses.

use std::fs;
use std::process::Command;

use maestro_test_directory::Scratch;

/// A staged development command keeps its native suffix and can stage itself again.
#[test]
fn installed_setup_command_keeps_its_native_name_and_runs() {
    let scratch = Scratch::new().unwrap();
    let root = scratch.path();
    let output = Command::new(env!("CARGO_BIN_EXE_development"))
        .current_dir(root)
        .args(["prepare-test-setup", "install"])
        .arg(root.join("bundle"))
        .output()
        .unwrap();
    assert!(output.status.success(), "{output:?}");
    let native = format!("development{}", std::env::consts::EXE_SUFFIX);
    let staged = root.join("bundle").join(&native);
    assert!(staged.is_file(), "{}", staged.display());

    let output = Command::new(&staged)
        .current_dir(root)
        .args(["prepare-test-setup", "install"])
        .arg(root.join("restored"))
        .output()
        .unwrap();

    assert!(output.status.success(), "{output:?}");
    assert_eq!(
        fs::read(root.join("restored").join(native)).unwrap(),
        fs::read(staged).unwrap()
    );
}
