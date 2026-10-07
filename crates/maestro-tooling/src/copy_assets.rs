use std::fs;
use std::io;
use std::path::Path;

/// Destination layout for prepared assets.
#[derive(Clone, Copy)]
pub enum Layout<'a> {
    /// Application library assets.
    Library,
    /// Standalone assets with supplied metadata and viewer inputs.
    Standalone {
        /// Composition-root package manifest.
        metadata: &'a Path,
        /// Prepared runnable viewer directory.
        viewer: &'a Path,
    },
}

/// Copy assets in group order, preserving completed groups on error.
///
/// # Errors
/// Returns missing-input and native filesystem failures.
pub fn copy(source: &Path, destination: &Path, layout: Layout<'_>) -> io::Result<()> {
    match layout {
        Layout::Library => copy_library(source, destination),
        Layout::Standalone { metadata, viewer } => {
            copy_standalone(source, destination, metadata, viewer)
        }
    }
}

fn copy_library(source: &Path, destination: &Path) -> io::Result<()> {
    copy_matches(
        &source.join("src/modes/interactive/theme"),
        &destination.join("modes/interactive/theme"),
        "json",
    )?;
    copy_matches(
        &source.join("src/modes/interactive/assets"),
        &destination.join("modes/interactive/assets"),
        "png",
    )?;
    let export = destination.join("core/export-html");
    fs::create_dir_all(&export)?;
    for name in ["template.html", "template.css", "template.js"] {
        fs::copy(
            source.join("src/core/export-html").join(name),
            export.join(name),
        )?;
    }
    copy_matches(
        &source.join("src/core/export-html/vendor"),
        &export.join("vendor"),
        "js",
    )
}

fn copy_standalone(
    source: &Path,
    destination: &Path,
    metadata: &Path,
    viewer: &Path,
) -> io::Result<()> {
    fs::create_dir_all(destination)?;
    fs::copy(metadata, destination.join("Cargo.toml"))?;
    for name in ["README.md", "CHANGELOG.md"] {
        fs::copy(source.join(name), destination.join(name))?;
    }
    copy_matches(
        &source.join("src/modes/interactive/theme"),
        &destination.join("theme"),
        "json",
    )?;
    copy_matches(
        &source.join("src/modes/interactive/assets"),
        &destination.join("assets"),
        "png",
    )?;
    if !viewer.is_dir() || fs::read_dir(viewer)?.next().is_none() {
        return Err(io::Error::other(format!(
            "viewer parameter must name a nonempty directory: {}",
            viewer.display()
        )));
    }
    copy_tree(viewer, &destination.join("export-html"))?;
    for name in ["docs", "examples"] {
        copy_tree(&source.join(name), &destination.join(name))?;
    }
    Ok(())
}

fn copy_matches(source: &Path, destination: &Path, suffix: &str) -> io::Result<()> {
    fs::create_dir_all(destination)?;
    let mut matches = fs::read_dir(source)?
        .map(|entry| entry.map(|entry| entry.path()))
        .collect::<io::Result<Vec<_>>>()?;
    matches.retain(|path| {
        path.file_name()
            .is_some_and(|name| !name.as_encoded_bytes().starts_with(b"."))
            && path
                .extension()
                .is_some_and(|extension| extension == suffix)
    });
    matches.sort();
    if matches.is_empty() {
        return Err(io::Error::new(
            io::ErrorKind::NotFound,
            format!("no .{suffix} assets in {}", source.display()),
        ));
    }
    for path in matches {
        let name = path
            .file_name()
            .ok_or_else(|| io::Error::other("missing asset filename"))?;
        fs::copy(&path, destination.join(name))?;
    }
    Ok(())
}

fn copy_tree(source: &Path, destination: &Path) -> io::Result<()> {
    let metadata = fs::symlink_metadata(source)?;
    if metadata.is_symlink() {
        return copy_link(source, destination);
    }
    if !metadata.is_dir() {
        fs::copy(source, destination)?;
        return Ok(());
    }
    fs::create_dir_all(destination)?;
    let mut entries = fs::read_dir(source)?.collect::<io::Result<Vec<_>>>()?;
    entries.sort_by_key(std::fs::DirEntry::file_name);
    for entry in entries {
        copy_tree(&entry.path(), &destination.join(entry.file_name()))?;
    }
    Ok(())
}

fn copy_link(source: &Path, destination: &Path) -> io::Result<()> {
    if let Ok(metadata) = fs::symlink_metadata(destination) {
        if metadata.is_dir() {
            return Err(io::Error::other(format!(
                "cannot replace directory {}",
                destination.display()
            )));
        }
        fs::remove_file(destination)?;
    }
    let target = fs::read_link(source)?;
    #[cfg(unix)]
    {
        std::os::unix::fs::symlink(target, destination)
    }
    #[cfg(windows)]
    {
        if source.is_dir() {
            std::os::windows::fs::symlink_dir(target, destination)
        } else {
            std::os::windows::fs::symlink_file(target, destination)
        }
    }
    #[cfg(not(any(unix, windows)))]
    {
        let _ = target;
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "symbolic links are unsupported",
        ))
    }
}
