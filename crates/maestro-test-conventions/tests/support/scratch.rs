#[allow(dead_code, reason = "Scratch uses only the shared declaration writer.")]
#[path = "../../../maestro-tooling/tests/fixtures/toolchain_support.rs"]
mod toolchain_support;

/// A fresh temporary directory, removed when dropped.
pub struct Scratch {
    pub root: maestro_test_directory::Scratch,
}

impl Scratch {
    pub fn new() -> Self {
        let root = maestro_test_directory::Scratch::new().expect("create fixture");
        toolchain_support::declare_toolchain(
            root.path(),
            include_str!("../../../../rust-toolchain.toml"),
        )
        .unwrap();
        Self { root }
    }
}
