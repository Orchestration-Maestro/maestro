//! The single daemon event loop and its completion-owned shutdown phase.

use super::super::App;
use crate::{api::RequestMessage, paths::DaemonPaths};
use maestro_settings::SettingsManager;
use maestro_workspace::terminal_id::TerminalId;
use std::{future::Future, time::Instant};

impl<C: FnMut() -> u128> App<C> {
    /// Runs current daemon producers over one channel selection and one deadline timer,
    /// answering each workspace command from `requests` in arrival order.
    ///
    /// The caller supplies shutdown readiness; transport and frontend arms share this loop.
    /// `settings` is the retained settings owner a configuration reload reads through.
    pub async fn run(
        &mut self,
        paths: &DaemonPaths,
        requests: &mut tokio::sync::mpsc::UnboundedReceiver<RequestMessage>,
        settings: &mut SettingsManager,
        shutdown: impl Future<Output = ()>,
    ) {
        tokio::pin!(shutdown);
        while !self.state.should_quit.0 {
            self.apply_queued_work().await;
            self.sync_session_save_schedule();
            self.handle_scheduled_tasks(Instant::now(), paths).await;
            let deadline = self.next_headless_loop_deadline(Instant::now(), false);
            tokio::select! {
                () = &mut shutdown => self.state.should_quit.0 = true,
                event = self.event_rx.recv() => {
                    if let Some(event) = event { self.handle_internal_event(event); }
                    else { self.state.should_quit.0 = true; }
                }
                Some(request) = requests.recv() => {
                    self.answer_request(request, settings, paths).await;
                }
                () = self.launch.render_notify.notified() => {}
                () = wait_for_deadline(deadline) => {}
            }
        }
        self.shutdown(paths).await;
    }

    /// Applies what the last pass queued: observations, finished worktree operations, terminal
    /// titles and detached runtimes.
    async fn apply_queued_work(&mut self) {
        self.drain_internal_events();
        self.settle_worktree_operations().await;
        self.sync_pending_terminal_titles().await;
        self.shutdown_detached_terminal_runtimes().await;
    }

    /// Completes final persistence before terminating children and draining their exit hooks.
    pub async fn shutdown(&mut self, paths: &DaemonPaths) {
        self.state.should_quit.0 = true;
        self.drain_all_internal_events();
        self.finish_history_reload(paths);
        self.save_session_now(paths);
        self.configure_tab_bar_status(&[], "");
        let terminals: Vec<_> = self
            .terminal_runtimes
            .iter()
            .map(|(id, _)| id.clone())
            .collect();
        for terminal in terminals {
            self.shutdown_terminal_runtime(&terminal).await;
        }
        self.state.terminal_runtime_shutdowns.clear();
    }

    /// Releases detached owners after the event batch has released topology borrows.
    pub(in crate::app) async fn shutdown_detached_terminal_runtimes(&mut self) {
        for terminal in std::mem::take(&mut self.state.terminal_runtime_shutdowns) {
            self.shutdown_terminal_runtime(&terminal).await;
        }
    }

    /// Keeps the runtime alive until its critical exit publication has been applied.
    pub(in crate::app) async fn shutdown_terminal_runtime(&mut self, terminal: &TerminalId) {
        let Some(mut runtime) = self.terminal_runtimes.remove(terminal) else {
            return;
        };
        if let Some(mut hook) = runtime.begin_shutdown().await {
            loop {
                tokio::select! {
                    _ = &mut hook => break,
                    event = self.event_rx.recv() => {
                        if let Some(event) = event { self.handle_internal_event(event); }
                        else { let _ = hook.await; break; }
                    }
                }
            }
        }
        self.drain_all_internal_events();
        drop(runtime);
    }
}

/// Waits on exactly the selected deadline, remaining idle when no producer has one.
async fn wait_for_deadline(deadline: Option<Instant>) {
    match deadline {
        Some(deadline) => tokio::time::sleep_until(deadline.into()).await,
        None => std::future::pending().await,
    }
}
