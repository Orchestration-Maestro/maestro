//! Regenerate the shipped dark and light theme documents from a brand pack.
#[cfg(not(target_arch = "wasm32"))]
use maestro_theme::{BrandMode, load_brand_pack};
#[cfg(not(target_arch = "wasm32"))]
use std::io::{Error, ErrorKind};

#[cfg(not(target_arch = "wasm32"))]
fn main() -> std::io::Result<()> {
    let arguments: Vec<String> = std::env::args().skip(1).collect();
    let [pack, directory] = arguments.as_slice() else {
        return Err(Error::new(
            ErrorKind::InvalidInput,
            "usage: brand_themes <pack.json> <output-directory>",
        ));
    };
    let pack = load_brand_pack(pack).map_err(Error::other)?;
    for (mode, file) in [
        (BrandMode::Dark, "dark.json"),
        (BrandMode::Light, "light.json"),
    ] {
        let document = pack.theme_json(mode).map_err(Error::other)?;
        std::fs::write(std::path::Path::new(directory).join(file), document)?;
    }
    Ok(())
}

/// Native author preparation is unavailable in browser builds.
#[cfg(target_arch = "wasm32")]
fn main() {}
