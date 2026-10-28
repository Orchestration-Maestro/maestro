//! The shipped sandbox through the application's native component composition.
#![cfg(test)]
use maestro_app::AgentSession;
use std::rc::Rc;

/// Runs on the executor retained by the application's native operations.
fn run<F: Future<Output = ()>>(action: impl FnOnce(Rc<tokio::task::LocalSet>) -> F) {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    let local = Rc::new(tokio::task::LocalSet::new());
    local.block_on(&runtime, action(local.clone()));
}

/// Loads the actual component with the production native I/O provider.
async fn loaded(root: &super::support::Root, local: Rc<tokio::task::LocalSet>) -> Rc<AgentSession> {
    let session = super::support::session::component_session(
        root,
        local,
        std::env::var("MAESTRO_SANDBOX_COMPONENT").unwrap(),
        None,
        super::support::widgets(),
    )
    .await;
    let _remove = session.extension_runner().on_error(Rc::new(|error| {
        panic!("unexpected sandbox callback error: {error:?}")
    }));
    session
}

#[cfg(target_os = "linux")]
mod linux;

#[test]
fn sandbox_registers_opt_in_controls_and_bash_override() {
    #[cfg(target_os = "linux")]
    if linux::isolated("sandbox_registers_opt_in_controls_and_bash_override") {
        return;
    }
    run(|local| async move {
        let root = super::support::Root::new();
        let session = loaded(&root, local).await;
        let runner = session.extension_runner();
        assert!(runner.get_flags().contains_key("no-sandbox"));
        assert!(runner.get_command("sandbox").is_some());
        let tools = runner.get_all_registered_tools();
        assert_eq!(tools.len(), 1);
        assert_eq!(tools[0].metadata().definition.name, "bash");
        assert_eq!(tools[0].metadata().label, "bash (sandboxed)");
        #[cfg(target_os = "linux")]
        linux::shutdown(&runner).await;
        session.dispose();
    });
}
