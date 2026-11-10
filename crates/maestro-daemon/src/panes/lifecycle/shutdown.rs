//! Session shutdown policy over the shared process owner.
use super::reader;
use crate::events::AppEvent;
use maestro_process::{
    children::{Signal, process_exists, signal_processes},
    terminal::jobs::session_processes_by_id,
};
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};
use std::time::{Duration, Instant};
use tokio::task::JoinHandle;

/// Reaps the direct child before waiting for lossless workspace delivery.
pub(super) fn watch_child(
    mut child: std::process::Child,
    completed: Arc<AtomicBool>,
    shared: &reader::ReadState,
) -> JoinHandle<()> {
    let pane_id = shared.pane_id;
    let events = shared.events.clone();
    tokio::spawn(async move {
        let result = tokio::task::spawn_blocking(move || {
            let status = child.wait();
            completed.store(true, Ordering::Release);
            status
        })
        .await;
        match result {
            Ok(Ok(status)) => {
                crate::diagnostics::pane_exited(pane_id.raw(), &format!("{status:?}"));
            }
            Ok(Err(error)) => {
                crate::diagnostics::pane_exit_failed(pane_id.raw(), &error.to_string());
            }
            Err(error) => tracing::warn!(%error, "pane child watcher failed"),
        }
        AppEvent::PaneDied { pane_id }.publish(&events).await;
    })
}

/// A reaped direct child is gone even if its PID has already been reused.
pub(super) fn process_alive_for_shutdown(
    pid: u32,
    child: u32,
    reaped: bool,
    exists: impl FnOnce(u32) -> bool,
) -> bool {
    !(pid == child && reaped) && exists(pid)
}

/// Waits only for this session's original process set.
fn wait_for_exit(
    pids: &[u32],
    child: u32,
    completed: Option<&AtomicBool>,
    grace: Duration,
) -> bool {
    let deadline = Instant::now() + grace;
    loop {
        let reaped = completed.is_some_and(|flag| flag.load(Ordering::Acquire));
        if pids
            .iter()
            .all(|pid| !process_alive_for_shutdown(*pid, child, reaped, process_exists))
        {
            return true;
        }
        if Instant::now() >= deadline {
            return false;
        }
        std::thread::sleep(Duration::from_millis(20));
    }
}

/// Escalates hangup, termination and kill over the owned session.
pub(super) fn shutdown_processes(child: u32, completed: Option<&AtomicBool>) {
    if child == 0 {
        return;
    }
    let mut pids = session_processes_by_id(child);
    if pids.is_empty() {
        pids.push(child);
    }
    pids.sort_unstable();
    pids.dedup();
    for signal in [Signal::Hangup, Signal::Terminate, Signal::Kill] {
        signal_processes(&pids, signal);
        if wait_for_exit(&pids, child, completed, Duration::from_millis(250)) {
            return;
        }
    }
    tracing::warn!(child, "pane session still alive after forced shutdown");
}
