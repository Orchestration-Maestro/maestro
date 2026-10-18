//! Canonical temporary-file admission through its shared predicate and reader.
include!("../../../../tests/support/images.rs");
#[cfg(unix)]
use super::file::selected_temp_dir;
use super::{
    ImageError,
    file::{is_path_in_temp_dir, read_path},
};
use crate::kitty::graphics::command::{Medium, Transmission};
use std::fs;
#[cfg(unix)]
use std::path::PathBuf;

#[test]
fn temporary_paths_require_canonical_containment() {
    let scratch = Scratch::new();
    let root = scratch.0.join("selected");
    let sibling = scratch.0.join("selected-sibling");
    fs::create_dir(&root).unwrap();
    fs::create_dir(&sibling).unwrap();
    let roots = vec![fs::canonicalize(&root).unwrap()];
    let outside = scratch.file("selected-sibling/tty-graphics-protocol-outside.rgb", b"ABC");
    let inside = scratch.file("selected/tty-graphics-protocol-inside.rgb", b"ABC");
    let outside = fs::canonicalize(outside).unwrap();
    let inside = fs::canonicalize(inside).unwrap();
    assert!(!is_path_in_temp_dir(&outside, &roots));
    assert!(is_path_in_temp_dir(&inside, &roots));
    let t = Transmission {
        medium: Medium::TemporaryFile,
        ..Transmission::default()
    };
    assert_eq!(
        read_path(&outside, &t, &roots),
        Err(ImageError::TemporaryFileNotInTempDir)
    );
    assert_eq!(fs::read(&outside).unwrap(), b"ABC");
    assert_eq!(read_path(&inside, &t, &roots).unwrap(), b"ABC");
    assert!(!inside.exists());
    #[cfg(unix)]
    {
        let alias = scratch.0.join("alias");
        std::os::unix::fs::symlink(&root, &alias).unwrap();
        let roots = vec![fs::canonicalize(alias).unwrap()];
        let spelled = root
            .join("..")
            .join("selected-sibling")
            .join("tty-graphics-protocol-outside.rgb");
        let canonical = fs::canonicalize(spelled).unwrap();
        assert!(!is_path_in_temp_dir(&canonical, &roots));
        assert_eq!(
            read_path(&canonical, &t, &roots),
            Err(ImageError::TemporaryFileNotInTempDir)
        );
        assert!(outside.exists());
        let outside_alias = root.join("outside-alias");
        std::os::unix::fs::symlink(&outside, &outside_alias).unwrap();
        let canonical = fs::canonicalize(&outside_alias).unwrap();
        assert!(!is_path_in_temp_dir(&canonical, &roots));
        assert_eq!(
            read_path(&canonical, &t, &roots),
            Err(ImageError::TemporaryFileNotInTempDir)
        );
        assert_eq!(fs::read(&outside).unwrap(), b"ABC");
        assert!(outside_alias.symlink_metadata().is_ok());
    }
    #[cfg(unix)]
    assert_temp_directory_selection();
}

/// Assert variable precedence and separator-only trimming without global writes.
#[cfg(unix)]
fn assert_temp_directory_selection() {
    assert_eq!(selected_temp_dir(None, None), PathBuf::from("/tmp"));
    assert_eq!(
        selected_temp_dir(None, Some("/second//".into())).as_os_str(),
        "/second"
    );
    assert_eq!(
        selected_temp_dir(Some("/first//".into()), Some("/second".into())).as_os_str(),
        "/first"
    );
    assert_eq!(
        selected_temp_dir(Some("".into()), Some("/second".into())).as_os_str(),
        ""
    );
    assert_eq!(
        selected_temp_dir(Some(" /first /".into()), None).as_os_str(),
        " /first "
    );
    assert_eq!(selected_temp_dir(Some("///".into()), None).as_os_str(), "");
}
