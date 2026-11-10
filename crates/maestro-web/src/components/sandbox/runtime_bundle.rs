//! The generated in-frame runtime: one self-starting classic script.

/// Script text built by the repository tooling from the runtime package.
///
/// The build script locates the file that `just sandbox-runtime` writes under `target/`.
///
/// It runs as the first classic script of a prepared document. A frame's content
/// security policy admits it by digest, and `'wasm-unsafe-eval'` lets it compile
/// its embedded module.
pub const BUNDLE: &str = include_str!(env!("MAESTRO_SANDBOX_RUNTIME"));

#[cfg(test)]
mod tests {
    use super::BUNDLE;

    /// Whether a byte can belong to a recorded source path.
    fn path_byte(byte: u8) -> bool {
        byte.is_ascii_alphanumeric() || b"_./-+@".contains(&byte)
    }

    /// The embedded module records Rust source paths only under machine-independent names:
    /// the toolchain's `/rustc/` prefix, the remapped `/checkout/`, or a relative path such as
    /// a dependency's `<name>-<version>/`, never a directory of the machine that built it.
    #[test]
    fn the_bundle_records_no_build_machine_path() {
        let (_, encoded) = BUNDLE.split_once("base64Decode(\"").unwrap();
        let (encoded, _) = encoded.split_once('"').unwrap();
        let module = crate::utils::base64::decode(encoded).unwrap();
        let machine: Vec<String> = module
            .split(|byte| !path_byte(*byte))
            .filter(|run| run.starts_with(b"/") && run.ends_with(b".rs"))
            .map(|run| String::from_utf8_lossy(run).into_owned())
            .filter(|path| !path.starts_with("/rustc/") && !path.starts_with("/checkout/"))
            .collect();
        assert!(machine.is_empty(), "{machine:?}");
    }
}
