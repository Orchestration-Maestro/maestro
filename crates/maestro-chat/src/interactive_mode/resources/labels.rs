//! Compact labels and expanded paths of listed resources.

use super::{HomeLookup, ResourceItem, format_display_path, get_short_path, is_package_source};
use maestro_app::parse_git_url;
use std::{collections::HashMap, error::Error, hash::Hash, io};

/// Displays an extension path with the home prefix abbreviated and a final
/// `/index.ts`, then a final `/index.js`, removed.
///
/// # Errors
/// Returns the failure of the home lookup.
pub(super) fn format_extension_display_path(
    path: &str,
    home: &HomeLookup<'_>,
) -> io::Result<String> {
    let display = format_display_path(path, home)?;
    let display = display.strip_suffix("/index.ts").unwrap_or(&display);
    Ok(display
        .strip_suffix("/index.js")
        .unwrap_or(display)
        .to_owned())
}

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

/// Labels a package source: an npm source without its `npm:` prefix, a Git source by
/// its repository path, and any other source, or an empty result, as written.
pub(super) fn get_compact_package_source_label(source: &str) -> String {
    if let Some(name) = source.strip_prefix("npm:") {
        return if name.is_empty() { source } else { name }.to_owned();
    }
    match parse_git_url(source) {
        Some(git) if !git.path.is_empty() => git.path,
        _ => source.to_owned(),
    }
}

/// Labels a package extension by its package, adding the folder of an index file or the
/// path of any other file below the package's `extensions` folder.
///
/// # Errors
/// Returns the failure of the short path's resolution or home lookup.
fn get_compact_extension_label(
    item: &ResourceItem<'_>,
    home: &HomeLookup<'_>,
) -> Result<String, Box<dyn Error>> {
    let source = item.source_info.map_or("", |info| info.source.as_str());
    let source_label = get_compact_package_source_label(source);
    let short = get_short_path(item.path, item.source_info, home)?.replace('\\', "/");
    let package_path = short.strip_prefix("extensions/").unwrap_or(&short);
    let parsed = maestro_path::posix::parse(package_path);
    Ok(match parsed.dir {
        _ if parsed.name != "index" => format!("{source_label}:{package_path}"),
        "" | "." => source_label,
        dir => format!("{source_label}:{dir}"),
    })
}

/// Labels each extension: a package extension by its package, and any other by the
/// shortest suffix of its displayed path that no other non-package extension shares.
///
/// # Errors
/// Returns the failure of a home lookup or a short path's resolution.
pub(super) fn get_compact_extension_labels(
    items: &[ResourceItem<'_>],
    home: &HomeLookup<'_>,
) -> Result<Vec<String>, Box<dyn Error>> {
    let displayed = items
        .iter()
        .map(|item| Ok(format_display_path(item.path, home)?.replace('\\', "/")))
        .collect::<io::Result<Vec<_>>>()?;
    let locals: Vec<_> = items
        .iter()
        .zip(&displayed)
        .filter(|(item, _)| !is_package_source(item.source_info))
        .map(|(_, path)| local_segments(path))
        .collect();
    let mut suffixes = get_distinct_suffix_labels(&locals).into_iter();
    let mut segments = locals.iter();
    items
        .iter()
        .map(|item| {
            if is_package_source(item.source_info) {
                return get_compact_extension_label(item, home);
            }
            let label = suffixes.next().unwrap_or_default();
            if segments.next().is_some_and(|segments| !segments.is_empty()) {
                return Ok(label);
            }
            let item = ResourceItem {
                path: item.path,
                source_info: None,
            };
            get_compact_path_label(&item, home)
        })
        .collect()
}

/// The nonempty segments of a displayed path other than `~`, without a final
/// `index.ts` or `index.js` that has a parent.
fn local_segments(path: &str) -> Vec<&str> {
    let mut segments: Vec<_> = path
        .split('/')
        .filter(|segment| !segment.is_empty() && *segment != "~")
        .collect();
    if segments.len() > 1 && matches!(segments.last(), Some(&("index.ts" | "index.js"))) {
        segments.pop();
    }
    segments
}

/// Labels each path by its shortest suffix of segments that no other path shares, or
/// by all its segments when another path shares every suffix.
///
/// One index of every suffix counts the paths ending with it, so each segment is
/// hashed once while indexing and each label reads only its own suffix counts.
pub(super) fn get_distinct_suffix_labels<S: Hash + Eq + AsRef<str>>(
    paths: &[Vec<S>],
) -> Vec<String> {
    let mut nodes = HashMap::new();
    let mut counts = vec![0_usize];
    let chains: Vec<Vec<usize>> = paths
        .iter()
        .map(|segments| {
            let mut node = 0;
            let chain = segments.iter().rev().map(|segment| {
                let next = counts.len();
                node = *nodes.entry((node, segment)).or_insert(next);
                if node == next {
                    counts.push(0);
                }
                counts[node] += 1;
                node
            });
            chain.collect()
        })
        .collect();
    paths
        .iter()
        .zip(&chains)
        .map(|(segments, chain)| {
            let length = chain
                .iter()
                .position(|&node| counts[node] == 1)
                .map_or(segments.len(), |position| position + 1);
            let suffix: Vec<_> = segments[segments.len() - length..]
                .iter()
                .map(AsRef::as_ref)
                .collect();
            suffix.join("/")
        })
        .collect()
}
