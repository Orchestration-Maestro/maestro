//! Real domain owners shared by the adapter's unit witnesses.
use maestro_extensions::{ExtensionRunner, Local, create_extension_runtime};
use std::{rc::Rc, sync::RwLock};

#[path = "../../maestro-extensions/tests/support/command_operations.rs"]
pub(super) mod context_actions;

pub(crate) mod api;
mod bash_output;
mod builtin_author;
mod builtin_factories;
mod builtin_local_shell;
mod builtin_operations;
mod callbacks;
mod catalog;
mod commands;
mod compaction;
mod credentials;
mod effects;
mod event_handlers;
mod events;
mod fault_replies;
mod history;
mod registrations;
mod replaced;
mod returned_bash;
mod schema;
mod tools;
mod tree;
mod unknown_events;
mod writer;

#[path = "../../maestro-extensions/tests/support/model.rs"]
pub(super) mod model_support;

#[path = "../../maestro-extensions/tests/support/session.rs"]
pub(crate) mod session;
#[path = "../tests/support/mod.rs"]
pub(super) mod support;

/// Creates a real runner with independent in-memory domain owners.
pub(super) fn runner(cwd: &str) -> ExtensionRunner<Local> {
    ExtensionRunner::<Local>::new(
        Rc::new(RwLock::default()),
        create_extension_runtime::<Local>(),
        session::runner_session(cwd).0,
        Rc::new(|_| {}),
        maestro_theme::LiveTheme::default(),
    )
}

/// A nested same-instance call as the Store receives it.
pub(super) type NestedCall = crate::guest_calls::Call<crate::guest_calls::Callback>;

/// Runs `invoke` in a native observer while a simulated Store thread waits in its routed
/// update import, answering each nested same-instance call the observer makes with `answer`.
pub(super) async fn during_routed_request<T: 'static>(
    pump: &mut crate::local_resources::Pump,
    invoke: impl FnOnce() -> T + 'static,
    mut answer: impl FnMut(NestedCall) + Send + 'static,
) -> T {
    let result = Rc::new(std::cell::RefCell::new(None));
    let slot = result.clone();
    let invoke = std::cell::RefCell::new(Some(invoke));
    let update = pump
        .registry()
        .tool_update(Rc::new(move |_| {
            if let Some(invoke) = invoke.borrow_mut().take() {
                *slot.borrow_mut() = Some(invoke());
            }
        }))
        .unwrap();
    let store = std::thread::spawn(move || {
        let mut route = update.update(r#"{"content":[]}"#.into());
        loop {
            match route.next() {
                Ok(nested) => answer(nested.call),
                Err(result) => break result.unwrap(),
            }
        }
    });
    while result.borrow().is_none() {
        assert!(pump.answer_next().await);
    }
    store.join().unwrap();
    result.borrow_mut().take().unwrap()
}
