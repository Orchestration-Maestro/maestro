//! Default archive extraction security on Unix filesystems.
use super::{Staging, archive};
use std::{fs, io::Cursor, os::unix::fs::PermissionsExt, path::Path};

/// Read all Unix permission bits, including privileged bits.
fn mode(path: &Path) -> u32 {
    fs::metadata(path).unwrap().permissions().mode() & 0o7777
}

#[test]
fn zip_extraction_masks_privileged_mode_bits() {
    let scratch = Staging::create(std::env::temp_dir().to_str().unwrap(), "zip-modes").unwrap();
    let root = Path::new(&scratch.path);
    let mut zip = zip::ZipArchive::new(Cursor::new(archive("privileged.zip"))).unwrap();
    zip.extract(root).unwrap();
    assert_eq!(mode(&root.join("payload")), 0o777);
    assert_eq!(mode(&root.join("ordinary")), 0o640);
    assert_eq!(fs::read(root.join("payload")).unwrap(), b"payload");
    assert_eq!(fs::read(root.join("ordinary")).unwrap(), b"ordinary");
}

#[test]
fn zip_extraction_confines_symlink_targets() {
    for fixture in ["outside-link.zip", "chained-link.zip", "inside-link.zip"] {
        let scratch = Staging::create(std::env::temp_dir().to_str().unwrap(), "zip-links").unwrap();
        let root = Path::new(&scratch.path);
        let destination = root.join("extract");
        fs::create_dir(&destination).unwrap();
        let outside = root.join("outside");
        fs::write(&outside, b"unchanged").unwrap();
        fs::set_permissions(&outside, fs::Permissions::from_mode(0o600)).unwrap();
        if fixture == "chained-link.zip" {
            std::os::unix::fs::symlink("../outside", destination.join("target")).unwrap();
        }
        let mut zip = zip::ZipArchive::new(Cursor::new(archive(fixture))).unwrap();
        let result = zip.extract(&destination);
        if fixture == "inside-link.zip" {
            result.unwrap();
            assert_eq!(fs::read(destination.join("link")).unwrap(), b"ordinary");
            assert_eq!(mode(&destination.join("ordinary")), 0o640);
        } else {
            assert!(result.is_err(), "{fixture}");
            assert!(fs::symlink_metadata(destination.join("link")).is_err());
        }
        assert_eq!(fs::read(&outside).unwrap(), b"unchanged");
        assert_eq!(mode(&outside), 0o600);
    }
}
