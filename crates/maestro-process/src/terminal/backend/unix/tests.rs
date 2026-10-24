//! Native login argument preservation.
use super::{PtyCommand, PtyEnvironment, native_command};

#[test]
fn login_shell_observes_login_basename() {
    let description = PtyCommand {
        program: "/bin/sh".into(),
        argv0: Some("-sh".into()),
        args: vec!["-c".into(), "printf '%s' \"$0\"".into()],
        cwd: None,
        environment: PtyEnvironment::Inherit(vec![]),
    };
    let output = native_command(description).output().unwrap();
    assert!(output.status.success());
    assert_eq!(output.stdout, b"-sh");
}
