//! Suspended canonical commands retain independently acquired native capabilities.
#![cfg(test)]
use maestro_extensions::types::ExtensionUIContextPort;
use maestro_extensions::{
    ExtensionCommandContextActions, ExtensionContextActions, ExtensionExecution, ExtensionRunner,
    ExtensionUIContext, Local, create_event_bus_for, types::ProviderActions,
};
use serde_json::{Value, json};
use std::{
    cell::RefCell,
    rc::Rc,
    sync::{Arc, RwLock},
};

/// Complete branded guidance, unchanged across the canonical component boundary.
pub(super) const STALE: &str = "This extension ctx is stale after session replacement or reload. Do not use a captured maestro or command ctx after ctx.newSession(), ctx.fork(), ctx.switchSession(), or ctx.reload(). For newSession, fork, and switchSession, move post-replacement work into withSession and use the ctx passed to withSession. For reload, do not use the old ctx after await ctx.reload().";

/// Signals final release of the original UI rather than replacement of its binding.
struct Ui(RefCell<Option<tokio::sync::oneshot::Sender<()>>>);
impl ExtensionUIContextPort for Ui {
    fn theme(&self) -> maestro_theme::LiveTheme {
        maestro_theme::LiveTheme::default()
    }
}
impl Drop for Ui {
    fn drop(&mut self) {
        self.0.get_mut().take().unwrap().send(()).unwrap();
    }
}

/// Invalidates while canonical imports are suspended; optionally admits another same-Store call.
pub(super) async fn retained(with_healthy: bool) {
    let bus = create_event_bus_for::<Local>(
        Rc::new(|_| panic!("no tails")),
        Rc::new(|_| panic!("no errors")),
    );
    let (_subscription, caught_error) = caught(&bus.for_extension());
    let (runner, history) = load(bus.for_extension()).await;
    let (released, teardown) = tokio::sync::oneshot::channel();
    let ui = Rc::new(Ui(RefCell::new(Some(released))));
    let original_ui = Rc::downgrade(&ui);
    runner.set_ui_context(Some(ExtensionUIContext::new(ui)));
    let (entered, mut admitted) = tokio::sync::mpsc::unbounded_channel();
    runner.bind_command_context(Some(ExtensionCommandContextActions::new(
        maestro_extensions::CommandContextOperations {
            wait_for_idle: wait(entered.clone(), false),
            reload: wait(entered, true),
            ..crate::context_actions::silent_command_operations()
        },
    )));
    let command = runner
        .get_command("bridge-observe")
        .unwrap()
        .command
        .handler;
    let (retired, (), ()) = futures_util::future::join3(
        command("retain-suspended".into(), runner.create_command_context()),
        async {
            if with_healthy {
                command("healthy-admitted".into(), runner.create_command_context())
                    .await
                    .unwrap();
            }
        },
        async {
            let mut resumes = vec![admitted.recv().await.unwrap()];
            if with_healthy {
                resumes.push(admitted.recv().await.unwrap());
            }
            retire(&runner, &history, &original_ui, resumes, caught_error).await;
        },
    )
    .await;
    let observed: Value = serde_json::from_str(&retired.unwrap_err().to_string()).unwrap();
    assert_eq!(
        observed,
        json!({"stale":STALE,"history":"live original history", "cwd":"/retained-original",
        "catalog":["live original catalog"], "aborted":true, "model":"owned before suspension"})
    );
    teardown.await.unwrap();
    assert!(
        original_ui.upgrade().is_none(),
        "final consumer failed to release the old UI"
    );
}

/// Installs an unequal replacement while retained raw handles still own their originals.
fn bind(
    runner: &ExtensionRunner<Local>,
    model: Arc<RwLock<maestro_models::Model>>,
    signal: maestro_extensions::Cancellation,
) {
    runner
        .bind_core(
            Local::unbound_actions(),
            ExtensionContextActions {
                get_model: Rc::new(move || Ok(Some(model.clone()))),
                get_signal: Rc::new(move || Ok(Some(signal.clone()))),
                ..Default::default()
            },
            ProviderActions::default(),
        )
        .unwrap();
}

/// Labels independent native admissions so healthy work resumes only after the caught import.
fn wait(
    entered: tokio::sync::mpsc::UnboundedSender<(bool, tokio::sync::oneshot::Sender<()>)>,
    healthy: bool,
) -> maestro_extensions::types::ReloadHandler {
    Rc::new(move || {
        let (resume, done) = tokio::sync::oneshot::channel();
        entered.send((healthy, resume)).unwrap();
        Box::pin(async move {
            done.await?;
            Ok(())
        })
    })
}

/// Seeds the real history, catalog and model before the guest acquires their capabilities.
async fn load(
    events: maestro_extensions::EventBus<Local>,
) -> (
    ExtensionRunner<Local>,
    Rc<RefCell<maestro_session::SessionManager>>,
) {
    let (session, history) = super::session::runner_session("/retained-original");
    let catalog = session.model_registry.clone();
    let model = super::model_support::model("owned before suspension");
    *catalog.get_all().write().unwrap() = vec![model.clone()];
    let (_, runner, _) = super::retained_components::loaded_in(events, session).await;
    let signal = maestro_extensions::Cancellation::new();
    bind(&runner, model, signal);
    (runner, history)
}

/// Releases the old wait before the caught-error signal releases independently admitted work.
async fn retire(
    runner: &ExtensionRunner<Local>,
    history: &Rc<RefCell<maestro_session::SessionManager>>,
    original_ui: &std::rc::Weak<Ui>,
    mut resumes: Vec<(bool, tokio::sync::oneshot::Sender<()>)>,
    caught_error: tokio::sync::oneshot::Receiver<String>,
) {
    let context = runner.create_context();
    let signal = context.signal().unwrap().unwrap();
    let model = context.model().unwrap().unwrap();
    let catalog = context.model_registry().unwrap();
    runner.invalidate(None);
    assert_eq!(
        runner.create_context().cwd().unwrap_err().to_string(),
        STALE
    );
    runner.set_ui_context(None);
    assert!(
        original_ui.upgrade().is_some(),
        "guest lost the acquired UI during suspension"
    );
    signal.abort();
    bind(
        runner,
        super::model_support::model("new binding"),
        maestro_extensions::Cancellation::new(),
    );
    model.write().unwrap().name = "mutated native model".into();
    *catalog.get_all().write().unwrap() =
        vec![super::model_support::model("live original catalog")];
    history
        .borrow_mut()
        .append_session_info("live original history")
        .unwrap();
    let position = resumes.iter().position(|(healthy, _)| !healthy).unwrap();
    resumes.swap_remove(position).1.send(()).unwrap();
    assert_eq!(caught_error.await.unwrap(), STALE);
    for (healthy, resume) in resumes {
        assert!(healthy);
        resume.send(()).unwrap();
    }
}

/// Receives the guest's caught inner error before releasing the independent native operation.
fn caught(
    events: &maestro_extensions::EventBus<Local>,
) -> (
    maestro_extensions::Subscription<Local>,
    tokio::sync::oneshot::Receiver<String>,
) {
    let (caught, caught_error) = tokio::sync::oneshot::channel();
    let caught = RefCell::new(Some(caught));
    let subscription = events.on(
        "retained-caught",
        Rc::new(move |data, _| {
            caught
                .borrow_mut()
                .take()
                .unwrap()
                .send(data.as_str().unwrap().to_owned())
                .unwrap();
            Ok(None)
        }),
    );
    (subscription, caught_error)
}
