//! Helpers that drive live pane processes; they read the daemon's private runtime table.

use crate::{
    service::event_hub::EventHub,
    settings::model::Config,
    state::{
        App,
        test_support::{manifest_cache, spawning::launch_inputs},
    },
    tests::block_on,
};
use maestro_workspace::terminal_id::TerminalId;

/// Types `script` into the shell of `terminal` and returns the visible text once a line
/// starts with `marker`.
pub(crate) async fn run_in_pane(
    app: &App,
    terminal: &TerminalId,
    script: &str,
    marker: &str,
) -> String {
    let runtime = app.terminal_runtimes.get(terminal).unwrap();
    runtime
        .send_bytes(script.as_bytes().to_vec())
        .await
        .unwrap();
    wait_for_line(app, terminal, marker).await
}

/// The visible text of `terminal` once a line starts with `marker`.
pub(crate) async fn wait_for_line(app: &App, terminal: &TerminalId, marker: &str) -> String {
    let runtime = app.terminal_runtimes.get(terminal).unwrap();
    loop {
        let _ = app.launch.render_dirty.take();
        let text = runtime.visible_text();
        if text.lines().any(|line| line.starts_with(marker)) {
            return text;
        }
        app.launch.render_notify.notified().await;
    }
}

/// Runs `body` alone in a process of its own, against a daemon whose panes start real
/// `/bin/sh` shells in the temporary root, then stops every pane it started.
pub(crate) fn with_live_daemon(test: &str, body: impl AsyncFnOnce(&mut App)) {
    use crate::settings::model::NewTerminalCwdConfig;
    use maestro_test_process::{TestProcess, run_alone};
    if run_alone(test).unwrap() {
        return;
    }
    TestProcess::named("state::tests::daemon_events::setup_entry")
        .unwrap()
        .nocapture()
        .single_threaded()
        .invoke_with(|args| {
            let mut config = Config::default();
            config.terminal.default_shell = "/bin/sh".into();
            config.terminal.new_cwd = NewTerminalCwdConfig::Path(
                maestro_test_directory::temporary_root()
                    .display()
                    .to_string(),
            );
            let (launch, receiver) = launch_inputs(args, manifest_cache().unwrap()).unwrap();
            let mut app = App::new(&config, || 0, launch, receiver, EventHub::default());
            block_on(async {
                body(&mut app).await;
                for (_, runtime) in app.terminal_runtimes.drain_for_handoff() {
                    runtime.shutdown().await;
                }
            });
            Ok::<_, std::io::Error>(())
        })
        .unwrap();
}
