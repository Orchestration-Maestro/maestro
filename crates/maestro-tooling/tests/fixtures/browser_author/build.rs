//! Project shared presentation into embedded author-fixture data.
use maestro_theme::{BrandMode, NativeThemeOperations, load_brand_pack};
use std::fmt::Write as _;
use std::{error::Error, fs, path::PathBuf};

/// Prepare values before compilation, retaining no borrowed presentation.
fn main() -> Result<(), Box<dyn Error>> {
    let path = std::env::var("MAESTRO_BRAND_PACK")?;
    let mode = match std::env::var("MAESTRO_BRAND_MODE").as_deref() {
        Ok("light") => BrandMode::Light,
        _ => BrandMode::Dark,
    };
    println!("cargo:rerun-if-env-changed=MAESTRO_BRAND_PACK");
    println!("cargo:rerun-if-env-changed=MAESTRO_BRAND_MODE");
    println!("cargo:rerun-if-changed={path}");
    let pack = load_brand_pack(&path, &NativeThemeOperations)?;
    let view = pack.presentation(mode)?;
    println!("cargo:rerun-if-changed={}", view.mark.template);
    let out = PathBuf::from(std::env::var_os("OUT_DIR").ok_or("missing OUT_DIR")?);
    let mut declarations = String::new();
    for (key, value) in view.css_properties() {
        write!(&mut declarations, "{key}:{value};")?;
    }
    write!(
        &mut declarations,
        "--maestro-mark-size:{}px;--maestro-mark-clear-space:{};",
        view.mark.minimum_size, view.mark.clear_space
    )?;
    fs::write(out.join("brand.css"), format!(":root{{{declarations}}}"))?;
    fs::write(
        out.join("mark.svg"),
        pack.mark_svg(mode, None, &NativeThemeOperations)?,
    )?;
    fs::write(out.join("wordmark.txt"), view.wordmark)?;
    let body = view.typography.get("body").ok_or("missing body type")?;
    fs::write(out.join("body-size.txt"), body.size.to_string())?;
    fs::write(
        out.join("body.txt"),
        &view
            .fonts
            .get(&body.font)
            .ok_or("missing body font")?
            .family,
    )?;
    for role in [
        "text",
        "background",
        "panel",
        "border",
        "accent",
        "on-accent",
    ] {
        fs::write(
            out.join(format!("{role}.txt")),
            *view.colors.get(role).ok_or("missing color role")?,
        )?;
    }
    Ok(())
}
