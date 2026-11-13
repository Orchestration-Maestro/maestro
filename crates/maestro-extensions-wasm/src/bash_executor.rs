//! Already-produced shell-command output.
#![forbid(
    clippy::pedantic,
    clippy::too_many_arguments,
    clippy::excessive_nesting
)]
use crate::Presence;
/// Shell output with guest field presence.
pub type BashResult =
    maestro_interchange::tool::bash_executor::BashResult<Presence<f64>, Presence<String>>;
