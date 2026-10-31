//! Native byte publication and symbolic-link policies.
#![cfg(test)]
use maestro_lock::{Replace, replace};
use std::{fs, path::PathBuf};

/// Isolated directory removed after each native transaction.
struct Directory(maestro_test_directory::Scratch);
impl Directory {
    fn new() -> Self {
        Self(maestro_test_directory::Scratch::new().unwrap())
    }
    /// The directory this test owns.
    fn path(&self) -> &std::path::Path {
        self.0.path()
    }
}

#[test]
fn replacement_handles_relative_and_extensionless_paths() {
    let dir = Directory::new();
    let relative = dir
        .path()
        .strip_prefix(std::env::current_dir().unwrap())
        .map_or_else(
            |_| PathBuf::from("target").join(format!("replacement-{}", std::process::id())),
            PathBuf::from,
        );
    let path = relative.join("nested/file");
    replace(
        &path,
        b"new bytes",
        Replace {
            through_symlinks: true,
            ..Replace::default()
        },
    )
    .unwrap();
    assert_eq!(fs::read(&path).unwrap(), b"new bytes");
    assert_eq!(fs::read_dir(path.parent().unwrap()).unwrap().count(), 1);
    fs::remove_dir_all(relative).unwrap();
    assert!(
        replace(
            std::path::Path::new("/"),
            b"no",
            Replace {
                through_symlinks: true,
                ..Replace::default()
            }
        )
        .is_err()
    );
    #[cfg(unix)]
    {
        use std::os::unix::ffi::{OsStrExt, OsStringExt};
        let name = std::ffi::OsString::from_vec(vec![b'n', 255]);
        let path = dir.path().join(&name);
        // A host may refuse names that are not valid UTF-8; the observations above stay
        // unconditional.
        match fs::write(&path, b"probe") {
            Ok(()) => (),
            // A control name in the same directory is stored, so this is the host refusing
            // these bytes rather than another write failure.
            Err(error) if fs::write(dir.path().join("control"), b"control").is_ok() => {
                eprintln!("host refuses the name bytes {:?}: {error}", name.as_bytes());
                return;
            }
            Err(error) => panic!("the recorded name bytes could not be written: {error}"),
        }
        // Leave the target absent, so the subcase still creates it.
        fs::remove_file(&path).unwrap();
        replace(
            &path,
            b"native",
            Replace {
                through_symlinks: false,
                ..Replace::default()
            },
        )
        .unwrap();
        assert_eq!(fs::read(path).unwrap(), b"native");
    }
}

#[cfg(unix)]
#[test]
fn replacement_refuses_a_read_only_target_and_leaves_it_whole() {
    let dir = Directory::new();
    let target = dir.path().join("target");
    fs::write(&target, b"old").unwrap();
    let Some(_restored) = maestro_test_directory::deny(&target, 0o400).unwrap() else {
        return;
    };

    let error = replace(&target, b"new", Replace::default()).unwrap_err();

    assert_eq!(error.kind(), std::io::ErrorKind::PermissionDenied);
    assert_eq!(fs::read(&target).unwrap(), b"old");
    assert_eq!(fs::read_dir(dir.path()).unwrap().count(), 1);
}

#[cfg(unix)]
#[test]
fn replacement_preserves_long_and_dangling_chains() {
    for (count, existing) in [
        (1, true),
        (16, true),
        (17, true),
        (40, true),
        (17, false),
        (41, true),
    ] {
        let dir = Directory::new();
        let target = dir.path().join("target");
        if existing {
            fs::write(&target, b"old").unwrap();
        }
        for i in 0..count {
            let next = if i + 1 == count {
                "target".into()
            } else {
                format!("link-{}", i + 1)
            };
            std::os::unix::fs::symlink(next, dir.path().join(format!("link-{i}"))).unwrap();
        }
        let before = fs::read_dir(dir.path()).unwrap().count();
        let result = replace(
            &dir.path().join("link-0"),
            b"published",
            Replace {
                through_symlinks: true,
                ..Replace::default()
            },
        );
        if count == 41 {
            assert_eq!(
                result.unwrap_err().to_string(),
                "symbolic link chain exceeds 40 links"
            );
            assert_eq!(fs::read(&target).unwrap(), b"old");
            assert_eq!(fs::read_dir(dir.path()).unwrap().count(), before);
        } else {
            result.unwrap();
            assert_eq!(fs::read(target).unwrap(), b"published");
            assert_eq!(fs::read_dir(dir.path()).unwrap().count(), count + 1);
        }
        for i in 0..count {
            let expected = if i + 1 == count {
                "target".into()
            } else {
                format!("link-{}", i + 1)
            };
            assert_eq!(
                fs::read_link(dir.path().join(format!("link-{i}"))).unwrap(),
                PathBuf::from(expected)
            );
        }
    }
}

#[cfg(unix)]
#[test]
fn replacement_can_replace_link_entry_when_following_is_disabled() {
    let dir = Directory::new();
    let target = dir.path().join("target");
    let link = dir.path().join("link");
    fs::write(&target, b"old").unwrap();
    std::os::unix::fs::symlink("target", &link).unwrap();
    replace(
        &link,
        b"followed",
        Replace {
            through_symlinks: true,
            ..Replace::default()
        },
    )
    .unwrap();
    assert_eq!(fs::read(&target).unwrap(), b"followed");
    assert_eq!(fs::read_link(&link).unwrap(), PathBuf::from("target"));
    replace(
        &link,
        b"entry",
        Replace {
            through_symlinks: false,
            ..Replace::default()
        },
    )
    .unwrap();
    assert_eq!(fs::read(&link).unwrap(), b"entry");
    assert!(
        !fs::symlink_metadata(&link)
            .unwrap()
            .file_type()
            .is_symlink()
    );
    assert_eq!(fs::read(&target).unwrap(), b"followed");
}

#[cfg(unix)]
#[test]
fn replacement_rejects_cycles_without_touching_disk() {
    for pair in [false, true] {
        let dir = Directory::new();
        std::os::unix::fs::symlink(
            if pair { "second" } else { "first" },
            dir.path().join("first"),
        )
        .unwrap();
        if pair {
            std::os::unix::fs::symlink("first", dir.path().join("second")).unwrap();
        }
        let result = replace(
            &dir.path().join("first"),
            b"no",
            Replace {
                through_symlinks: true,
                ..Replace::default()
            },
        );
        assert_eq!(
            result.unwrap_err().to_string(),
            "symbolic link chain exceeds 40 links"
        );
        assert_eq!(
            fs::read_link(dir.path().join("first")).unwrap(),
            PathBuf::from(if pair { "second" } else { "first" })
        );
        if pair {
            assert_eq!(
                fs::read_link(dir.path().join("second")).unwrap(),
                PathBuf::from("first")
            );
        }
        assert_eq!(
            fs::read_dir(dir.path()).unwrap().count(),
            if pair { 2 } else { 1 }
        );
    }
}

#[test]
fn replacement_accepts_a_full_length_target_leaf() {
    let dir = maestro_test_directory::Scratch::new().unwrap();
    let target = dir.path().join("x".repeat(255));
    if let Err(error) = fs::write(&target, b"old") {
        assert!(fs::write(dir.path().join("control"), b"control").is_ok());
        eprintln!("host refuses a 255-byte leaf: {error}");
        return;
    }
    replace(&target, b"published", Replace::default()).unwrap();
    assert_eq!(fs::read(&target).unwrap(), b"published");
    fs::remove_file(&target).unwrap();
    replace(&target, b"created", Replace::default()).unwrap();
    assert_eq!(fs::read(target).unwrap(), b"created");
    assert_eq!(fs::read_dir(dir.path()).unwrap().count(), 1);
}
