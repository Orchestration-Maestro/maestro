//! Callbacks walked live in registration order, admitted once per identity or once per
//! registration.
//!
//! One collection serves every caller that keeps callbacks in the order they were
//! added: agent listeners, session listeners, extension error delivery, terminal
//! input listeners and native output-failure handlers. Removing a registration is
//! cheap and clearing every registration takes work proportional to their number.

mod listeners;

pub use listeners::{Admission, ListenerCursor, Listeners};
