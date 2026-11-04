//! Recreate the package-local fixed circle image.

/// Supply the package directory and native standard output.
#[cfg(not(target_arch = "wasm32"))]
fn main() -> std::io::Result<()> {
    maestro_models::catalog_generation::generate_test_image::generate_test_image(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")),
        &mut std::io::stdout(),
    )
}

/// Developer image materialization has no browser entry effects.
#[cfg(target_arch = "wasm32")]
fn main() {}
