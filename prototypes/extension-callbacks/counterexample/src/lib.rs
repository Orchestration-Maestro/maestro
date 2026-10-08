//! A guest-exported resource cannot be passed to an import declared with the same resource.
//!
//! The world exports the resource `handler` from `guest` and the import `host.register`
//! takes an owned `handler`. The generated exported and imported `Handler` are distinct
//! Rust types, so registering the exported handler does not compile.

wit_bindgen::generate!({
    inline: "
        package maestro:counterexample;
        interface types { record event { text: string } resource context; }
        interface guest {
            use types.{event, context};
            resource handler {
                invoke: async func(event: event, ctx: borrow<context>) -> result<event, string>;
            }
        }
        interface host {
            use guest.{handler};
            register: func(name: string, callback: own<handler>) -> result<_, string>;
        }
        world author { import host; export guest; }
    ",
});

use exports::maestro::counterexample::guest::{Guest, GuestHandler, Handler};
use maestro::counterexample::host;
use maestro::counterexample::types::{Context, Event};

/// A handler that returns its event.
struct Echo;

impl GuestHandler for Echo {
    async fn invoke(&self, event: Event, _ctx: &Context) -> Result<Event, String> {
        Ok(event)
    }
}

/// The world's guest implementation.
struct Author;

impl Guest for Author {
    type Handler = Echo;
}

/// Registers an exported handler through the import; this is the call that fails to compile.
pub fn register_exported_handler() -> Result<(), String> {
    host::register("echo", Handler::new(Echo))
}

export!(Author);
