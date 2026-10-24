//! The command operations a print run binds to the session now in effect.
use super::PrintRun;
use maestro_app::{ExtensionCommandContextActions, NavigateTreeOptions, SessionChangeResult};
use std::rc::{Rc, Weak};

/// The command operations, each read from the run when it runs.
pub(crate) fn command_actions(run: Weak<PrintRun>) -> ExtensionCommandContextActions {
    let (idle, new, fork, navigate, switch, reload) = (
        run.clone(),
        run.clone(),
        run.clone(),
        run.clone(),
        run.clone(),
        run,
    );
    ExtensionCommandContextActions {
        wait_for_idle: Rc::new(move || {
            let run = idle.clone();
            Box::pin(async move {
                run_of(&run)?.current_session().wait_for_idle().await?;
                Ok(())
            })
        }),
        new_session: Rc::new(move |options| {
            let run = new.clone();
            Box::pin(async move {
                Ok(run_of(&run)?
                    .runtime
                    .new_session(options.unwrap_or_default())
                    .await?)
            })
        }),
        fork: Rc::new(move |entry_id, options| {
            let run = fork.clone();
            Box::pin(async move {
                Ok(run_of(&run)?
                    .runtime
                    .fork(entry_id, options.unwrap_or_default())
                    .await?)
            })
        }),
        navigate_tree: Rc::new(move |target_id, options| {
            let run = navigate.clone();
            Box::pin(async move {
                let options = options.unwrap_or_default();
                let navigation = NavigateTreeOptions {
                    summarize: options.summarize,
                    custom_instructions: options.custom_instructions,
                    replace_instructions: options.replace_instructions,
                    label: options.label,
                };
                let navigated = run_of(&run)?
                    .current_session()
                    .navigate_tree(target_id, navigation)
                    .await?;
                Ok(SessionChangeResult {
                    cancelled: navigated.cancelled,
                })
            })
        }),
        switch_session: Rc::new(move |session_path, options| {
            let run = switch.clone();
            Box::pin(async move {
                Ok(run_of(&run)?
                    .runtime
                    .switch_session(session_path, options.unwrap_or_default())
                    .await?)
            })
        }),
        reload: Rc::new(move || {
            let run = reload.clone();
            Box::pin(async move {
                run_of(&run)?.current_session().reload().await?;
                Ok(())
            })
        }),
    }
}

/// The run a stored command operation belongs to, while its caller keeps it.
fn run_of(run: &Weak<PrintRun>) -> Result<Rc<PrintRun>, Box<dyn std::error::Error + Send + Sync>> {
    run.upgrade()
        .ok_or_else(|| "the print run has ended".into())
}
