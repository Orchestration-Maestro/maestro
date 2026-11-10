//! Input events sent to applications running inside terminal panes.

pub mod key_mods;
pub mod keyboard;

mod function_keys;
pub mod key_encode;
mod kitty;
pub mod mouse_encode;
pub mod pointer;
