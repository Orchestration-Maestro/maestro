//! Generate native catalog data through the default developer transport.

/// Supply native transport, output streams and the manifest-relative destination.
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()?;
    let destination =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../maestro-models-catalog/src");
    runtime.block_on(maestro_models_generator::generate_models::generate_models(
        &maestro_http::default_fetch(),
        &destination,
        &mut std::io::stdout(),
        &mut std::io::stderr(),
    ))?;
    Ok(())
}
