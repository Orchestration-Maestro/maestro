use std::fs;
use std::io;
use std::path::Path;

/// Destination layout for prepared assets.
#[derive(Clone, Copy)]
pub enum Layout<'a> {
    /// Application library assets.
    Library,
    /// Standalone assets with supplied metadata, viewer and package inputs.
    Standalone {
        /// Composition-root package manifest.
        metadata: &'a Path,
        /// Prepared runnable viewer directory.
        viewer: &'a Path,
        /// Package directory holding the README, history, documentation and examples.
        package: &'a Path,
    },
}

/// Copy assets in group order, preserving completed groups on error.
///
/// # Errors
/// Returns filesystem failures. A failure reading an input names that input's
/// path; a destination failure keeps its native error, and groups copied before
/// the failure remain on disk.
pub fn copy(source: &Path, destination: &Path, layout: Layout<'_>) -> io::Result<()> {
    match layout {
        Layout::Library => copy_library(source, destination),
        Layout::Standalone {
            metadata,
            viewer,
            package,
        } => copy_standalone(source, destination, metadata, viewer, package),
    }
}

/// Copy the library theme, interactive images and prepared export assets.
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
        copy_file(
            &source.join("src/core/export-html").join(name),
            &export.join(name),
        )?;
    }
    copy_matches(
        &source.join("src/core/export-html/vendor"),
        &export.join("vendor"),
        "js",
    )
}

/// Copy standalone metadata, themes, viewer assets, documentation and examples.
fn copy_standalone(
    source: &Path,
    destination: &Path,
    metadata: &Path,
    viewer: &Path,
    package: &Path,
) -> io::Result<()> {
    fs::create_dir_all(destination)?;
    copy_file(metadata, &destination.join("Cargo.toml"))?;
    for name in ["README.md", "CHANGELOG.md"] {
        copy_file(&package.join(name), &destination.join(name))?;
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
    let populated = viewer.is_dir()
        && fs::read_dir(viewer)
            .map_err(|error| input_error(viewer, &error))?
            .next()
            .is_some();
    if !populated {
        return Err(io::Error::other(format!(
            "viewer parameter must name a nonempty directory: {}",
            viewer.display()
        )));
    }
    copy_tree(viewer, &destination.join("export-html"))?;
    for name in ["docs", "examples"] {
        copy_tree(&package.join(name), &destination.join(name))?;
    }
    Ok(())
}

/// Copy visible files matching an extension in stable filename order.
fn copy_matches(source: &Path, destination: &Path, suffix: &str) -> io::Result<()> {
    fs::create_dir_all(destination)?;
    let mut matches = fs::read_dir(source)
        .map_err(|error| input_error(source, &error))?
        .map(|entry| entry.map(|entry| entry.path()))
        .collect::<io::Result<Vec<_>>>()
        .map_err(|error| input_error(source, &error))?;
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
        copy_file(&path, &destination.join(name))?;
    }
    Ok(())
}

/// Copy an input tree while preserving symbolic links, naming the entry that failed.
fn copy_tree(source: &Path, destination: &Path) -> io::Result<()> {
    let metadata = fs::symlink_metadata(source).map_err(|error| input_error(source, &error))?;
    if metadata.is_symlink() {
        return copy_link(source, destination);
    }
    if !metadata.is_dir() {
        return copy_file(source, destination);
    }
    fs::create_dir_all(destination)?;
    let mut entries = fs::read_dir(source)
        .map_err(|error| input_error(source, &error))?
        .map(|entry| entry.map_err(|error| input_error(source, &error)))
        .collect::<io::Result<Vec<_>>>()?;
    entries.sort_by_key(std::fs::DirEntry::file_name);
    for entry in entries {
        copy_tree(&entry.path(), &destination.join(entry.file_name()))?;
    }
    Ok(())
}

/// Name an input path in a filesystem failure, keeping the operating system's error kind.
fn input_error(source: &Path, error: &io::Error) -> io::Error {
    io::Error::new(
        error.kind(),
        format!("cannot copy {}: {error}", source.display()),
    )
}

/// Copy one input file, naming its source when it cannot be opened.
///
/// Opening first separates the two sides: `fs::copy` reports one error for either.
fn copy_file(source: &Path, destination: &Path) -> io::Result<()> {
    fs::File::open(source).map_err(|error| input_error(source, &error))?;
    fs::copy(source, destination)?;
    Ok(())
}

/// Replace a destination file with the source's symbolic link.
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
    let target = fs::read_link(source).map_err(|error| input_error(source, &error))?;
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
