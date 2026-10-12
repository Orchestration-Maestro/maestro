//! Native child-process lifecycles shared by tools, packages and the toolkit.

#[cfg(not(target_arch = "wasm32"))]
pub mod child_process;
