//! Desktop notifications preserve option-like titles as operands.
#![cfg(test)]
#![cfg(target_os = "linux")]

use maestro_daemon::platform::show_desktop_notification;
use std::os::unix::fs::PermissionsExt;

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
    let dir = directory.path().to_path_buf();
    let script = dir.join("notify-send");
    std::fs::write(
        &script,
        "#!/bin/sh\nprintf '%s\\n' \"$@\" > \"$MAESTRO_NOTIFICATION_ARGS\"\n",
    )
    .unwrap();
    std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o700)).unwrap();
    assert!(
        maestro_test_process::TestProcess::current(
            "daemon_desktop_notification_separates_option_like_titles"
        )
        .unwrap()
        .env("DISPLAY", ":0")
        .env_remove("WAYLAND_DISPLAY")
        .env("PATH", &dir)
        .env("MAESTRO_NOTIFICATION_ARGS", dir.join("args"))
        .status()
        .unwrap()
        .success()
    );
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn desktop_notification_does_not_claim_unavailable_or_failed_delivery() {
    if std::env::var_os("MAESTRO_NOTIFICATION_UNAVAILABLE").is_some() {
        assert!(!show_desktop_notification("title", Some("")).unwrap());
        return;
    }
    let name = "desktop_notification_does_not_claim_unavailable_or_failed_delivery";
    let directory = maestro_test_directory::Scratch::new().unwrap();
    let dir = directory.path().to_path_buf();
    assert!(
        maestro_test_process::TestProcess::current(name)
            .unwrap()
            .env("MAESTRO_NOTIFICATION_UNAVAILABLE", "1")
            .env_remove("DISPLAY")
            .env_remove("WAYLAND_DISPLAY")
            .status()
            .unwrap()
            .success()
    );
    assert!(
        maestro_test_process::TestProcess::current(name)
            .unwrap()
            .env("MAESTRO_NOTIFICATION_UNAVAILABLE", "1")
            .env("DISPLAY", ":0")
            .env("PATH", &dir)
            .status()
            .unwrap()
            .success()
    );
    let script = dir.join("notify-send");
    std::fs::write(&script, "#!/bin/sh\nexit 1\n").unwrap();
    std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o700)).unwrap();
    assert!(
        maestro_test_process::TestProcess::current(name)
            .unwrap()
            .env("MAESTRO_NOTIFICATION_UNAVAILABLE", "1")
            .env("DISPLAY", ":0")
            .env("PATH", &dir)
            .status()
            .unwrap()
            .success()
    );
    std::fs::remove_dir_all(dir).unwrap();
}
