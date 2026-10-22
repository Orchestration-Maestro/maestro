//! Directory-listing format selection and the entry for one file name.

use suppaftp::list::ListParser;

/// Listing formats a server may answer in, in the order they are detected.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Format {
    /// DOS text listing.
    Dos,
    /// Unix text listing.
    Posix,
    /// Machine listing, whose entry may carry a modification stamp.
    Mlsd,
}

/// The format of a listing, read from its last content line.
pub(super) fn format(last: &str) -> Option<Format> {
    if ListParser::parse_dos(last).is_ok() {
        Some(Format::Dos)
    } else if ListParser::parse_posix(last).is_ok() {
        Some(Format::Posix)
    } else if facts(last) {
        Some(Format::Mlsd)
    } else {
        None
    }
}

/// The first listed line whose entry is named `name`.
pub(super) fn entry<'a>(lines: &'a [String], format: Format, name: &str) -> Option<&'a str> {
    lines.iter().find_map(|line| {
        let parsed = match format {
            Format::Dos => ListParser::parse_dos(line),
            Format::Posix => ListParser::parse_posix(line),
            Format::Mlsd => ListParser::parse_mlsd(line),
        };
        parsed
            .ok()
            .filter(|file| file.name() == name)
            .map(|_| line.as_str())
    })
}

/// Whether a line carries machine-listing facts, the shape only a machine listing has.
fn facts(line: &str) -> bool {
    if line.starts_with(' ') {
        return true;
    }
    line.split(' ')
        .next()
        .and_then(|packed| packed.split_once('='))
        .is_some_and(|(name, value)| !name.is_empty() && value.contains(';'))
}
