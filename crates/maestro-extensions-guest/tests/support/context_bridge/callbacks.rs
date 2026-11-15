//! Authored callback programs driven through the native component fixture.
#![forbid(
    clippy::pedantic,
    clippy::too_many_arguments,
    clippy::excessive_nesting
)]
use super::ui;

#[path = "components.rs"]
pub(super) mod components;
#[path = "listeners.rs"]
pub(super) mod listeners;
#[path = "ordering.rs"]
pub(super) mod ordering;
#[path = "results.rs"]
pub(super) mod results;
