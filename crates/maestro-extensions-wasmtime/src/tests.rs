//! Real domain owners shared by the adapter's unit witnesses.
use maestro_extensions::{ExtensionRunner, Local, create_extension_runtime};
use std::{rc::Rc, sync::RwLock};

pub(crate) mod api;
mod bash_output;
mod callbacks;
mod catalog;
mod commands;
mod compaction;
mod effects;
mod event_handlers;
mod events;
mod fault_replies;
mod history;
mod registrations;
mod replaced;
mod returned_bash;
mod tools;
mod tree;
mod unknown_events;
mod writer;

#[path = "../../maestro-extensions/tests/support/model.rs"]
pub(super) mod model_support;

#[path = "../../maestro-extensions/tests/support/session.rs"]
mod session;
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
