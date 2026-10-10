//! Language labels derived from file paths.

/// Tab-separated lowercase suffix and language label, one pair per line.
const PATH_LANGUAGES: &str = include_str!("../../assets/path-languages.tsv");

/// The language label for the text after the last `.` of `file_path`, compared in lowercase.
///
/// A path without a dot is compared whole, so only a bare `Dockerfile` or `Makefile`
/// matches a name; `Path::extension`, base names and trimming play no part.
#[must_use]
pub fn get_language_from_path(file_path: &str) -> Option<&'static str> {
    let suffix = file_path.rsplit('.').next()?.to_lowercase();
    PATH_LANGUAGES
        .lines()
        .filter_map(|line| line.split_once('\t'))
        .find_map(|(known, language)| (known == suffix).then_some(language))
}
