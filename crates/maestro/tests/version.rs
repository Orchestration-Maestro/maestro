use std::process::Command;

#[test]
fn prints_only_name_and_package_version() {
    let output = Command::new(env!("CARGO_BIN_EXE_maestro"))
        .output()
        .expect("maestro should start");

    assert!(output.status.success());
    assert_eq!(
        output.stdout,
        format!("maestro {}\n", env!("CARGO_PKG_VERSION")).as_bytes()
    );
    assert!(output.stderr.is_empty());
}
