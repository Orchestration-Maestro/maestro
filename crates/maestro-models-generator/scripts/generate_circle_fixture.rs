//! Recreate the package-local fixed circle image.

/// Supply the package directory and native standard output.
fn main() -> std::io::Result<()> {
    maestro_models_generator::generate_test_image::generate_test_image(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")),
        &mut std::io::stdout(),
    )
}
