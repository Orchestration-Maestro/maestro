//! A recognizer source that opens with `^` compiles through the start-anchored owner.
use std::{fs, path::Path};

/// Source openers that start with `^` yet stay on a plain `Regex`, each with the reason the
/// owner does not apply. An entry is the literal's first characters.
const PLAIN_REGEX: &[(&str, &str)] = &[
    // Reference spellings of the text rule; `stepwise` compiles them through the owner.
    (
        "^([`~]+|[^`~])(?:",
        "inline text spelling compiled by the owner",
    ),
    (
        "^(`+|[^`])(?:",
        "inline text spelling compiled by the owner",
    ),
    // Block quote: the container engine takes the plain regex type.
    (
        "^( {0,3}> ?(",
        "block quote source consumed as a plain regex",
    ),
    // Shared source text that the anchored link-tag rule compiles through the owner.
    (
        "^<!--(?:-?>",
        "inline tag source compiled by the anchored tag rule",
    ),
    // Only the first alternative has `^`; the caller walks every later match.
    (
        "^[^_*]*?__[^_*]*?",
        "right-delimiter runs scanned with find_iter",
    ),
    (
        "^[^_*]*?\\*\\*[^_*]*?",
        "right-delimiter runs scanned with find_iter",
    ),
    // The haystack is one scalar.
    (
        "^((?![*_])[\\s\\p{P}\\p{S}])",
        "single-scalar punctuation test",
    ),
    // Item stops are matched against one bounded line.
    ("^ {{0,{prefix}}}", "item stops matched on one line"),
    // Retained HTML block boundaries are matched against one physical line.
    (
        "^ {0,3}(?:<(script|pre|style|textarea)",
        "html opener matched on one line",
    ),
    (
        "^ {0,3}</(?!script|pre|style|textarea)",
        "html closing matched on one line",
    ),
];

/// Body of every string literal in `text` that is the first literal of its statement,
/// paired with the text of the statement before it.
fn caret_literals(text: &str) -> Vec<(&str, &str)> {
    let bytes = text.as_bytes();
    let mut found = Vec::new();
    let mut index = 0;
    while index < bytes.len() {
        let hashes = raw_hashes(bytes, index);
        let quote = match (bytes[index], hashes) {
            (b'"', _) => Some(index),
            (_, Some(count)) => Some(index + 1 + count),
            _ => None,
        };
        let Some(quote) = quote else {
            index += char_width(bytes, index);
            continue;
        };
        let closer = hashes.map_or(String::from("\""), |count| {
            format!("\"{}", "#".repeat(count))
        });
        let body = quote + 1;
        let end = if hashes.is_some() {
            text[body..].find(&closer).map(|at| body + at)
        } else {
            plain_end(bytes, body)
        };
        let end = end.unwrap_or(bytes.len());
        if text[body..end].starts_with('^') {
            let start = text[..index].rfind([';', '}']).map_or(0, |at| at + 1);
            found.push((&text[body..end], &text[start..index]));
        }
        index = end + closer.len();
    }
    found
}

/// Hashes of a raw string opener at `index`, if one starts there.
fn raw_hashes(bytes: &[u8], index: usize) -> Option<usize> {
    let word_before =
        index > 0 && (bytes[index - 1].is_ascii_alphanumeric() || bytes[index - 1] == b'_');
    if bytes[index] != b'r' || word_before {
        return None;
    }
    let count = bytes[index + 1..]
        .iter()
        .take_while(|byte| **byte == b'#')
        .count();
    (bytes.get(index + 1 + count) == Some(&b'"')).then_some(count)
}

/// Width of the character at `index`, treating a quoted character as one unit.
fn char_width(bytes: &[u8], index: usize) -> usize {
    if bytes[index] != b'\'' {
        return 1;
    }
    match (bytes.get(index + 1), bytes.get(index + 2)) {
        (Some(b'\\'), _) => bytes[index + 2..]
            .iter()
            .position(|byte| *byte == b'\'')
            .map_or(1, |at| at + 3),
        (Some(_), Some(b'\'')) => 3,
        _ => 1,
    }
}

/// End of an ordinary string literal whose body starts at `body`.
fn plain_end(bytes: &[u8], body: usize) -> Option<usize> {
    let mut index = body;
    while index < bytes.len() {
        match bytes[index] {
            b'\\' => index += 2,
            b'"' => return Some(index),
            _ => index += 1,
        }
    }
    None
}

/// Openers in `text` that start with `^` outside `Anchored::new` and outside the allow-list.
fn bypasses<'a>(text: &'a str, allowed: &[(&str, &str)]) -> Vec<&'a str> {
    caret_literals(text)
        .into_iter()
        .filter(|(body, statement)| {
            !statement.contains("Anchored::new(")
                && !allowed.iter().any(|(opener, _)| body.starts_with(opener))
        })
        .map(|(body, _)| body)
        .collect()
}

/// Sources under `directory`, skipping test modules and the owner itself.
fn sources(directory: &Path, out: &mut Vec<(String, String)>) {
    for entry in fs::read_dir(directory).unwrap() {
        let path = entry.unwrap().path();
        let name = path.file_name().unwrap().to_str().unwrap().to_owned();
        if path.is_dir() && name != "tests" && name != "test_support" {
            sources(&path, out);
        } else if path.extension().is_some_and(|ext| ext == "rs")
            && name != "tests.rs"
            && name != "anchored.rs"
        {
            out.push((name, fs::read_to_string(&path).unwrap()));
        }
    }
}

#[test]
fn start_anchored_sources_compile_through_the_owner() {
    let mut files = Vec::new();
    sources(
        &Path::new(env!("CARGO_MANIFEST_DIR")).join("src"),
        &mut files,
    );
    assert!(files.len() > 10);
    for (name, text) in &files {
        assert_eq!(bypasses(text, PLAIN_REGEX), Vec::<&str>::new(), "{name}");
    }
}

#[test]
fn a_start_anchored_source_on_a_plain_regex_is_found_in_every_position() {
    let seeded = [
        "static A: LazyLock<Option<Regex>> = LazyLock::new(|| Regex::new(r\"^a\").ok());",
        "fn b() -> Option<Regex> { Regex::with_flags(r#\"^b\"#, \"u\").ok() }",
        "impl X { fn c(&self) -> Option<Regex> { let s = [r\"^c\", \"d\"].concat(); Regex::new(&s).ok() } }",
        "macro_rules! m { () => { Regex::new(\"^d\") }; }",
        "fn e() { let f = format!(\"^e{}\", 1); }",
    ];
    for source in seeded {
        assert_eq!(bypasses(source, &[]).len(), 1, "{source}");
    }
}

#[test]
fn owner_sources_and_allow_listed_openers_are_accepted() {
    let accepted = [
        "static A: LazyLock<Option<Anchored>> =\n    LazyLock::new(|| Anchored::new(r\"^a\", \"\"));",
        "fn b() { Anchored::new(\n        r#\"^b\"#,\n        \"\",\n    ) }",
        "fn c() { Regex::new(r\"a^\"); Regex::new(r\"[^x]\"); let d = '\"'; }",
        "fn e() { Regex::new(\"^listed\") }",
    ];
    for source in accepted {
        assert!(
            bypasses(source, &[("^listed", "control")]).is_empty(),
            "{source}"
        );
    }
}
