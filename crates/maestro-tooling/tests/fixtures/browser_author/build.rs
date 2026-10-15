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
    watch_config(&path, &view.mark.template)?;
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

/// Extend the static watch roots with invocation-selected identity inputs.
fn watch_config(path: &str, template: &str) -> Result<(), Box<dyn Error>> {
    println!("cargo:rerun-if-env-changed=MAESTRO_BROWSER_WATCH_CONFIG");
    if let Some(config) = std::env::var_os("MAESTRO_BROWSER_WATCH_CONFIG") {
        let root = std::env::current_dir()?;
        let roots = serde_json::to_string(&[
            root.as_path(),
            root.join("../../../../../assets/brand").as_path(),
            std::path::Path::new(path),
            std::path::Path::new(template),
        ])?;
        let config = PathBuf::from(config);
        fs::create_dir_all(config.parent().ok_or("missing watch output directory")?)?;
        fs::write(
            config,
            include_str!("Trunk.toml").replace(
                r#"watch = [".", "../../../../../assets/brand"]"#,
                &format!("watch = {roots}"),
            ),
        )?;
    }
    Ok(())
}
