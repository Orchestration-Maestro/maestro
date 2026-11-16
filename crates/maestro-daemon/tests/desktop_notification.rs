//! Desktop notifications preserve option-like titles as operands.
#![cfg(test)]
#![cfg(target_os = "linux")]

use maestro_daemon::platform::show_desktop_notification;
use std::path::Path;

/// Install the `notify-send` alias with its operand-capture record.
fn install_notify_send(directory: &Path, record: &serde_json::Value) {
    let artifact = std::env::var_os("MAESTRO_NATIVE_FIXTURE")
        .expect("nextest native-fixture setup supplies the artifact");
    let fixture = directory.join("notify-send.fixture");
    std::fs::write(&fixture, format!("notify-send\n{record}")).unwrap();
    maestro_test_process::install_executable(Path::new(&artifact), &directory.join("notify-send"))
        .unwrap();
}

#[test]
fn daemon_desktop_notification_separates_option_like_titles() {
    if let Some(path) = std::env::var_os("MAESTRO_NOTIFICATION_ARGS") {
        assert!(show_desktop_notification("-danger", Some("body")).unwrap());
        assert_eq!(
            std::fs::read_to_string(path).unwrap(),
            "--\n-danger\nbody\n"
        );
        return;
    }
    let directory = maestro_test_directory::Scratch::new().unwrap();
    let bin = directory.path().join("bin");
    let args = directory.path().join("args");
    std::fs::create_dir(&bin).unwrap();
    install_notify_send(&bin, &serde_json::json!({ "args": args, "code": 0 }));
    assert!(
        maestro_test_process::TestProcess::current(
            "daemon_desktop_notification_separates_option_like_titles"
        )
        .unwrap()
        .env("DISPLAY", ":0")
        .env_remove("WAYLAND_DISPLAY")
        .env("PATH", &bin)
        .env("MAESTRO_NOTIFICATION_ARGS", &args)
        .status()
        .unwrap()
        .success()
    );
}

#[test]
fn desktop_notification_does_not_claim_unavailable_or_failed_delivery() {
    if std::env::var_os("MAESTRO_NOTIFICATION_UNAVAILABLE").is_some() {
        assert!(!show_desktop_notification("title", Some("")).unwrap());
        return;
    }
    let name = "desktop_notification_does_not_claim_unavailable_or_failed_delivery";
    let directory = maestro_test_directory::Scratch::new().unwrap();
    let bin = directory.path().join("bin");
    std::fs::create_dir(&bin).unwrap();
    assert!(
        maestro_test_process::TestProcess::current(name)
            .unwrap()
            .env("MAESTRO_NOTIFICATION_UNAVAILABLE", "1")
            .env_remove("DISPLAY")
            .env_remove("WAYLAND_DISPLAY")
            .env("PATH", &bin)
            .status()
            .unwrap()
            .success()
    );
    assert!(
        maestro_test_process::TestProcess::current(name)
            .unwrap()
            .env("MAESTRO_NOTIFICATION_UNAVAILABLE", "1")
            .env("DISPLAY", ":0")
            .env("PATH", &bin)
            .status()
            .unwrap()
            .success()
    );
    install_notify_send(&bin, &serde_json::json!({ "args": null, "code": 1 }));
    assert!(
        maestro_test_process::TestProcess::current(name)
            .unwrap()
            .env("MAESTRO_NOTIFICATION_UNAVAILABLE", "1")
            .env("DISPLAY", ":0")
            .env("PATH", &bin)
            .status()
            .unwrap()
            .success()
    );
}
