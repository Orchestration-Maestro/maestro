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

#[cfg(test)]
mod tests {
    use super::call;

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
