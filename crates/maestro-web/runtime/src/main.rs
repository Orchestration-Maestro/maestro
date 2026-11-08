//! In-frame sandbox runtime, compiled once for the browser and embedded as a generated bundle.
//!
//! The bundle runs as the first classic script of a prepared document. Its start
//! function reads the inert configuration block, assigns the sandbox identifier
//! and every global, then installs the bridge, each runtime selection in order,
//! navigation interception and finally the compiled program. An extension world
//! without a configuration block passes the bridge role and identifier as program
//! arguments instead.
mod artifacts;
mod attachments;
mod bridge;
mod capture;
mod config;
mod console;
mod download;
mod js;
mod navigation;
mod settle;

use wasm_bindgen::{JsValue, prelude::wasm_bindgen};

/// Install the frame runtime from the document's configuration block, if it has one.
///
/// The binding integration runs this while the script that carries the bundle
/// executes, so the frame's later scripts find every helper in place.
///
/// # Errors
/// Returns the native failure of malformed configuration or an installation step.
#[wasm_bindgen(start)]
pub fn start() -> Result<(), JsValue> {
    config::install_document()
}

/// Install a message bridge for the role and identifier passed as program arguments.
///
/// # Errors
/// Returns the native failure of an unknown role or an installation step.
fn main() -> Result<(), JsValue> {
    let mut arguments = std::env::args().skip(1);
    match (arguments.next(), arguments.next()) {
        (Some(role), Some(id)) => config::install_role(&role, &id),
        _ => Ok(()),
    }
}
