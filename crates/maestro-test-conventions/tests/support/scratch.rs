use maestro_test_process::compiler;

/// A fresh temporary directory, removed when dropped.
pub struct Scratch {
    pub root: maestro_test_directory::Scratch,
}

impl Scratch {
    pub fn new() -> Self {
        let root = maestro_test_directory::Scratch::new().expect("create fixture");
        compiler::declare_toolchain(root.path(), include_str!("../../../../rust-toolchain.toml"))
            .unwrap();
        // Cargo keeps its compiler probes in an existing target directory and otherwise
        // re-runs the compiler on every `cargo metadata` read of the fixture.
        std::fs::create_dir(root.path().join("target")).unwrap();
        Self { root }
    }
}
