//! Shared context recorder and forwarding assertions.
#![cfg(test)]
use super::model_support::model;
use maestro_extensions::{CompactOptions, ContextUsage, ExtensionContext, ExtensionContextActions};
use maestro_models::{Cancellation, Model};
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
    sync::{Arc, RwLock},
};
/// A shared model handle.
pub type SharedModel = Arc<RwLock<Model>>;
/// Names of the bound operations called, in order.
type Log = Rc<RefCell<Vec<&'static str>>>;

/// Whether two optional model handles are the same model.
pub fn same_model(actual: Option<SharedModel>, expected: &SharedModel) -> bool {
    actual.is_some_and(|actual| Arc::ptr_eq(&actual, expected))
}

/// Bound operations that log their own name and return fixed values.
pub struct Recorder {
    /// Operation names in call order.
    pub log: Log,
    /// Compaction requests in call order.
    pub compacts: Rc<RefCell<Vec<Option<CompactOptions>>>>,
    /// The model the reader returns.
    pub model: SharedModel,
    /// The cancellation the signal reader returns.
    pub signal: Cancellation,
}
impl Recorder {
    /// A recorder with an empty log.
    pub fn new() -> Self {
        Self {
            log: Log::default(),
            compacts: Rc::default(),
            model: model("bound"),
            signal: Cancellation::new(),
        }
    }
    /// Takes the names logged since the last call.
    pub fn take(&self) -> Vec<&'static str> {
        std::mem::take(&mut *self.log.borrow_mut())
    }
    /// Operations returning this recorder's values after logging their names.
    pub fn actions(&self) -> ExtensionContextActions {
        let log = |name| {
            let log = Rc::clone(&self.log);
            move || log.borrow_mut().push(name)
        };
        let (model, signal, compacts) = (
            Arc::clone(&self.model),
            self.signal.clone(),
            Rc::clone(&self.compacts),
        );
        let (get_model, is_idle, get_signal) =
            (log("get_model"), log("is_idle"), log("get_signal"));
        let (abort, pending, shutdown) =
            (log("abort"), log("has_pending_messages"), log("shutdown"));
        let (usage, compact) = (log("get_context_usage"), log("compact"));
        let prompt = log("get_system_prompt");
        ExtensionContextActions {
            get_model: Rc::new(move || {
                get_model();
                Ok(Some(Arc::clone(&model)))
            }),
            is_idle: Rc::new(move || {
                is_idle();
                Ok(false)
            }),
            get_signal: Rc::new(move || {
                get_signal();
                Ok(Some(signal.clone()))
            }),
            abort: Rc::new(move || {
                abort();
                Ok(())
            }),
            has_pending_messages: Rc::new(move || {
                pending();
                Ok(true)
            }),
            shutdown: Rc::new(move || {
                shutdown();
                Ok(())
            }),
            get_context_usage: Rc::new(move || {
                usage();
                Ok(Some(ContextUsage {
                    tokens: None,
                    context_window: 200_000.0,
                    percent: None,
                }))
            }),
            compact: Rc::new(move |options| {
                compact();
                compacts.borrow_mut().push(options);
                Ok(())
            }),
            get_system_prompt: Rc::new(move || {
                prompt();
                Ok("bound prompt".to_owned())
            }),
        }
    }
}

/// A full compaction request whose callbacks reject and record any call.
pub fn full_compaction(called: &Rc<Cell<bool>>) -> CompactOptions {
    let (complete, error) = (Rc::clone(called), Rc::clone(called));
    CompactOptions {
        custom_instructions: Some("keep".into()),
        on_complete: Some(Rc::new(move |_| {
            complete.set(true);
            Err("unexpected completion".into())
        })),
        on_error: Some(Rc::new(move |_| {
            error.set(true);
            Err("unexpected failure".into())
        })),
    }
}

/// Each forwarding operation returns its bound value and calls only itself.
pub fn check_bound_operations(ctx: &ExtensionContext, recorder: &Recorder) {
    assert!(same_model(ctx.model().unwrap(), &recorder.model));
    assert_eq!(recorder.take(), ["get_model"]);
    assert!(!ctx.is_idle().unwrap());
    assert_eq!(recorder.take(), ["is_idle"]);
    assert!(ctx.signal().unwrap().is_some());
    assert_eq!(recorder.take(), ["get_signal"]);
    ctx.abort().unwrap();
    assert_eq!(recorder.take(), ["abort"]);
    assert!(ctx.has_pending_messages().unwrap());
    assert_eq!(recorder.take(), ["has_pending_messages"]);
    ctx.shutdown().unwrap();
    assert_eq!(recorder.take(), ["shutdown"]);
    let usage = ctx.get_context_usage().unwrap();
    let expected = ContextUsage {
        tokens: None,
        context_window: 200_000.0,
        percent: None,
    };
    assert_eq!(usage, Some(expected));
    assert_eq!(recorder.take(), ["get_context_usage"]);
    assert_eq!(ctx.get_system_prompt().unwrap(), "bound prompt");
    assert_eq!(recorder.take(), ["get_system_prompt"]);
}

/// Forwards absent, default and full compaction requests unchanged.
pub fn check_compaction_forwarding(ctx: &ExtensionContext, recorder: &Recorder) {
    let called = Rc::new(Cell::new(false));
    let full = full_compaction(&called);
    let (complete, error) = (
        full.on_complete.clone().unwrap(),
        full.on_error.clone().unwrap(),
    );
    let start = recorder.compacts.borrow().len();
    ctx.compact(None).unwrap();
    ctx.compact(Some(CompactOptions::default())).unwrap();
    ctx.compact(Some(full)).unwrap();
    assert_eq!(recorder.take(), ["compact", "compact", "compact"]);
    let compacts = recorder.compacts.borrow();
    let compacts = &compacts[start..];
    assert!(compacts[0].is_none());
    let default = compacts[1].as_ref().unwrap();
    assert!(default.custom_instructions.is_none());
    assert!(default.on_complete.is_none() && default.on_error.is_none());
    let forwarded = compacts[2].as_ref().unwrap();
    assert_eq!(forwarded.custom_instructions.as_deref(), Some("keep"));
    assert!(Rc::ptr_eq(
        forwarded.on_complete.as_ref().unwrap(),
        &complete
    ));
    assert!(Rc::ptr_eq(forwarded.on_error.as_ref().unwrap(), &error));
    assert!(!called.get());
}

pub use crate::support::command_operations::silent_command_operations;
