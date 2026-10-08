//! A handler that replaces its event with an event of another kind has no edit to return.

use std::rc::Rc;

use super::controlled::{Controlled, Flag};
use super::guest_family as fixtures;
use crate::component_adapter::{Exports, release};
use crate::loader::Extension;
use crate::types::{
    AbortSignal, ExtensionAPI, ExtensionEvent, ExtensionFuture, ExtensionHandler,
    SessionBeforeCompactEvent, SessionEvent, SignalPort,
};

/// A signal that is never cancelled.
struct Live;

impl SignalPort for Live {
    fn aborted(&self) -> bool {
        false
    }
}

/// A handler that replaces its event with an input.
fn to_input() -> ExtensionHandler {
    Rc::new(|event, _ctx| {
        Box::pin(async move {
            *event = ExtensionEvent::Input(fixtures::input("replacement"));
            Ok(None)
        })
    })
}

/// A handler that replaces its event with a compaction.
fn to_compaction() -> ExtensionHandler {
    Rc::new(|event, _ctx| {
        Box::pin(async move {
            let compaction = SessionBeforeCompactEvent {
                data: fixtures::compact_data(),
                signal: AbortSignal::new(Rc::new(Live)),
            };
            *event = ExtensionEvent::Session(SessionEvent::BeforeCompact(compaction));
            Ok(None)
        })
    })
}

/// An extension whose handlers swap the kind of the event they receive.
struct Swapper;

impl Extension for Swapper {
    fn load(api: ExtensionAPI) -> ExtensionFuture<'static, ()> {
        Box::pin(async move {
            api.on("to_input", to_input())?;
            api.on("to_compaction", to_compaction())
        })
    }
}

#[test]
fn maestro_event_replaced_by_another_kind_has_no_edit_to_return() -> Result<(), String> {
    super::block_on(async {
        let host = Controlled::default();
        let exports = Exports::new(host.clone());
        exports.start::<Swapper>().await?;
        let to_input = host.observed().callback("event to_input")?;
        let to_compaction = host.observed().callback("event to_compaction")?;
        let compaction = exports
            .invoke_session_before_compact(
                to_input,
                fixtures::compact_data(),
                host.signal(&Flag::default()),
                host.context("/work"),
            )
            .await;
        let input = exports
            .invoke_input(
                to_compaction,
                fixtures::input("original"),
                host.context("/work"),
            )
            .await;
        release(to_input);
        release(to_compaction);
        assert_eq!((compaction.event, compaction.decision), (None, Ok(None)));
        assert_eq!((input.event, input.decision), (None, Ok(None)));
        Ok(())
    })
}
