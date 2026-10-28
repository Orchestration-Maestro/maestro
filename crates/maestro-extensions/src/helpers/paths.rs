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
/// as absolute. The host lists one root, `/`, and opens each path below it as
/// the native path [`native_path`] gives back. Unix paths already have this
/// spelling and stay unchanged.
#[must_use]
pub fn guest_path(path: &str) -> String {
    if cfg!(windows) {
        win32_guest_path(path)
    } else {
        path.to_owned()
    }
}

/// Spells a component path as the native path the host opens.
///
/// It reverses [`guest_path`]. Unix paths stay unchanged.
#[must_use]
pub fn native_path(path: &str) -> String {
    if cfg!(windows) {
        win32_native_path(path)
    } else {
        path.to_owned()
    }
}

/// The Windows component spelling, kept apart so every host checks it.
///
/// A drive path gains a leading `/`, so `C:\x` is `/C:/x`; its verbatim
/// `\\?\` prefix names the same file and is dropped. A share
/// `\\server\share\x` becomes `/?/UNC/server/share/x`, and any other verbatim
/// or device path keeps its prefix as `/?/`. No Windows file name is `?`, so
/// neither form can be read as a path below the current drive's root, which is
/// what any other leading `/` names. Names keep their spelling, so Windows
/// alone decides which spellings name one drive or share.
fn win32_guest_path(path: &str) -> String {
    let path = path.replace('\\', "/");
    match path
        .strip_prefix("//?/")
        .or_else(|| path.strip_prefix("//./"))
    {
        Some(local) if is_drive(local) => format!("/{local}"),
        Some(verbatim) => format!("/?/{verbatim}"),
        None if is_drive(&path) => format!("/{path}"),
        None => match path.strip_prefix("//") {
            Some(share) => format!("/?/UNC/{share}"),
            None => path,
        },
    }
}

/// The Windows native spelling, kept apart so every host checks it.
///
/// It reverses [`win32_guest_path`]: `/C:/x` is `C:\x`, a bare `/C:` is the
/// drive's root, `/?/UNC/server/share/x` is `\\server\share\x` and any other
/// `/?/x` is `\\?\x`. Another absolute path, such as `/tmp/x`, stays below the
/// current drive's root, as it does natively. Relative paths only change
/// separators.
fn win32_native_path(path: &str) -> String {
    let path = path.replace('/', "\\");
    let Some(below) = path.strip_prefix('\\') else {
        return path;
    };
    if let Some(share) = below.strip_prefix(r"?\UNC\") {
        format!(r"\\{share}")
    } else if let Some(verbatim) = below.strip_prefix(r"?\") {
        format!(r"\\?\{verbatim}")
    } else if is_drive(below) && below.len() == 2 {
        format!(r"{below}\")
    } else if is_drive(below) {
        below.to_owned()
    } else {
        path
    }
}

/// Whether a path starts with a Windows drive letter and its colon.
fn is_drive(path: &str) -> bool {
    matches!(path.as_bytes(), [letter, b':', ..] if letter.is_ascii_alphabetic())
}

#[cfg(test)]
mod tests {
    use super::{call, win32_guest_path, win32_native_path};

    /// Windows drive, share and verbatim paths take spellings the host maps back.
    /// Server and share names keep their spelling, so Windows alone decides which
    /// spellings name one share: `ß` and `SS` stay apart.
    #[test]
    fn windows_paths_take_the_component_spelling() {
        for (native, guest) in [
            (r"C:\Users\me\file Ω name", "/C:/Users/me/file Ω name"),
            ("C:/Users/me", "/C:/Users/me"),
            (r"d:\data", "/d:/data"),
            (r"C:\", "/C:/"),
            (r"\\?\C:\Users\me", "/C:/Users/me"),
            (r"\\server\share\dir", "/?/UNC/server/share/dir"),
            (r"\\Server\sHare Ω\Dir", "/?/UNC/Server/sHare Ω/Dir"),
            (r"\\server\ω\Dir", "/?/UNC/server/ω/Dir"),
            (r"\\localhost\straße\x", "/?/UNC/localhost/straße/x"),
            (r"\\localhost\strasse\x", "/?/UNC/localhost/strasse/x"),
            (r"\\localhost\ᾀ\x", "/?/UNC/localhost/ᾀ/x"),
            (r"\\localhost\𐐨\x", "/?/UNC/localhost/𐐨/x"),
            (r"\\?\UNC\server\share\dir", "/?/UNC/server/share/dir"),
            (r"\\?\Volume{0}\dir", "/?/Volume{0}/dir"),
            (r"\\.\pipe\name", "/?/pipe/name"),
            ("//server/share", "/?/UNC/server/share"),
            ("//server", "/?/UNC/server"),
            (r"\Windows\notes", "/Windows/notes"),
            (r"..\sibling\file", "../sibling/file"),
            ("relative/file", "relative/file"),
        ] {
            assert_eq!(win32_guest_path(native), guest, "{native:?}");
        }
    }

    /// The host opens each component spelling as the native path it came from.
    /// A Unix-style absolute path names the current drive's root, never a share.
    #[test]
    fn windows_component_paths_map_back_to_native_paths() {
        for (guest, native) in [
            ("/C:/Users/me/file Ω name", r"C:\Users\me\file Ω name"),
            ("/d:/data", r"d:\data"),
            ("/C:", r"C:\"),
            ("/C:/", r"C:\"),
            ("/?/UNC/server/share/dir", r"\\server\share\dir"),
            ("/?/UNC/localhost/straße/x", r"\\localhost\straße\x"),
            ("/?/Volume{0}/dir", r"\\?\Volume{0}\dir"),
            ("/tmp/x", r"\tmp\x"),
            ("/", r"\"),
            ("../sibling/file", r"..\sibling\file"),
            ("relative/file", r"relative\file"),
        ] {
            assert_eq!(win32_native_path(guest), native, "{guest:?}");
        }
        for native in [
            r"C:\Users\me",
            r"\\server\share\dir",
            r"\\?\Volume{0}\dir",
            r"\Windows\notes",
            r"..\sibling\file",
        ] {
            assert_eq!(win32_native_path(&win32_guest_path(native)), native);
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
