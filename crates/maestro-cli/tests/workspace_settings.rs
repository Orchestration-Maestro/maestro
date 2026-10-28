//! Settings output observed through the command presentation boundary.
#![cfg(not(target_arch = "wasm32"))]
use maestro_cli::workspace_commands::settings::run_reset_keys;
use maestro_test_directory::Scratch;
use std::fs;

#[test]
fn reset_keys_presents_backup_and_reload_guidance() {
    let scratch = Scratch::new().unwrap();
    let root = scratch.path().to_str().unwrap();
    fs::write(
        scratch.path().join("keybindings.json"),
        r#"{"multiplexer.focus":"alt+h"}"#,
    )
    .unwrap();
    let (mut stdout, mut stderr) = (Vec::new(), Vec::new());
    assert_eq!(
        run_reset_keys(&[], root, 42, &mut stdout, &mut stderr).unwrap(),
        0
    );
    let text = String::from_utf8(stdout).unwrap();
    assert!(text.contains("keybindings.json.bak-42"), "{text}");
    assert!(text.contains("Removed 1 multiplexer override"), "{text}");
    assert!(text.contains("maestro daemon reload"), "{text}");
    assert!(text.contains("restart"), "{text}");
    assert!(stderr.is_empty());
}

#[test]
fn reset_keys_rejects_usage_before_effects_and_preserves_error_cause() {
    let scratch = Scratch::new().unwrap();
    let missing = scratch.path().join("missing");
    let root = missing.to_str().unwrap();
    let (mut stdout, mut stderr) = (Vec::new(), Vec::new());
    assert_eq!(
        run_reset_keys(&["--unknown".into()], root, 42, &mut stdout, &mut stderr).unwrap(),
        2
    );
    assert_eq!(stderr, b"usage: maestro settings reset-keys\n");
    assert!(stdout.is_empty());
    assert!(!missing.exists());
    stderr.clear();
    assert_eq!(
        run_reset_keys(&[], root, 42, &mut stdout, &mut stderr).unwrap(),
        1
    );
    let text = String::from_utf8(stderr).unwrap();
    assert!(text.contains("could not reset keybindings"), "{text}");
    assert!(text.contains(root), "{text}");
    let cause = fs::read(missing.join("keybindings.json"))
        .unwrap_err()
        .to_string();
    assert!(text.contains(&cause), "{text}");
    assert!(stdout.is_empty());
    assert!(!missing.exists());
}
