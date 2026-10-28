//! Native path requests reuse the existing lexical and filesystem owner.
use super::{decode, encode};
/// Runs one path operation without interpreting its spelling in the adapter.
pub(super) fn call(operation: &str, data: &str) -> Result<String, String> {
    if operation == "paths.make_temp_dir" {
        let (prefix,): (String,) = decode(data)?;
        return encode(
            &maestro_tools::output::output_accumulator::make_temp_dir(&prefix)
                .map_err(|error| error.to_string())?,
        );
    }
    if operation == "paths.basename" {
        let (path,): (String,) = decode(data)?;
        return encode(&maestro_path::basename(&path, None));
    }
    if operation == "paths.expand_path" {
        let (path,): (String,) = decode(data)?;
        return encode(
            &maestro_tools::path_utils::expand_path(&path).map_err(|error| error.to_string())?,
        );
    }
    if operation == "paths.guest_path" {
        let (path,): (String,) = decode(data)?;
        return encode(&guest_path(&path));
    }
    if operation == "paths.resolve" {
        let (paths,): (Vec<String>,) = decode(data)?;
        let paths: Vec<_> = paths.iter().map(String::as_str).collect();
        return encode(
            &maestro_resources::paths::resolve(&paths).map_err(|error| error.to_string())?,
        );
    }
    let (path, cwd): (String, String) = decode(data)?;
    match operation {
        "paths.resolve_to_cwd" => encode(
            &maestro_tools::path_utils::resolve_to_cwd(&path, &cwd)
                .map_err(|error| error.to_string())?,
        ),
        "paths.resolve_read_path" => encode(
            &maestro_tools::path_utils::resolve_read_path(&path, &cwd)
                .map_err(|error| error.to_string())?,
        ),
        _ => Err(format!("Unknown native helper: {operation}")),
    }
}

/// Spells a native path the way a component's standard library opens it.
///
/// The component's libc treats only `/` as a separator and only a leading `/`
/// as absolute. The host names each filesystem root it exposes with this same
/// spelling. Unix paths already have it and stay unchanged.
#[must_use]
pub fn guest_path(path: &str) -> String {
    if cfg!(windows) {
        win32_guest_path(path)
    } else {
        path.to_owned()
    }
}

/// The Windows spelling, kept apart so every host checks it.
///
/// A drive path gains a leading `/` and an uppercase letter, so `c:\x`
/// reaches the `/C:` root, and a share `\\server\share` becomes
/// `/SERVER/SHARE`: the form the component's libc reports for its cwd.
/// Windows ignores letter case in drive, server and share names, while the
/// component matches roots byte for byte, so each spelling of one root gets
/// the same name. Names uppercase as Windows compares them, one character at
/// a time. The verbatim `\\?\` prefix names the same file and is
/// dropped.
fn win32_guest_path(path: &str) -> String {
    let path = match path.strip_prefix(r"\\?\") {
        Some(share) if share.starts_with(r"UNC\") => format!(r"\{}", &share[3..]),
        Some(local) => local.to_owned(),
        None => path.to_owned(),
    }
    .replace('\\', "/");
    match path.as_bytes() {
        [letter, b':', ..] if letter.is_ascii_alphabetic() => {
            format!("/{}{}", letter.to_ascii_uppercase() as char, &path[1..])
        }
        [b'/', b'/', ..] => {
            let root = path[2..]
                .match_indices('/')
                .nth(1)
                .map_or(path.len(), |(at, _)| at + 2);
            let name: String = path[1..root].chars().map(upcase).collect();
            format!("{name}{}", &path[root..])
        }
        _ => path,
    }
}

/// Uppercases one character the way Windows compares names.
///
/// Windows maps each UTF-16 unit to at most one unit, so a character whose
/// uppercase form is longer, such as `ß`, or that needs two units keeps its
/// case. Expanding it would give distinct shares like `ß` and `SS` one root.
fn upcase(character: char) -> char {
    let mut upper = character.to_uppercase();
    match (upper.next(), upper.next()) {
        (Some(single), None) if character.len_utf16() == 1 => single,
        _ => character,
    }
}

#[cfg(test)]
mod tests {
    use super::{call, win32_guest_path};

    /// Windows drive, share and verbatim paths reach the component's roots.
    /// Share names uppercase one UTF-16 unit at a time, as Windows compares
    /// them, so shares Windows tells apart keep separate roots.
    #[test]
    fn windows_paths_take_the_component_spelling() {
        for (native, guest) in [
            (r"C:\Users\me\file Ω name", "/C:/Users/me/file Ω name"),
            ("C:/Users/me", "/C:/Users/me"),
            (r"d:\data", "/D:/data"),
            (r"C:\", "/C:/"),
            (r"\\?\C:\Users\me", "/C:/Users/me"),
            (r"\\server\share\dir", "/SERVER/SHARE/dir"),
            (r"\\Server\sHare Ω\Dir", "/SERVER/SHARE Ω/Dir"),
            (r"\\server\ω\Dir", "/SERVER/Ω/Dir"),
            (r"\\localhost\straße\x", "/LOCALHOST/STRAßE/x"),
            (r"\\localhost\strasse\x", "/LOCALHOST/STRASSE/x"),
            (r"\\localhost\𐐨\x", "/LOCALHOST/𐐨/x"),
            (r"\\?\UNC\server\share\dir", "/SERVER/SHARE/dir"),
            ("//server/share", "/SERVER/SHARE"),
            ("//server", "/SERVER"),
            (r"..\sibling\file", "../sibling/file"),
            ("relative/file", "relative/file"),
        ] {
            assert_eq!(win32_guest_path(native), guest, "{native:?}");
        }
    }

    /// The helper route returns the running host's spelling.
    #[test]
    fn guest_path_helper_uses_the_host_spelling() {
        let (native, guest) = if cfg!(windows) {
            (r"C:\work\a b", "/C:/work/a b")
        } else {
            (r"/work/a\b", r"/work/a\b")
        };
        let data = serde_json::to_string(&(native,)).unwrap();
        assert_eq!(
            call("paths.guest_path", &data),
            Ok(serde_json::to_string(guest).unwrap())
        );
    }

    /// Basename uses the native path flavor for roots and trailing separators.
    #[test]
    fn basename_uses_host_roots_and_trailing_separators() {
        let root = if cfg!(windows) { r"C:\" } else { "/" };
        for (path, expected) in [(root, ""), ("/work/project///", "project"), ("", "")] {
            let data = serde_json::to_string(&(path,)).unwrap();
            assert_eq!(
                call("paths.basename", &data),
                Ok(serde_json::to_string(expected).unwrap())
            );
        }
    }

    /// Temporary directories are distinct, private and confined to the host temporary root.
    #[test]
    fn temporary_directories_are_private_and_distinct() {
        let make = |prefix: &str| {
            let data = serde_json::to_string(&(prefix,)).unwrap();
            let encoded = call("paths.make_temp_dir", &data).unwrap();
            std::path::PathBuf::from(serde_json::from_str::<String>(&encoded).unwrap())
        };
        let first = make("maestro-rg-");
        let second = make("maestro-rg-");
        let third = make("maestro-other-");
        assert!(first.is_dir());
        assert!(second.is_dir());
        assert!(third.is_dir());
        assert_ne!(first, second);
        assert_eq!(
            first.parent(),
            Some(maestro_test_directory::temporary_root().as_path())
        );
        assert!(
            first
                .file_name()
                .unwrap()
                .to_str()
                .unwrap()
                .starts_with("maestro-rg-")
        );
        assert!(
            third
                .file_name()
                .unwrap()
                .to_str()
                .unwrap()
                .starts_with("maestro-other-")
        );
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            for path in [&first, &second, &third] {
                assert_eq!(
                    std::fs::metadata(path).unwrap().permissions().mode() & 0o777,
                    0o700
                );
            }
        }
        std::fs::remove_dir(first).unwrap();
        std::fs::remove_dir(second).unwrap();
        std::fs::remove_dir(third).unwrap();
        for prefix in [
            "",
            ".",
            "..",
            "../outside",
            "a/b",
            "a\\b",
            "a\0b",
            "C:outside",
        ] {
            let data = serde_json::to_string(&(prefix,)).unwrap();
            assert!(call("paths.make_temp_dir", &data).is_err(), "{prefix:?}");
        }
    }

    /// The host home becomes `~` only for itself and the paths below it, never for a
    /// sibling that merely starts with the same characters.
    #[test]
    fn shorten_path_abbreviates_only_home_and_descendants() {
        let test = "shorten_path_abbreviates_only_home_and_descendants";
        let home = || {
            std::collections::BTreeMap::from([("HOME", "/home/al"), ("USERPROFILE", "/home/al")])
        };
        let selected = maestro_test_process::run_per_environment(test, ["home"], |_| home());
        if selected.unwrap().is_none() {
            return;
        }
        let home = maestro_tools::path_utils::expand_path("~").unwrap();
        for (path, expected) in [
            (home.clone(), "~".to_owned()),
            (format!("{home}/project"), "~/project".to_owned()),
            (format!("{home}ice"), format!("{home}ice")),
            ("/work".to_owned(), "/work".to_owned()),
        ] {
            let data = serde_json::to_string(&(&path,)).unwrap();
            assert_eq!(
                call("paths.shorten_path", &data),
                Ok(serde_json::to_string(&expected).unwrap()),
                "{path:?}"
            );
        }
    }

    /// A backslash is part of a POSIX directory name, not a separator.
    #[cfg(unix)]
    #[test]
    fn basename_preserves_posix_backslashes() {
        assert_eq!(
            call("paths.basename", r#"["/work/a\\b"]"#),
            Ok(r#""a\\b""#.into())
        );
    }
}
