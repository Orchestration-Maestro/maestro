//! Native path requests reuse the existing lexical and filesystem owner.
use super::{decode, encode};
/// Runs one path operation without interpreting its spelling in the adapter.
pub(super) fn call(operation: &str, data: &str) -> Result<String, String> {
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
