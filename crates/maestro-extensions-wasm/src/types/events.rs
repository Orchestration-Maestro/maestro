//! Author-facing events and results over the generated wire unions.
//!
//! The wire unions carry cancellation signals beside the event, so an edit a handler makes
//! survives a returned error and capabilities an author retains are never handed back. Only
//! the events that hold such a capability differ from the wire; every other payload is the
//! generated record itself.
#![forbid(
    clippy::pedantic,
    clippy::too_many_arguments,
    clippy::excessive_nesting
)]

use std::ops::{Deref, DerefMut};
use std::rc::Rc;

use super::context::{AbortSignal, ExtensionContext};
use super::extension_result::ExtensionFuture;
use crate::bindings::maestro::extension::events as wire;
use wire::{
    InputEvent, InputEventResult, SessionBeforeCompactEventData, SessionBeforeCompactResult,
};

/// A compaction is about to happen: the generated data plus the cancellation signal the
/// handler may retain.
#[derive(Debug)]
pub struct SessionBeforeCompactEvent {
    /// The generated event data.
    pub data: SessionBeforeCompactEventData,
    /// Cancels the compaction when the user aborts it.
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

/// A result a handler returns, matching the event it handled.
#[derive(Debug)]
pub enum ExtensionEventResult {
    /// Verdict or replacement of a compaction handler.
    SessionBeforeCompact(SessionBeforeCompactResult),
    /// What an input handler decided.
    Input(InputEventResult),
}

impl ExtensionEventResult {
    /// The wire union for this result.
    #[must_use]
    pub fn into_wire(self) -> wire::ExtensionEventResult {
        match self {
            Self::SessionBeforeCompact(result) => {
                wire::ExtensionEventResult::SessionBeforeCompact(result)
            }
            Self::Input(result) => wire::ExtensionEventResult::Input(result),
        }
    }
}

/// A registered event handler. It receives the event mutably, so an edit survives a
/// returned error, and answers with an optional result.
pub type ExtensionHandler = Rc<
    dyn for<'a> Fn(
        &'a mut ExtensionEvent,
        ExtensionContext,
    ) -> ExtensionFuture<'a, Option<ExtensionEventResult>>,
>;
