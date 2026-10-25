//! Minimal browser input for cold build and prepared-include observations.

/// Keep the prepared input observable in the final optimized module.
#[wasm_bindgen::prelude::wasm_bindgen]
pub fn wordmark() -> String {
    include_str!(concat!(env!("MAESTRO_BROWSER_INPUTS"), "/wordmark.txt")).to_owned()
}

/// Start without framework dependencies unrelated to build preparation.
fn main() {}
