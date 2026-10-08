//! Author-facing events and results over the generated wire unions.
//!
//! Each event export returns the event as the handler left it together with the handler's
//! decision, unless the handler replaced it with another kind, in which case no event is
//! returned. An edit of the same kind survives a returned error. A cancellation signal
//! travels to the guest beside its event and is never handed back. Only the events that hold
//! such a capability differ from the wire; every other payload is the generated record itself.
#![forbid(
    clippy::pedantic,
    clippy::too_many_arguments,
    clippy::excessive_nesting
)]

use std::ops::{Deref, DerefMut};
use std::rc::Rc;

use super::context::{AbortSignal, ExtensionContext};
use super::extension_result::ExtensionFuture;
use crate::bindings::maestro::extension::events::{
    InputEvent, InputEventResult, SessionBeforeCompactEventData, SessionBeforeCompactResult,
};

/// A compaction is about to happen: the generated data plus the cancellation signal the
/// handler may retain.
#[derive(Debug)]
pub struct SessionBeforeCompactEvent {
    /// The generated event data.
    pub data: SessionBeforeCompactEventData,
    /// Reports whether the host cancelled the compaction.
    pub signal: AbortSignal,
}

impl Deref for SessionBeforeCompactEvent {
    type Target = SessionBeforeCompactEventData;

    fn deref(&self) -> &Self::Target {
        &self.data
    }
}

impl DerefMut for SessionBeforeCompactEvent {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.data
    }
}

/// Session events.
#[derive(Debug)]
pub enum SessionEvent {
    /// A compaction is about to happen.
    BeforeCompact(SessionBeforeCompactEvent),
}

/// An event delivered to handlers, in the shape authors write against.
#[derive(Debug)]
pub enum ExtensionEvent {
    /// A session event.
    Session(SessionEvent),
    /// A user input before agent processing.
    Input(InputEvent),
}

/// A handler's verdict or replacement.
#[derive(Debug)]
pub enum ExtensionEventResult {
    /// Verdict or replacement of a compaction handler.
    SessionBeforeCompact(SessionBeforeCompactResult),
    /// What an input handler decided.
    Input(InputEventResult),
}

/// A registered event handler. It receives the event mutably and answers with an optional
/// result; the adapter returns the event as the handler left it, whether the handler
/// answered or failed, unless it replaced the event with another kind, leaving no event
/// to return.
pub type ExtensionHandler = Rc<
    dyn for<'a> Fn(
        &'a mut ExtensionEvent,
        ExtensionContext,
    ) -> ExtensionFuture<'a, Option<ExtensionEventResult>>,
>;
