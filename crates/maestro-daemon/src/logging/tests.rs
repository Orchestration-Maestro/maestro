//! File rotation behavior.
use super::writer::*;
use maestro_test_directory::{Scratch, temporary_root};
use std::{fs, io::Write, path::PathBuf};
use tracing_subscriber::fmt::writer::MakeWriter;

#[test]
fn log_generation_suffix_follows_the_filename() {
    let path = PathBuf::from("/tmp/maestro.log");
    assert_eq!(
        rotated_log_path(&path, 2),
        PathBuf::from("/tmp/maestro.log.2")
    );
}

#[test]
fn log_rotation_shifts_generations_in_order() {
    let dir = Scratch::new().unwrap();
    let path = dir.path().join("maestro.log");
    fs::write(&path, "current").unwrap();
    fs::write(rotated_log_path(&path, 1), "older").unwrap();

    let mut state = RotatingFileState {
        path: path.clone(),
        max_bytes: 128,
        retained_files: 2,
        file: None,
        current_size: 0,
        disabled: false,
    };
    state.rotate_files().unwrap();

    assert_eq!(
        fs::read_to_string(rotated_log_path(&path, 1)).unwrap(),
        "current"
    );
    assert_eq!(
        fs::read_to_string(rotated_log_path(&path, 2)).unwrap(),
        "older"
    );
    assert!(!path.exists());
}

#[test]
fn log_limit_replaces_without_retained_generations() {
    let dir = Scratch::new().unwrap();
    let path = dir.path().join("maestro.log");

    let writer = RotatingFileMakeWriter::new(dir.path(), "maestro.log", 8, 0).unwrap();
    {
        let mut guard = writer.make_writer();
        guard.write_all(b"12345678").unwrap();
        guard.write_all(b"abc").unwrap();
        guard.flush().unwrap();
    }

    assert_eq!(fs::read_to_string(&path).unwrap(), "abc");
    assert!(!rotated_log_path(&path, 1).exists());
}

#[test]
fn structured_log_events_keep_fields_levels_and_messages() {
    let ((), events) = capture(|| {
        super::startup("daemon");
        super::shutdown("client");
    });
    assert_eq!(events[0].level, tracing::Level::INFO);
    assert_eq!(
        events[0].fields,
        serde_json::json!({
            "event":"app.startup", "subsystem":"daemon", "outcome":"started",
            "pid":std::process::id(), "message":"maestro starting"
        })
    );
    assert_eq!(
        events[1].fields,
        serde_json::json!({
            "event":"app.shutdown", "subsystem":"client", "outcome":"completed",
            "pid":std::process::id(), "message":"maestro exiting"
        })
    );
    request_events();
    lifecycle_events();
}

/// One captured structured event.
#[derive(Debug)]
pub(crate) struct Recorded {
    /// Original event severity.
    pub(crate) level: tracing::Level,
    /// Typed fields, including the formatted message.
    pub(crate) fields: serde_json::Value,
}

/// Capture typed fields without setting a process-global subscriber.
pub(crate) fn capture<T>(action: impl FnOnce() -> T) -> (T, Vec<Recorded>) {
    use tracing_subscriber::prelude::*;
    let events = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
    let subscriber = tracing_subscriber::registry().with(Recorder(events.clone()));
    let result = maestro_test_tracing::with_default(subscriber, action);
    let events = std::mem::take(&mut *events.lock().unwrap());
    (result, events)
}

/// Thread-local event collection layer.
struct Recorder(std::sync::Arc<std::sync::Mutex<Vec<Recorded>>>);
impl<S: tracing::Subscriber> tracing_subscriber::Layer<S> for Recorder {
    fn on_event(&self, event: &tracing::Event<'_>, _: tracing_subscriber::layer::Context<'_, S>) {
        let mut fields = Fields(serde_json::Map::new());
        event.record(&mut fields);
        self.0.lock().unwrap().push(Recorded {
            level: *event.metadata().level(),
            fields: fields.0.into(),
        });
    }
}

/// Typed event field visitor.
struct Fields(serde_json::Map<String, serde_json::Value>);
impl tracing::field::Visit for Fields {
    fn record_debug(&mut self, field: &tracing::field::Field, value: &dyn std::fmt::Debug) {
        self.0
            .insert(field.name().into(), format!("{value:?}").into());
    }
    fn record_str(&mut self, field: &tracing::field::Field, value: &str) {
        self.0.insert(field.name().into(), value.into());
    }
    fn record_u64(&mut self, field: &tracing::field::Field, value: u64) {
        self.0.insert(field.name().into(), value.into());
    }
    fn record_bool(&mut self, field: &tracing::field::Field, value: bool) {
        self.0.insert(field.name().into(), value.into());
    }
}

/// Check all routine request names and both outcome branches.
fn request_events() {
    for method in [
        "pane.get",
        "pane.read",
        "pane.list",
        "workspace.list",
        "tab.list",
        "pane.report_agent",
        "pane.report_agent_session",
        "pane.report_metadata",
        "pane.send",
    ] {
        for changes_ui in [false, true] {
            let ((), events) = capture(|| {
                super::api_request_started("r", method, changes_ui);
                super::api_request_completed("r", method, "ok", changes_ui);
                super::api_request_completed("r", method, "rejected", changes_ui);
            });
            let expected = if changes_ui && method == "pane.send" {
                tracing::Level::INFO
            } else {
                tracing::Level::DEBUG
            };
            assert_eq!(
                events.iter().map(|e| e.level).collect::<Vec<_>>(),
                [expected, expected, tracing::Level::INFO]
            );
            assert_eq!(
                events[0].fields,
                serde_json::json!({"event":"api.request.start",
                "subsystem":"api","outcome":"started","request_id":"r","method":method,
                "changes_ui":changes_ui,"message":"api request received"})
            );
            for (index, outcome) in [(1, "ok"), (2, "rejected")] {
                assert_eq!(
                    events[index].fields,
                    serde_json::json!({"event":"api.request.complete",
                    "subsystem":"api","outcome":outcome,"request_id":"r","method":method,
                    "message":"api request completed"})
                );
            }
        }
    }
}

/// Check all remaining production lifecycle helpers and their entire field sets.
fn lifecycle_events() {
    let ((), events) = capture(emit_lifecycle_events);
    let expected: Vec<serde_json::Value> =
        serde_json::from_str(include_str!("tests/events.json")).unwrap();
    assert_eq!(events.len(), expected.len());
    for (actual, expected) in events.iter().zip(expected) {
        assert_eq!(actual.level.as_str(), expected["level"]);
        assert_eq!(actual.fields, expected["fields"]);
    }
}

/// Invoke the production event boundary with recognizable field values.
fn emit_lifecycle_events() {
    use super::*;
    let path = std::path::Path::new("snapshot.json");
    api_request_failed("r", "pane.send", "bad");
    api_wait_started("r", "p", Some(42));
    api_wait_started("r", "p", None);
    api_wait_completed("r", "p", "ok");
    api_wait_timed_out("r", "p");
    pane_spawn_started(1, 24, 80, 1000);
    pane_spawned(1, 2);
    pane_exited(1, "exit 0");
    pane_exit_failed(1, "bad");
    workspace_created("w", 1);
    workspace_focused("w");
    workspace_closed("w");
    workspace_renamed("w");
    tab_focused("w", "t");
    tab_renamed("w", "t");
    session_saved(path, 3);
    session_save_failed(path, "bad");
    session_cleared(path);
    session_clear_failed(path, "bad");
    session_restored(3, "partial");
    update_check_started();
    update_check_failed("bad");
    update_available("1.2.3");
    config_write_failed(path, "reload", "bad");
    integration_action("install", "shell", "ok");
}

#[test]
fn log_roles_filters_and_failures_use_one_writer() {
    // Windows runs this too: DaemonPaths joins with '/', which a canonical `\\?\` root refuses.
    let scratch = Scratch::new_in(&temporary_root()).unwrap();
    let root = scratch.path();
    let paths = crate::paths::DaemonPaths::new(root.to_str().unwrap(), Some("work")).unwrap();
    for (role, filter, has_debug, has_info, has_warn) in [
        ("in-process", None, false, true, true),
        ("client", Some("maestro=debug"), true, true, true),
        ("daemon", Some("["), false, true, true),
        ("daemon", Some("maestro=warn"), false, false, true),
    ] {
        let path = paths.log_path(role);
        let _ = fs::remove_file(&path);
        let subscriber = super::file_subscriber(&paths, role, filter).unwrap();
        maestro_test_tracing::with_default(subscriber, || {
            super::api_request_started("r", "pane.get", false);
            super::startup(role);
            super::update_check_failed("controlled failure");
        });
        let text = fs::read_to_string(path).unwrap();
        assert_eq!(text.contains("api request received"), has_debug);
        assert_eq!(text.contains("maestro starting"), has_info);
        assert_eq!(text.contains("update check failed"), has_warn);
        assert!(text.contains("maestro_daemon::logging::events"));
        assert!(!text.contains('\u{1b}'));
    }
    assert_eq!(
        super::help_log_paths_summary(&paths),
        format!(
            "{}/daemons/work/maestro-in-process.log (plus maestro-client.log, maestro-daemon.log)",
            root.display()
        )
    );
    let blocked = root.join("not-a-directory");
    fs::write(&blocked, "keep").unwrap();
    let invalid = crate::paths::DaemonPaths::new(blocked.to_str().unwrap(), None).unwrap();
    assert!(super::file_subscriber(&invalid, "daemon", None).is_err());
    super::init_file_logging(&invalid, "daemon", None);
    assert_eq!(fs::read_to_string(&blocked).unwrap(), "keep");
    shared_writer_and_rotation_failure(root);
}

/// Shared guards append to the same output and filesystem failure is swallowed once.
fn shared_writer_and_rotation_failure(root: &std::path::Path) {
    let writer = RotatingFileMakeWriter::new(root, "shared.log", 8, 0).unwrap();
    let other = writer.clone();
    let mut first = writer.make_writer();
    let mut second = other.make_writer();
    first.write_all(b"1234").unwrap();
    second.write_all(b"5678").unwrap();
    second.flush().unwrap();
    let path = root.join("shared.log");
    assert_eq!(fs::read_to_string(&path).unwrap(), "12345678");
    #[cfg(unix)]
    {
        // Replace the pathname while its original append handle remains open.
        fs::rename(&path, root.join("moved.log")).unwrap();
        fs::create_dir(&path).unwrap();
        first.write_all(b"ignored").unwrap();
        assert!(writer.state.lock().unwrap().disabled);
        assert_eq!(
            fs::read_to_string(root.join("moved.log")).unwrap(),
            "12345678"
        );
        fs::remove_dir(&path).unwrap();
        second.write_all(b"still ignored").unwrap();
        second.flush().unwrap();
        assert!(!path.exists());
    }
}
