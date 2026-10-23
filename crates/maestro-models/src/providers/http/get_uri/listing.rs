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
    lines
        .iter()
        .map(String::as_str)
        .find(|line| named(format, line, name))
}

/// The modification stamp of a machine-listing line, when it carries one.
pub(super) fn modified(line: &str) -> Option<&str> {
    machine(line)?.modified
}

/// Whether this line's entry is named `name`.
fn named(format: Format, line: &str, name: &str) -> bool {
    match format {
        Format::Dos => ListParser::parse_dos(line).is_ok_and(|file| file.name() == name),
        Format::Posix => ListParser::parse_posix(line).is_ok_and(|file| file.name() == name),
        Format::Mlsd => machine(line).is_some_and(|entry| entry.name == name),
    }
}

/// One machine-listing entry.
struct Entry<'a> {
    /// The name the server spelled, after the facts.
    name: &'a str,
    /// The `modify` fact, when the entry carries one.
    modified: Option<&'a str>,
}

/// Parse a machine-listing line, keeping the entries and names the reference parser keeps.
///
/// The facts end at the first space and the name is everything after it, so a name may contain
/// semicolons and may start with a space. Facts this port does not use are still accepted, and an
/// entry whose facts it does not understand keeps its identity instead of vanishing from the
/// listing.
fn machine(line: &str) -> Option<Entry<'_>> {
    let (packed, name) = line.split_once(' ').unwrap_or((line, ""));
    // The reference drops these names and every entry pointing outside the listed directory.
    if name.is_empty() || name == "." || name == ".." || name.contains('/') {
        return None;
    }
    let modified = packed
        .split(';')
        .filter_map(|fact| fact.split_once('='))
        .find(|(fact, _)| fact.eq_ignore_ascii_case("modify"))
        .map(|(_, stamp)| stamp);
    Some(Entry { name, modified })
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
