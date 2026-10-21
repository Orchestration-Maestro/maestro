//! Project shared presentation into embedded author-fixture data.
#[cfg(not(target_arch = "wasm32"))]
use maestro_theme::{BrandMode, load_brand_pack};
#[cfg(not(target_arch = "wasm32"))]
use std::fmt::Write as _;
#[cfg(not(target_arch = "wasm32"))]
use std::{error::Error, fs, path::PathBuf};

/// Prepare values before compilation, retaining no borrowed presentation.
#[cfg(not(target_arch = "wasm32"))]
fn main() -> Result<(), Box<dyn Error>> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let [path, mode, output] = args.as_slice() else {
        return Err("usage: browser_author_inputs <pack> <mode> <output-directory>".into());
    };
    let mode = if mode == "light" {
        BrandMode::Light
    } else {
        BrandMode::Dark
    };
    let pack = load_brand_pack(path)?;
    let view = pack.presentation(mode)?;
    let out = PathBuf::from(output);
    fs::create_dir_all(&out)?;
    watch_directories(path, &view.mark.template, &out)?;
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
    fs::write(out.join("mark.svg"), pack.mark_svg(mode, None)?)?;
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

/// Observe authored paths and canonical targets without retaining borrowed presentation.
#[cfg(not(target_arch = "wasm32"))]
fn watch_directories(
    path: &str,
    template: &str,
    out: &std::path::Path,
) -> Result<(), Box<dyn Error>> {
    let mut roots = Vec::new();
    for file in [
        PathBuf::from(path),
        PathBuf::from(template),
        fs::canonicalize(path)?,
        fs::canonicalize(template)?,
    ] {
        roots.push(
            file.parent()
                .ok_or("missing identity directory")?
                .to_path_buf(),
        );
    }
    fs::write(
        out.join("watch-dirs.json"),
        serde_json::to_vec_pretty(&roots)?,
    )?;
    Ok(())
}

/// Native author preparation is unavailable in browser builds.
#[cfg(target_arch = "wasm32")]
fn main() {}
