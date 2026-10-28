//! Build the shipped dependency-backed duration tool as an installable component.
// maestro-component: MAESTRO_WITH_DEPS_COMPONENT
#![forbid(
    clippy::pedantic,
    clippy::too_many_arguments,
    clippy::excessive_nesting
)]

/// The single authored factory shared with native author checks.
#[path = "../../../examples/extensions/with-deps/main.rs"]
pub mod authored;
