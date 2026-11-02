//! Direct native fixture entry, without libtest arguments or stream framing.

mod native;

/// Dispatch a finite fixture request and report failures on standard error.
fn main() -> std::io::Result<()> {
    native::run()
}
