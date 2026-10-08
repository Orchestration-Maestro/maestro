//! Support shared by the author test targets: the controlled adapter, typed fixtures and
//! small helpers to drive futures without a clock.
#![allow(dead_code, reason = "each test target uses part of the shared support")]

#[path = "../../src/tests/controlled.rs"]
pub mod controlled;
#[macro_use]
#[path = "../../src/tests/fixtures.rs"]
mod fixtures;
#[macro_use]
#[path = "../../src/tests/event_fixtures.rs"]
mod event_fixtures;
#[path = "../../src/tests/author.rs"]
pub mod author;
#[path = "../../src/tests/compaction_example.rs"]
pub mod compaction_example;

use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll, Waker};

/// Typed values of the generated records.
pub mod guest_family {
    pub use maestro_extensions_wasm::bindings::maestro::extension::{events, models, session};

    define_fixtures!();
    define_event_fixtures!();
}

/// Runs a future to completion on a current-thread runtime.
pub fn block_on<T>(future: impl Future<Output = T>) -> T {
    match tokio::runtime::Builder::new_current_thread().build() {
        Ok(runtime) => runtime.block_on(future),
        Err(error) => panic!("cannot build a runtime: {error}"),
    }
}

/// Polls a future once without a waker that does anything.
pub fn poll_once<F: Future>(future: &mut Pin<Box<F>>) -> Poll<F::Output> {
    future
        .as_mut()
        .poll(&mut Context::from_waker(Waker::noop()))
}
