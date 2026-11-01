//! Controlled output failures and committed durability warnings.
use super::*;
use maestro_app::presentation_data::keybindings::KeybindingsReset;

/// Writer that preserves an observable failure cause.
struct Refused;

impl Write for Refused {
    fn write(&mut self, _: &[u8]) -> io::Result<usize> {
        Err(io::Error::new(
            io::ErrorKind::BrokenPipe,
            "controlled output failure",
        ))
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

/// A committed application outcome with an original sync cause.
fn committed_warning() -> KeybindingsReset {
    KeybindingsReset {
        backup_path: Some("keybindings.json.bak-42".into()),
        removed: 1,
        dir_sync: Some(io::Error::other("controlled directory sync cause")),
    }
}

#[test]
fn committed_warning_preserves_success_and_original_cause() {
    let (mut stdout, mut stderr) = (Vec::new(), Vec::new());
    assert_eq!(
        present_reset(committed_warning(), &mut stdout, &mut stderr).unwrap(),
        0
    );
    let stdout = String::from_utf8(stdout).unwrap();
    assert!(stdout.contains("keybindings.json.bak-42"), "{stdout}");
    assert!(
        stdout.contains("Removed 1 multiplexer overrides."),
        "{stdout}"
    );
    assert!(stdout.contains("maestro daemon reload"), "{stdout}");
    assert_eq!(
        stderr,
        b"Keybindings reset; directory sync warning: controlled directory sync cause\n"
    );
}

#[test]
fn outcome_presentation_propagates_stdout_and_warning_writer_failures() {
    let mut output = Vec::new();
    let error = present_reset(committed_warning(), &mut Refused, &mut output).unwrap_err();
    assert_eq!(error.kind(), io::ErrorKind::BrokenPipe);
    assert_eq!(error.to_string(), "controlled output failure");
    assert!(output.is_empty());
    let error = present_reset(committed_warning(), &mut output, &mut Refused).unwrap_err();
    assert_eq!(error.kind(), io::ErrorKind::BrokenPipe);
    assert_eq!(error.to_string(), "controlled output failure");
    assert!(!output.is_empty());
}

#[test]
fn usage_and_application_errors_propagate_stderr_failures() {
    let scratch = maestro_test_directory::Scratch::new().unwrap();
    let root = scratch.path().to_str().unwrap();
    let mut stdout = Vec::new();
    let error = run_reset_keys(&["extra".into()], root, 42, &mut stdout, &mut Refused).unwrap_err();
    assert_eq!(error.kind(), io::ErrorKind::BrokenPipe);
    std::fs::write(scratch.path().join("keybindings.json"), "[]").unwrap();
    let error = run_reset_keys(&[], root, 42, &mut stdout, &mut Refused).unwrap_err();
    assert_eq!(error.kind(), io::ErrorKind::BrokenPipe);
    assert_eq!(error.to_string(), "controlled output failure");
    assert!(stdout.is_empty());
}
