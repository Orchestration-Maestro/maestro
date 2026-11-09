//! Give the runtime binary its Emscripten link settings, so any Cargo build for the target
//! links it: the plain build as well as the repository tooling's generated bundle.

/// Linker settings that make the script start itself without help from the page.
///
/// The allocator is dlmalloc. The binding integration runs its start function before the
/// static constructors, so an allocator that initialises itself in a constructor would be
/// used before it is ready. This one initialises on first use.
const LINK_SETTINGS: [&str; 8] = [
    "-sWASM_BINDGEN",
    "-sSINGLE_FILE=1",
    "-sSINGLE_FILE_BINARY_ENCODE=0",
    "-sWASM_ASYNC_COMPILATION=0",
    "-sDYNAMIC_EXECUTION=0",
    "-sENVIRONMENT=web",
    "-sMALLOC=dlmalloc",
    "-Wno-experimental",
];

/// Pass the link settings to the binary when Cargo builds it for Emscripten.
fn main() {
    println!("cargo::rerun-if-changed=build.rs");
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("emscripten") {
        for setting in LINK_SETTINGS {
            println!("cargo::rustc-link-arg-bins={setting}");
        }
    }
}
