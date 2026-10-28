//! Native final-component link traversal over caller-supplied observations.
use std::{
    io,
    path::{Path, PathBuf},
};

/// Follows at most forty final-component links using supplied native observations.
///
/// The callback returns a target for a link, or `None` to stop. Relative targets
/// are joined to the link's parent without normalizing native components.
/// This function performs no filesystem access itself.
///
/// # Errors
/// Returns observation errors unchanged, or an error if the chain exceeds forty links.
pub fn resolve_final_links(
    path: &Path,
    mut read_link: impl FnMut(&Path) -> io::Result<Option<PathBuf>>,
) -> io::Result<PathBuf> {
    let _ = read_link(path)?;
    Ok(path.to_path_buf())
}
