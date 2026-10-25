//! Embedded X11 colour names in file order with ASCII-only lookup.

use crate::color::Rgb;
use std::{collections::HashMap, sync::LazyLock};

/// One embedded name and its RGB value.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Entry {
    /// Name with its original spelling.
    pub name: &'static str,
    /// Colour channels.
    pub color: Rgb,
}

/// Immutable prepared name lookup.
pub struct ColorMap {
    /// ASCII-lowercased names.
    colors: HashMap<Vec<u8>, Rgb>,
    /// Shortest name in bytes.
    min: usize,
    /// Longest name in bytes.
    max: usize,
}

/// Entries parsed once from the embedded data.
static ENTRIES: LazyLock<Vec<Entry>> = LazyLock::new(|| {
    include_str!("res/rgb.txt")
        .lines()
        .filter_map(|line| {
            let r = line.get(..3)?.trim().parse().ok()?;
            let g = line.get(4..7)?.trim().parse().ok()?;
            let b = line.get(8..11)?.trim().parse().ok()?;
            let name = line.get(12..)?.trim_matches([' ', '\t']);
            Some(Entry {
                name,
                color: Rgb { r, g, b },
            })
        })
        .collect()
});

/// Prepared index, including bounds observed from the embedded names.
static MAP: LazyLock<ColorMap> = LazyLock::new(|| {
    let mut colors = HashMap::new();
    let mut min = usize::MAX;
    let mut max = 0;
    for entry in entries() {
        min = min.min(entry.name.len());
        max = max.max(entry.name.len());
        colors
            .entry(entry.name.as_bytes().to_ascii_lowercase())
            .or_insert(entry.color);
    }
    ColorMap { colors, min, max }
});

/// Return all embedded entries in file order.
#[must_use]
pub fn entries() -> &'static [Entry] {
    &ENTRIES
}

/// Return the shared immutable name index.
#[must_use]
pub fn map() -> &'static ColorMap {
    &MAP
}

impl ColorMap {
    /// Resolve a name with ASCII-only case folding and unchanged whitespace.
    #[must_use]
    pub fn get(&self, name: &[u8]) -> Option<Rgb> {
        if !(self.min..=self.max).contains(&name.len()) {
            return None;
        }
        self.colors.get(&name.to_ascii_lowercase()).copied()
    }
}
