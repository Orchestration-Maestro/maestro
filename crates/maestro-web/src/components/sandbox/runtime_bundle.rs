//! The generated in-frame runtime: one self-starting classic script.

/// Script text built by the repository tooling from the runtime package.
///
/// It runs as the first classic script of a prepared document. A frame's content
/// security policy admits it by digest, and `'wasm-unsafe-eval'` lets it compile
/// its embedded module.
pub const BUNDLE: &str = include_str!("../../../assets/maestro-sandbox-runtime.js");
