//! The command-line startup entry over its existing application owners.

/// The extracted native pair runs work on the supplied local set without starting input.
#[cfg(unix)]
#[test]
fn native_selector_terminal_drives_the_supplied_local_set() {
    use std::rc::Rc;
    let local = Rc::new(tokio::task::LocalSet::new());
    let (terminal, runtime) = maestro_cli::input::selector::native_terminal(Rc::clone(&local));
    let (sender, receiver) = tokio::sync::oneshot::channel();
    runtime.spawn_local(Box::pin(async move {
        sender.send(()).unwrap();
        Ok(())
    }));
    crate::support::startup::runtime()
        .block_on(local.run_until(receiver))
        .unwrap();
    drop(terminal);
}
