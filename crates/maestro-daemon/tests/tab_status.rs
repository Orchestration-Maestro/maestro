//! Retained status segments react only to current command output.
#![cfg(test)]
#![cfg(target_os = "linux")]

use crate::support::{self, launch};

use maestro_daemon::{
    api::event_hub::EventHub,
    app::{App, state::TabBarStatusSegment},
    events::AppEvent,
    settings::{model::Config, tab_bar::TabBarRightEntryConfig},
};
use maestro_test_directory::temporary_root;
use std::time::Instant;
use tokio::sync::mpsc;

/// Daemon with a deterministic public identity clock.
type TestApp = App<fn() -> u128>;

/// Supplies a daemon without launching any pane process.
fn test_app() -> TestApp {
    let (launch, receiver) = launch::launch_inputs(&[], support::manifest_cache());
    let mut app = App::new(
        &Config::default(),
        (|| 0) as fn() -> u128,
        launch,
        receiver,
        EventHub::default(),
    );
    app.state
        .topology
        .new_with_tab(temporary_root(), None, |_, _| Ok(()))
        .unwrap();
    support::populate_terminals(&mut app.state);
    app.state.active = Some(0);
    support::assert_identity_invariants(&app.state);
    app
}

/// A command that produces two complete lines.
fn command() -> TabBarRightEntryConfig {
    TabBarRightEntryConfig::Command {
        command: "printf 'old\\nfinal\\n'".into(),
        interval_seconds: 5,
        timeout_seconds: 2,
    }
}

#[test]
fn daemon_tab_bar_command_events_render_only_when_visible_output_changes() {
    let mut app = test_app();
    app.configure_tab_bar_status(&[command()], " ");
    let generation = app.tab_bar_status_generation();
    for (generation, output, changed) in [
        (generation, None, false),
        (generation, Some("ready"), true),
        (generation, Some("ready"), false),
        (generation.wrapping_add(1), Some("stale"), false),
    ] {
        let impact =
            app.handle_internal_event_with_render_impact(AppEvent::TabBarCommandFinished {
                generation,
                segment_index: 0,
                result: Ok(output.map(str::to_owned)),
            });
        assert_eq!(impact, changed);
        assert_eq!(app.launch.render_dirty.take().generic, changed);
    }
}

#[test]
fn daemon_stale_command_result_does_not_replace_reloaded_status() {
    let mut app = test_app();
    app.configure_tab_bar_status(&[command()], " ");
    let generation = app.tab_bar_status_generation();
    app.configure_tab_bar_status(
        &[TabBarRightEntryConfig::Text {
            text: "fresh".into(),
        }],
        " ",
    );
    app.handle_internal_event(AppEvent::TabBarCommandFinished {
        generation,
        segment_index: 0,
        result: Ok(Some("stale".into())),
    });
    assert_eq!(
        app.state.tab_bar_right,
        vec![TabBarStatusSegment::Text(Some("fresh".into()))]
    );
}

#[test]
fn daemon_in_flight_command_has_no_second_deadline() {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap()
        .block_on(async {
            let mut app = test_app();
            app.configure_tab_bar_status(&[command()], " ");
            assert!(app.next_tab_bar_status_deadline().is_some());
            app.handle_tab_bar_status_tasks(Instant::now());
            assert_eq!(app.next_tab_bar_status_deadline(), None);
            let completion = app.event_rx.recv().await.unwrap();
            assert!(matches!(&completion, AppEvent::TabBarCommandFinished { result: Ok(Some(output)), .. } if output == "final"));
            app.handle_internal_event(completion);
            assert!(app.next_tab_bar_status_deadline().is_some());
        });
}

#[test]
fn daemon_datetime_refresh_updates_its_segment_once_per_deadline() {
    let mut app = test_app();
    app.configure_tab_bar_status(
        &[TabBarRightEntryConfig::Datetime {
            format: "%Y-%m-%d %H:%M:%S".into(),
        }],
        " ",
    );
    app.state.tab_bar_right[0] = TabBarStatusSegment::Text(None);
    let deadline = app.next_tab_bar_status_deadline().unwrap();
    assert!(app.handle_tab_bar_status_tasks(deadline));
    assert!(
        matches!(&app.state.tab_bar_right[0], TabBarStatusSegment::Text(Some(value)) if !value.is_empty())
    );
    assert!(!app.handle_tab_bar_status_tasks(deadline));
}

#[test]
fn tab_status_command_reports_its_sanitized_last_line() {
    tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap().block_on(async {
    let mut app = test_app();
    app.configure_tab_bar_status(&[command()], " ");
    app.handle_tab_bar_status_tasks(Instant::now());
    let event = app.event_rx.recv().await.unwrap();
    assert!(
        matches!(&event, AppEvent::TabBarCommandFinished { generation, segment_index: 0, result: Ok(Some(output)) } if *generation == app.tab_bar_status_generation() && output == "final")
    );
    app.handle_internal_event(event);
    assert_eq!(
        app.state.tab_bar_right,
        vec![TabBarStatusSegment::Text(Some("final".into()))]
    );

    });
}

#[test]
fn tab_status_command_drains_large_output_and_keeps_the_last_line() {
    tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap().block_on(async {
    let mut app = test_app();
    app.configure_tab_bar_status(
        &[TabBarRightEntryConfig::Command {
            command: "head -c 5000 /dev/zero | tr '\\0' x; printf '\\nREADY\\n'".into(),
            interval_seconds: 5,
            timeout_seconds: 2,
        }],
        " ",
    );
    app.handle_tab_bar_status_tasks(Instant::now());
    assert!(
        matches!(app.event_rx.recv().await.unwrap(), AppEvent::TabBarCommandFinished { result: Ok(Some(output)), .. } if output == "READY")
    );

    });
}

#[test]
fn daemon_reload_aborts_an_in_flight_command_task_and_its_descendants() {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap()
        .block_on(check_descendant_reload());
}

/// Uses FIFO handshakes rather than sleeps to start and release a real descendant.
async fn check_descendant_reload() {
    use std::io::Write;
    let directory = maestro_test_directory::Scratch::new().unwrap();
    let root = directory.path().to_path_buf();
    let started = root.join("started");
    let gate = root.join("gate");
    let survived = root.join("survived");
    assert!(
        std::process::Command::new("mkfifo")
            .args([&started, &gate])
            .status()
            .unwrap()
            .success()
    );
    let reader = tokio::task::spawn_blocking(move || std::fs::read_to_string(started).unwrap());
    let mut app = test_app();
    app.configure_tab_bar_status(&[TabBarRightEntryConfig::Command {
        command: format!("sh -c 'printf \"%s\\n\" \"$$\" > \"{}/started\"; read x < \"{}\"; printf survived > \"{}\"' & wait", root.display(), gate.display(), survived.display()),
        interval_seconds: 5,
        timeout_seconds: 20,
    }], " ");
    app.handle_tab_bar_status_tasks(Instant::now());
    let pid = reader.await.unwrap().trim().parse::<u32>().unwrap();
    app.configure_tab_bar_status(
        &[TabBarRightEntryConfig::Text {
            text: "reloaded".into(),
        }],
        " ",
    );
    // Release any surviving reader before waiting; missing descendant cleanup fails visibly.
    let mut release = std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .open(&gate)
        .unwrap();
    release.write_all(b"go\n").unwrap();
    wait_for_process_exit(pid);
    assert!(
        !survived.exists(),
        "status command descendant survived reload"
    );
    assert!(matches!(
        app.event_rx.try_recv(),
        Err(mpsc::error::TryRecvError::Empty)
    ));
    drop(app.launch);
    assert!(
        app.event_rx.recv().await.is_none(),
        "cancelled command published a result"
    );
    std::fs::remove_dir_all(root).unwrap();
}

/// Waits for the kernel's killed/exited state, without polling the Tokio executor.
fn wait_for_process_exit(pid: u32) {
    while let Ok(status) = std::fs::read_to_string(format!("/proc/{pid}/status")) {
        if status.lines().any(|line| {
            line.starts_with("State:") && (line.contains("Z (") || line.contains("X ("))
        }) {
            break;
        }
        std::thread::yield_now();
    }
}

#[test]
fn local_datetime_observes_supplied_timezone_inside_runtime() {
    use maestro_test_process::TestProcess;
    if let Ok(seconds) = std::env::var("MAESTRO_TEST_ZONE_OFFSET") {
        let offset = time::UtcOffset::from_whole_seconds(seconds.parse().unwrap()).unwrap();
        tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap()
            .block_on(async {
                let before = time::OffsetDateTime::now_utc().unix_timestamp();
                let local = maestro_daemon::platform::local_datetime().unwrap();
                let after = time::OffsetDateTime::now_utc().unix_timestamp();
                assert!((before..=after).contains(&local.assume_offset(offset).unix_timestamp()));
            });
        return;
    }
    for (zone, offset) in [("Etc/UTC", "0"), ("Etc/GMT+7", "-25200")] {
        let output =
            TestProcess::current("local_datetime_observes_supplied_timezone_inside_runtime")
                .unwrap()
                .env("TZ", zone)
                .env("MAESTRO_TEST_ZONE_OFFSET", offset)
                .output()
                .unwrap();
        assert!(
            output.status.success(),
            "{zone}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
}

#[test]
fn internal_drain_retains_status_and_bell_render_impact() {
    let mut app = test_app();
    app.configure_tab_bar_status(&[command()], " ");
    app.launch
        .events
        .try_send(AppEvent::TabBarCommandFinished {
            generation: app.tab_bar_status_generation(),
            segment_index: 0,
            result: Ok(None),
        })
        .unwrap();
    app.launch
        .events
        .try_send(AppEvent::TerminalBell {
            pane_id: app.state.topology.workspaces[0].tabs[0].root_pane,
            count: 1,
        })
        .unwrap();
    assert!(!app.drain_internal_events());
    assert!(matches!(
        app.event_rx.try_recv(),
        Err(mpsc::error::TryRecvError::Empty)
    ));
    app.launch
        .events
        .try_send(AppEvent::TabBarCommandFinished {
            generation: app.tab_bar_status_generation(),
            segment_index: 0,
            result: Ok(Some("changed".into())),
        })
        .unwrap();
    assert!(app.drain_internal_events());
    assert!(!app.drain_internal_events());
}
