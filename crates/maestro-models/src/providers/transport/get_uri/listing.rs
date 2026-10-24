//! Directory-listing format selection and the entry for one file name.

mod parse_list_dos;
mod parse_list_unix;

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

/// The content lines of a listing, without blank lines and without a `total` summary line.
pub(super) fn lines(text: &str) -> Vec<String> {
    text.lines()
        .filter(|line| !line.chars().all(maestro_text::is_whitespace_scalar))
        .filter(|line| !line.starts_with("total"))
        .map(str::to_owned)
        .collect()
}

/// The format of a listing, read from its last content line.
pub(super) fn format(last: &str) -> Option<Format> {
    if parse_list_dos::test_line(last) {
        Some(Format::Dos)
    } else if parse_list_unix::test_line(last) {
        Some(Format::Posix)
    } else if machine_listing(last) {
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
        Format::Dos => parse_list_dos::name(line) == Some(name),
        Format::Posix => parse_list_unix::name(line) == Some(name),
        Format::Mlsd => machine(line).is_some_and(|entry| entry.name == name),
    }
}

/// One machine-listing entry, reduced to what a source observation reads.
struct Machine<'a> {
    /// The name the server spelled, after the facts.
    name: &'a str,
    /// The last nonempty `modify` fact, when the entry carries one.
    modified: Option<&'a str>,
}

/// Whether a line has the shape only a machine listing has.
fn machine_listing(line: &str) -> bool {
    if line.starts_with(' ') {
        return true;
    }
    let head = line
        .split(maestro_text::is_whitespace_scalar)
        .next()
        .unwrap_or(line);
    head.split_once('=')
        .is_some_and(|(name, value)| !name.is_empty() && value.contains(';'))
}

/// Parse a machine-listing line, keeping the entries and names the reference parser keeps.
///
/// The facts end at the first space and the name is everything after it, so a name may contain
/// semicolons and may start with a space. A current- or parent-directory entry is refused at its
/// type fact, and the last nonempty modification fact is the one the entry reports.
fn machine(line: &str) -> Option<Machine<'_>> {
    let (packed, name) = line.split_once(' ').unwrap_or(("", line));
    if name.is_empty() || name == "." || name == ".." || name.contains('/') {
        return None;
    }
    let mut modified = None;
    for fact in packed.split(';') {
        let (fact_name, value) = fact.split_once('=').unwrap_or((fact, ""));
        if value.is_empty() {
            continue;
        }
        match fact_name.to_ascii_lowercase().as_str() {
            "type" if matches!(value, "cdir" | "pdir") => return None,
            "modify" => modified = Some(value),
            _ => {}
        }
    }
    Some(Machine { name, modified })
}
