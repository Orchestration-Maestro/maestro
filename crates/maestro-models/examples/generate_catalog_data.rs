//! Generate native catalog data through the default developer transport.

/// Supply native transport, output streams and the manifest-relative destination.
#[cfg(not(target_arch = "wasm32"))]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()?;
    let destination =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/catalog/models_generated");
    runtime.block_on(
        maestro_models::catalog_generation::generate_models::generate_models(
            &maestro_models::default_fetch(),
            &destination,
            &mut std::io::stdout(),
            &mut std::io::stderr(),
        ),
    )?;
    Ok(())
}
/// Developer generation has no browser entry effects.
#[cfg(target_arch = "wasm32")]
fn main() {}
