//! Literal POSIX shell serialization through the shared text owner.
#![cfg(test)]

#[test]
fn shell_literal_operands_share_one_posix_quoter() {
    for (input, expected) in [
        ("", "''"),
        ("a,b", "a,b"),
        ("@%_+=:,./-Az09", "@%_+=:,./-Az09"),
        ("a'b", "'a'\\''b'"),
        ("a\n b", "'a\n b'"),
        ("é;$\"", "'é;$\"'"),
    ] {
        assert_eq!(maestro_text::shell_quote(input), expected);
        #[cfg(unix)]
        {
            let output = std::process::Command::new("/bin/sh")
                .args([
                    "-c",
                    &format!("printf %s {}", maestro_text::shell_quote(input)),
                ])
                .output()
                .unwrap();
            assert!(output.status.success());
            assert_eq!(output.stdout, input.as_bytes());
        }
    }
}
