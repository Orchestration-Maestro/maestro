#[allow(dead_code, reason = "Scratch uses only the shared declaration writer.")]
#[path = "../../../maestro-tooling/tests/fixtures/toolchain_support.rs"]
mod toolchain_support;

use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

/// A fresh temporary directory, removed when dropped.
pub struct Scratch {
    pub root: PathBuf,
}

impl Scratch {
    pub fn new() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        loop {
            let root = std::env::temp_dir().join(format!(
                "maestro-conventions-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            match fs::create_dir(&root) {
                Ok(()) => {
                    toolchain_support::declare_toolchain(
                        &root,
                        include_str!("../../../../rust-toolchain.toml"),
                    )
                    .unwrap();
                    return Self { root };
                }
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {}
                Err(error) => panic!("create fixture: {error}"),
            }
        }
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let result = fs::remove_dir_all(&self.root);
        if !std::thread::panicking() {
            result.expect("remove fixture directory");
        }
    }
}
