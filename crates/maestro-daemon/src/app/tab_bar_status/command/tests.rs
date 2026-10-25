//! Command launch deadlines and bounded stream accumulation.

use super::*;

#[test]
fn tab_status_command_timeout_starts_before_task_is_polled() {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .start_paused(true)
        .build()
        .unwrap()
        .block_on(check_unpolled_deadline());
}

/// A launch must expire before its first scheduled native spawn.
async fn check_unpolled_deadline() {
    let directory = maestro_test_directory::Scratch::new().unwrap();
    let path = directory.path().join("never-ran");
    let (sender, mut receiver) = mpsc::channel(1);
    let _task = spawn_status_command(
        sender,
        StatusCommandRequest {
            generation: 7,
            segment_index: 3,
            command: format!("printf ran > '{}'", path.display()),
            timeout: Duration::from_secs(1),
            environment: Vec::new(),
            cwd: None,
        },
    );
    tokio::time::advance(Duration::from_secs(2)).await;
    assert!(
        matches!(receiver.recv().await.unwrap(), AppEvent::TabBarCommandFinished {
        generation: 7, segment_index: 3, result: Err(error),
    } if error == "timed out after 1s")
    );
    assert!(!path.exists());
}

#[test]
fn bounded_last_line_drain_retains_prefix_and_empty_line() {
    let mut output = LastOutputLine::default();
    for _ in 0..8 {
        output.push(&[b'x'; 1024]);
    }
    assert_eq!(output.last().len(), MAX_COMMAND_LINE_BYTES);
    output.push(b"\nlast");
    output.push(b" line\n");
    assert_eq!(output.last(), b"last line");
    output.push(b"\n");
    assert!(output.last().is_empty());
}
