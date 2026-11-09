//! Replacement targets keep their own ownership while a later replacement runs.
#![cfg(test)]
use maestro_extensions::{
    CommandContextOperations, ExtensionCommandContextActions, ExtensionContextActions,
    ExtensionExecution, ExtensionRunner, Local, NewSessionHandler, ReplacedSessionContext,
    create_event_bus_for,
    types::{ProviderActions, SessionChangeResult},
};
use serde_json::json;
use std::{cell::RefCell, rc::Rc};

use super::{components::local, retained_components::report};

/// The stale guidance every retired read reports.
const STALE: &str = super::retained_contexts::STALE;

/// Every replacement target this run supplied, in arrival order.
struct Host {
    /// One runner per target, so a target can be retired by identity.
    targets: RefCell<Vec<Rc<ExtensionRunner<Local>>>>,
    /// Whether the second call retires the first target before its continuation.
    retire: bool,
}

/// Loads the component over its own origin session.
async fn loaded() -> ExtensionRunner<Local> {
    let bus = create_event_bus_for::<Local>(Rc::new(|_| {}), Rc::new(|_| {}));
    let (session, _) = super::session::runner_session("/target-origin");
    let (_, runner, _) = super::retained_components::loaded_in(bus.for_extension(), session).await;
    runner
}

/// Builds one fresh target runner, binds the replacement operation and keeps it.
fn target(host: &Rc<Host>, label: &str) -> ReplacedSessionContext {
    let session = super::session::runner_session(&format!("/target-{label}")).0;
    let runner = Rc::new(ExtensionRunner::<Local>::new(
        Rc::new(std::sync::RwLock::new(Vec::new())),
        maestro_extensions::create_extension_runtime::<Local>(),
        session,
        Rc::new(|_| {}),
        maestro_theme::LiveTheme::default(),
    ));
    runner.bind_command_context(Some(ExtensionCommandContextActions::new(
        CommandContextOperations {
            new_session: handler(host),
            ..super::context_actions::silent_command_operations()
        },
    )));
    let replaced = ReplacedSessionContext::new(
        runner.create_command_context(),
        Rc::new(|_, _| Box::pin(async { Ok(()) })),
        Rc::new(|_, _| Box::pin(async { Ok(()) })),
    );
    host.targets.borrow_mut().push(runner);
    replaced
}

#[test]
fn overlapping_continuations_keep_distinct_target_owners() {
    local(async {
        let runner = loaded().await;
        let host = Rc::new(Host {
            targets: RefCell::default(),
            retire: false,
        });
        runner.bind_command_context(Some(ExtensionCommandContextActions::new(
            CommandContextOperations {
                new_session: handler(&host),
                ..super::context_actions::silent_command_operations()
            },
        )));
        let answer = report(&runner, "retention chain").await;
        assert_eq!(answer["resolved"]["cancelled"], json!(false));
        assert_eq!(
            answer["resolved"]["trace"],
            json!([
                "nested-continuation:cwd=/target-b",
                "outer:before=cwd=/target-a:after=cwd=/target-a",
                "nested:false",
            ])
        );
        assert_eq!(answer["retained"], json!("cwd=/target-a"));
        assert_eq!(host.targets.borrow().len(), 2);
    });
}

#[test]
fn second_replacement_stales_only_the_retired_target() {
    local(async {
        let runner = loaded().await;
        let host = Rc::new(Host {
            targets: RefCell::default(),
            retire: true,
        });
        runner.bind_command_context(Some(ExtensionCommandContextActions::new(
            CommandContextOperations {
                new_session: handler(&host),
                ..super::context_actions::silent_command_operations()
            },
        )));
        let answer = report(&runner, "retention chain").await;
        assert_eq!(answer["resolved"]["cancelled"], json!(false));
        let stale = format!("err={STALE}");
        assert_eq!(
            answer["resolved"]["trace"],
            json!([
                "nested-continuation:cwd=/target-b",
                format!("outer:before=cwd=/target-a:after={stale}"),
                "nested:false",
            ])
        );
        assert_eq!(answer["retained"], json!(stale));
    });
}

/// Supplies target sessions and retires the first one when the second call starts.
fn handler(host: &Rc<Host>) -> NewSessionHandler {
    let host = host.clone();
    Rc::new(move |options| {
        let host = host.clone();
        Box::pin(async move {
            let label = match host.targets.borrow().len() {
                0 => "a",
                _ => "b",
            };
            if host.retire && label == "b" {
                let first = host.targets.borrow()[0].clone();
                retire(&first);
            }
            let replaced = target(&host, label);
            let continuation = options
                .and_then(|options| options.with_session)
                .expect("a continuation");
            continuation(replaced).await?;
            Ok(SessionChangeResult { cancelled: false })
        })
    })
}

/// Retires the first target twice, which a target keeps handling once stale.
fn retire(first: &ExtensionRunner<Local>) {
    for _ in 0..2 {
        first.invalidate(None);
    }
}

#[test]
fn origin_and_target_aliases_release_without_cycles() {
    local(async {
        let (origin_errors, origin_teardown) = super::lifetimes::observed_reporter(&Rc::default());
        let origin_bus = create_event_bus_for::<Local>(Rc::new(|_| {}), origin_errors);
        let (origin_owner, origin_runner, origin_events) =
            super::lifetimes::load(origin_bus.for_extension(), "/alias-origin").await;
        let (target_errors, target_teardown) = super::lifetimes::observed_reporter(&Rc::default());
        let target_bus = create_event_bus_for::<Local>(Rc::new(|_| {}), target_errors);
        let (target_owner, target_runner, target_events) =
            super::lifetimes::load(target_bus.for_extension(), "/alias-target").await;
        let origin = origin_runner.create_command_context();
        let target = ReplacedSessionContext::new(
            target_runner.create_command_context(),
            Rc::new(|_, _| Box::pin(async { Ok(()) })),
            Rc::new(|_, _| Box::pin(async { Ok(()) })),
        );
        origin_bus.clear();
        drop((origin_owner, origin_runner, origin_events, origin_bus));
        assert_eq!(origin.cwd().unwrap(), "/alias-origin");
        assert_eq!(target.cwd().unwrap(), "/alias-target");
        origin_teardown.await.unwrap();
        drop(origin);
        target_bus.clear();
        drop((target_owner, target_runner, target_events, target_bus));
        assert_eq!(target.cwd().unwrap(), "/alias-target");
        target_teardown.await.unwrap();
        drop(target);
    });
}

#[test]
fn activation_failure_preserves_independent_registrations() {
    local(async {
        use super::provider_catalog::{provider_config, queued_runtime};
        let (session, _) = super::session::runner_session("/activation-failure");
        let catalog = session.model_registry.clone();
        let runtime = queued_runtime(&json!([
            {"name":"first-valid", "config":provider_config("kept-first")},
            {"name":"refused", "config":provider_config("refused-model")},
            {"name":"last-valid", "config":provider_config("kept-last")},
        ]));
        let events =
            create_event_bus_for::<Local>(Rc::new(|_| {}), Rc::new(|_| {})).for_extension();
        let (_, runner, _) = super::retained_components::unbound_with_runtime(
            events,
            session,
            "/activation-failure",
            runtime.clone(),
        )
        .await;
        let errors = Rc::new(RefCell::new(Vec::new()));
        let seen = errors.clone();
        drop(runner.on_error(Rc::new(move |error| {
            seen.borrow_mut().push((
                error.extension_path.clone(),
                error.event.clone(),
                error.error.clone(),
            ));
            Ok(())
        })));
        bind_refusing(&runner, &catalog);
        assert_eq!(
            *errors.borrow(),
            [(
                std::env::var("MAESTRO_CONTEXT_COMPONENT").unwrap(),
                "register_provider".to_owned(),
                "supplied activation refused".to_owned()
            )]
        );
        assert!(catalog.find("refused", "refused-model").is_none());
        assert!(
            runtime
                .pending_provider_registrations
                .read()
                .unwrap()
                .is_empty()
        );
        assert_unrelated_providers_kept(&catalog, "after the failed activation");
        later_registrations(&runner).await;
        assert_unrelated_providers_kept(&catalog, "after later registrations");
    });
}

/// Binds a supplied registration operation that refuses one provider and registers the rest.
fn bind_refusing(
    runner: &ExtensionRunner<Local>,
    catalog: &maestro_extensions::SharedHandle<maestro_catalog::ModelRegistry>,
) {
    let native = catalog.clone();
    runner
        .bind_core(
            Local::unbound_actions(),
            ExtensionContextActions::default(),
            ProviderActions {
                register_provider: Some(Rc::new(move |name, config| {
                    if name.contains("refused") {
                        return Err("supplied activation refused".into());
                    }
                    Ok(native.register_provider(&name, config)?)
                })),
                unregister_provider: None,
            },
        )
        .unwrap();
}

/// Both valid factory providers stay registered, the one before and the one after the refusal.
fn assert_unrelated_providers_kept(
    catalog: &maestro_extensions::SharedHandle<maestro_catalog::ModelRegistry>,
    when: &str,
) {
    assert!(
        catalog.find("first-valid", "kept-first").is_some(),
        "earlier provider {when}"
    );
    assert!(
        catalog.find("last-valid", "kept-last").is_some(),
        "later provider {when}"
    );
}

/// A later refusal reports to its own caller while a later valid registration still applies.
async fn later_registrations(runner: &ExtensionRunner<Local>) {
    use super::provider_catalog::{provider_call, provider_config};
    let late = provider_call(
        runner,
        json!({"op":"register","provider":"refused-late","config":provider_config("late")}),
    )
    .await;
    assert_eq!(late["error"], json!("supplied activation refused"));
    assert_eq!(late["models"], json!([]));
    let valid = provider_call(
        runner,
        json!({"op":"register","provider":"late-valid","config":provider_config("late")}),
    )
    .await;
    assert_eq!(valid["error"], serde_json::Value::Null);
    assert_eq!(valid["models"][0]["id"], json!("late"));
}
