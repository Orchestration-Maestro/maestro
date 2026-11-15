//! Activates the unified dependency features for host build dependencies.

/// Reruns only when this script changes, so a checkout that carries newer modification times
/// leaves the crates built on this one as they are.
fn main() {
    println!("cargo::rerun-if-changed=build.rs");
}
