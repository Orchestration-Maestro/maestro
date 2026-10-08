//! Shared event channels with callback-scoped extension delivery.
//!
//! Supply a serial cooperative spawner that never polls inline or concurrently
//! with the emitting synchronous segment. A suspension must let tails progress.
//!
//! ```
//! use maestro_extensions::create_event_bus;
//! use serde_json::json;
//! use std::sync::{Arc, Mutex};
//!
//! # fn main() -> Result<(), Box<dyn std::error::Error>> {
//! let runtime = tokio::runtime::Builder::new_current_thread().build()?;
//! runtime.block_on(async {
//!     let controller = create_event_bus(
//!         Arc::new(|tail| { tokio::spawn(tail); }),
//!         Arc::new(|diagnostic| eprint!("{diagnostic}")),
//!     );
//!     let events = controller.for_extension();
//!     let received = Arc::new(Mutex::new(Vec::new()));
//!     let output = Arc::clone(&received);
//!     let subscription = events.on("saved", Arc::new(move |data, _| {
//!         output.lock().map_err(|error| error.to_string())?.push(data);
//!         Ok(None)
//!     }));
//!     events.emit("saved", json!({"path": "notes.md"})).await;
//!     subscription.unsubscribe();
//!     events.emit("saved", json!({"path": "ignored.md"})).await;
//!     assert_eq!(received.lock().unwrap().as_slice(), &[Arc::new(json!({"path": "notes.md"}))]);
//! });
//! # Ok(())
//! # }
//! ```

/// Invocation-local callback emission.
pub mod callbacks;
/// Ordered shared channel delivery.
pub mod event_bus;
pub use callbacks::CallbackEmitter;
pub use event_bus::{
    ErrorReporter, EventBus, EventBusController, EventError, EventListener, EventTail,
    Subscription, TailSpawner, create_event_bus,
};
