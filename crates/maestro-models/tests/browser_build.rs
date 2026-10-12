#![cfg(not(target_arch = "wasm32"))]

#[path = "support/wasm_example.rs"]
mod wasm_example;

#[test]
fn maestro_browser_environment_has_no_key() -> Result<(), Box<dyn std::error::Error>> {
    assert!(wasm_example::build()?.starts_with(b"\0asm"));
    Ok(())
}
