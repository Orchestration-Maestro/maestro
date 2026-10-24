//! Identity-deduplicated callbacks walked live in registration order.
//!
//! One collection serves every caller that keeps callbacks in the order they were
//! added: agent listeners, session listeners, extension error delivery and terminal
//! input listeners. Removing a registration is cheap and clearing every registration
//! takes work proportional to their number.

mod listeners;

pub use listeners::{ListenerCursor, Listeners};
