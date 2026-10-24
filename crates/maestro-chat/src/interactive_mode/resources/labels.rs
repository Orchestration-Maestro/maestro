//! Compact labels of listed resources.

use super::{HomeLookup, ResourceItem, get_short_path};
use std::error::Error;

/// Labels a resource by the last segment of its short path, ignoring empty and `~`
/// segments and treating backslashes as separators, or by the short path itself when
/// no segment remains.
///
/// # Errors
/// Returns the failure of the short path's resolution or home lookup.
pub(super) fn get_compact_path_label(
    item: &ResourceItem<'_>,
    home: &HomeLookup<'_>,
) -> Result<String, Box<dyn Error>> {
    let short = get_short_path(item.path, item.source_info, home)?;
    let label = short
        .replace('\\', "/")
        .split('/')
        .rfind(|segment| !segment.is_empty() && *segment != "~")
        .map(str::to_owned);
    Ok(label.unwrap_or(short))
}
