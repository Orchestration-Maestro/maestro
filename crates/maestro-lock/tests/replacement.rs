//! Native byte publication and symbolic-link policies.
#![cfg(test)]
use maestro_lock::{Replace, replace};
use std::{fs, path::PathBuf};

/// Isolated directory removed after each native transaction.
struct Directory(PathBuf);
impl Directory {
    fn new(name: &str) -> Self {
        let path =
            std::env::temp_dir().join(format!("maestro-replacement-{}-{name}", std::process::id()));
        fs::create_dir(&path).unwrap();
        Self(path)
    }
}
impl Drop for Directory {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).unwrap();
    }
}

#[test]
fn replacement_handles_relative_and_extensionless_paths() {
    let dir = Directory::new("paths");
    let relative = dir
        .0
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
                through_symlinks: true
            }
        )
        .is_err()
    );
    #[cfg(unix)]
    {
        use std::os::unix::ffi::{OsStrExt, OsStringExt};
        let name = std::ffi::OsString::from_vec(vec![b'n', 255]);
        let path = dir.0.join(&name);
        // A host may refuse names that are not valid UTF-8; the observations above stay
        // unconditional.
        if fs::write(&path, b"probe").is_err() {
            eprintln!("host refuses the name bytes {:?}", name.as_bytes());
            return;
        }
        replace(
            &path,
            b"native",
            Replace {
                through_symlinks: false,
            },
        )
        .unwrap();
        assert_eq!(fs::read(path).unwrap(), b"native");
    }
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
        let dir = Directory::new(&format!("chain-{count}-{existing}"));
        let target = dir.0.join("target");
        if existing {
            fs::write(&target, b"old").unwrap();
        }
        for i in 0..count {
            let next = if i + 1 == count {
                "target".into()
            } else {
                format!("link-{}", i + 1)
            };
            std::os::unix::fs::symlink(next, dir.0.join(format!("link-{i}"))).unwrap();
        }
        let before = fs::read_dir(&dir.0).unwrap().count();
        let result = replace(
            &dir.0.join("link-0"),
            b"published",
            Replace {
                through_symlinks: true,
            },
        );
        if count == 41 {
            assert_eq!(
                result.unwrap_err().to_string(),
                "symbolic link chain exceeds 40 links"
            );
            assert_eq!(fs::read(&target).unwrap(), b"old");
            assert_eq!(fs::read_dir(&dir.0).unwrap().count(), before);
        } else {
            result.unwrap();
            assert_eq!(fs::read(target).unwrap(), b"published");
            assert_eq!(fs::read_dir(&dir.0).unwrap().count(), count + 1);
        }
        for i in 0..count {
            let expected = if i + 1 == count {
                "target".into()
            } else {
                format!("link-{}", i + 1)
            };
            assert_eq!(
                fs::read_link(dir.0.join(format!("link-{i}"))).unwrap(),
                PathBuf::from(expected)
            );
        }
    }
}

#[cfg(unix)]
#[test]
fn replacement_can_replace_link_entry_when_following_is_disabled() {
    let dir = Directory::new("policies");
    let target = dir.0.join("target");
    let link = dir.0.join("link");
    fs::write(&target, b"old").unwrap();
    std::os::unix::fs::symlink("target", &link).unwrap();
    replace(
        &link,
        b"followed",
        Replace {
            through_symlinks: true,
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
        let dir = Directory::new(&format!("cycle-{pair}"));
        std::os::unix::fs::symlink(if pair { "second" } else { "first" }, dir.0.join("first"))
            .unwrap();
        if pair {
            std::os::unix::fs::symlink("first", dir.0.join("second")).unwrap();
        }
        let result = replace(
            &dir.0.join("first"),
            b"no",
            Replace {
                through_symlinks: true,
            },
        );
        assert_eq!(
            result.unwrap_err().to_string(),
            "symbolic link chain exceeds 40 links"
        );
        assert_eq!(
            fs::read_link(dir.0.join("first")).unwrap(),
            PathBuf::from(if pair { "second" } else { "first" })
        );
        if pair {
            assert_eq!(
                fs::read_link(dir.0.join("second")).unwrap(),
                PathBuf::from("first")
            );
        }
        assert_eq!(
            fs::read_dir(&dir.0).unwrap().count(),
            if pair { 2 } else { 1 }
        );
    }
}
