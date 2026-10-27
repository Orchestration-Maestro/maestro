//! Admitted command work keeps its selected operation while other requests progress.
use super::runner;
use crate::read_imports::ReadEndpoint;
use crate::{
    command_reads::Request,
    context_reads::{Owner, Request as Read},
    local_resources::Pump,
};
use maestro_extensions::ExtensionCommandContextActions;
use std::{cell::Cell, rc::Rc};
use tokio::sync::oneshot;

mod admissions;
mod reentry;
mod sessions;

#[test]
fn admitted_wait_survives_retirement_without_blocking_reads() {
    tokio::runtime::Builder::new_current_thread()
        .build()
        .unwrap()
        .block_on(admitted_wait());
}

/// Suspends real selected work, services a read, then resumes after retirement.
async fn admitted_wait() {
    let runner = runner("/command-owner");
    let starts = Rc::new(Cell::new(0));
    let (resume, resumed) = oneshot::channel();
    let resumed = Rc::new(std::cell::RefCell::new(Some(resumed)));
    runner.bind_command_context(Some(ExtensionCommandContextActions::new(
        maestro_extensions::CommandContextOperations {
            wait_for_idle: Rc::new({
                let starts = starts.clone();
                move || {
                    starts.set(starts.get() + 1);
                    let resumed = resumed.borrow_mut().take().unwrap();
                    Box::pin(async move { resumed.await.map_err(Into::into) })
                }
            }),
            ..crate::context_actions::silent_command_operations()
        },
    )));
    let (mut pump, admission) = Pump::new(crate::tests::support::widgets());
    let endpoint = pump
        .registry()
        .context(Owner::Command(runner.create_command_context()))
        .unwrap();
    let waiting = endpoint.clone();
    let worker = std::thread::spawn(move || {
        tokio::runtime::Builder::new_current_thread()
            .build()
            .unwrap()
            .block_on(async {
                waiting.command(Request::Wait).await.unwrap();
                assert_eq!(
                    waiting.command(Request::Reload).await.unwrap_err(),
                    "retired wait"
                );
            });
    });
    assert!(pump.answer_next().await);
    assert_eq!(starts.get(), 1);
    let reader = std::thread::spawn(move || {
        assert_eq!(endpoint.read(Read::Cwd).unwrap(), "/command-owner");
    });
    assert!(pump.answer_next().await);
    reader.join().unwrap();
    runner.bind_command_context(None);
    runner.invalidate(Some("retired wait".into()));
    resume.send(()).unwrap();
    drop(admission);
    pump.serve().await;
    worker.join().unwrap();
    assert_eq!(starts.get(), 1);
}

#[test]
fn reload_uses_the_operation_selected_when_consumed() {
    tokio::runtime::Builder::new_current_thread()
        .build()
        .unwrap()
        .block_on(async {
            let runner = runner("/reload");
            let (mut pump, admission) = Pump::new(crate::tests::support::widgets());
            let endpoint = pump
                .registry()
                .context(Owner::Command(runner.create_command_context()))
                .unwrap();
            runner.bind_command_context(Some(ExtensionCommandContextActions::new(
                maestro_extensions::CommandContextOperations {
                    reload: Rc::new(|| Box::pin(async { Err("selected reload".into()) })),
                    ..crate::context_actions::silent_command_operations()
                },
            )));
            let worker = std::thread::spawn(move || {
                tokio::runtime::Builder::new_current_thread()
                    .build()
                    .unwrap()
                    .block_on(async {
                        assert_eq!(
                            endpoint.command(Request::Reload).await.unwrap_err(),
                            "selected reload"
                        );
                    });
            });
            assert!(pump.answer_next().await);
            runner.bind_command_context(None);
            drop(admission);
            pump.serve().await;
            worker.join().unwrap();
        });
}

#[test]
fn navigation_preserves_exact_target_and_optional_fields() {
    tokio::runtime::Builder::new_current_thread()
        .build()
        .unwrap()
        .block_on(navigation());
}

/// Decodes empty and populated records through the same shared native option type.
async fn navigation() {
    let runner = runner("/navigate");
    runner.bind_command_context(Some(ExtensionCommandContextActions::new(
        maestro_extensions::CommandContextOperations {
            navigate_tree: Rc::new(|id, options| {
                assert_eq!(id, "  e\u{feff}\u{85} ");
                if let Some(options) = options {
                    check_navigation_options(&options);
                }
                Box::pin(async {
                    Ok(maestro_extensions::types::SessionChangeResult { cancelled: true })
                })
            }),
            ..crate::context_actions::silent_command_operations()
        },
    )));
    let (pump, admission) = Pump::new(crate::tests::support::widgets());
    let endpoint = pump
        .registry()
        .context(Owner::Command(runner.create_command_context()))
        .unwrap();
    let worker = std::thread::spawn(move || {
        tokio::runtime::Builder::new_current_thread().build().unwrap().block_on(async {
            for options in [None, Some("{}"), Some(r#"{"summarize":true,"customInstructions":" focus ","replaceInstructions":false,"label":" checkpoint "}"#)] {
                let options = options.map(crate::writer_reads::decode::<maestro_extensions::NavigateTreeOptions>).transpose().unwrap();
                assert!(endpoint.command(|reply| Request::Navigate("  e\u{feff}\u{85} ".into(), options, reply)).await.unwrap().cancelled);
            }
        });
    });
    drop(admission);
    pump.serve().await;
    worker.join().unwrap();
}

/// Checks absence independently of explicit false and whitespace-bearing strings.
fn check_navigation_options(options: &maestro_extensions::NavigateTreeOptions) {
    if options.summarize.is_some() {
        assert_eq!(options.summarize, Some(true));
        assert_eq!(options.custom_instructions.as_deref(), Some(" focus "));
        assert_eq!(options.replace_instructions, Some(false));
        assert_eq!(options.label.as_deref(), Some(" checkpoint "));
    } else {
        assert_eq!(options.custom_instructions, None);
        assert_eq!(options.replace_instructions, None);
        assert_eq!(options.label, None);
    }
}
