//! Fixed-image materialization through the native developer operation.
#![cfg(not(target_arch = "wasm32"))]
use crate::catalog_command_support::Scratch;
use maestro_models::catalog_generation::generate_test_image::generate_test_image;
use sha2::{Digest, Sha256};
use std::{fs, io, path::Path};

/// Inspect the completed image on the first notice write.
struct ObserveImage<'a> {
    /// Selected target.
    target: &'a Path,
    /// Captured output.
    bytes: Vec<u8>,
}
impl io::Write for ObserveImage<'_> {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        if self.bytes.is_empty() {
            assert_eq!(
                fs::read(self.target)?,
                include_bytes!("data/red-circle.png")
            );
        }
        self.bytes.extend_from_slice(bytes);
        Ok(bytes.len())
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

#[test]
fn circle_materialization_preserves_bytes_and_decoded_pixels() {
    let scratch = Scratch::new().unwrap();
    let target = scratch.path().join("tests/data/red-circle.png");
    let mut output = ObserveImage {
        target: &target,
        bytes: Vec::new(),
    };
    generate_test_image(scratch.path(), &mut output).unwrap();
    let bytes = fs::read(&target).unwrap();
    assert_eq!(bytes.len(), 2_565);
    assert_eq!(
        Sha256::digest(&bytes).as_slice(),
        &[
            0x8a, 0x06, 0x61, 0xc7, 0x39, 0x8b, 0x69, 0xa4, 0x05, 0x1a, 0x47, 0x73, 0x3b, 0x6e,
            0xd5, 0x54, 0x0b, 0x23, 0x54, 0x9f, 0x22, 0x45, 0xba, 0x2d, 0x60, 0x8f, 0xd9, 0xd0,
            0x50, 0xb7, 0xe9, 0x19,
        ]
    );
    let image = image::load_from_memory_with_format(&bytes, image::ImageFormat::Png)
        .unwrap()
        .to_rgba8();
    assert_eq!(image.dimensions(), (200, 200));
    assert_eq!(image.as_raw().len(), 160_000);
    assert_eq!(
        Sha256::digest(image.as_raw()).as_slice(),
        &[
            0xef, 0xb1, 0xee, 0x15, 0x32, 0xb2, 0xbb, 0x2b, 0xb6, 0xa6, 0x69, 0x9a, 0xe3, 0x42,
            0xad, 0x3c, 0xa0, 0xd6, 0x4b, 0x33, 0x32, 0xe1, 0xdf, 0x21, 0x6f, 0x92, 0xc5, 0x43,
            0x15, 0x44, 0xde, 0x54,
        ]
    );
    assert_eq!(
        output.bytes,
        format!("Generated test image at: {}\n", target.display()).as_bytes()
    );
}

#[test]
fn circle_materialization_creates_missing_parents() {
    let scratch = Scratch::new().unwrap();
    let package = scratch
        .path()
        .join("missing/nested/package with spaces é\u{feff}");
    let target = package.join("tests/data/red-circle.png");
    let mut output = Vec::new();
    generate_test_image(&package, &mut output).unwrap();
    assert_eq!(
        fs::read(&target).unwrap(),
        include_bytes!("data/red-circle.png")
    );
    assert_eq!(
        output,
        format!("Generated test image at: {}\n", target.display()).as_bytes()
    );
}

#[test]
fn circle_materialization_replaces_existing_target() {
    let scratch = Scratch::new().unwrap();
    let target = scratch.path().join("tests/data/red-circle.png");
    fs::create_dir_all(target.parent().unwrap()).unwrap();
    fs::write(&target, vec![b'x'; 5_130]).unwrap();
    for _ in 0..2 {
        let mut output = Vec::new();
        generate_test_image(scratch.path(), &mut output).unwrap();
        assert_eq!(
            fs::read(&target).unwrap(),
            include_bytes!("data/red-circle.png")
        );
        assert_eq!(
            output,
            format!("Generated test image at: {}\n", target.display()).as_bytes()
        );
    }
}

#[test]
fn circle_materialization_reports_parent_failure_before_notice() {
    for blocker in ["tests", "tests/data"] {
        let scratch = Scratch::new().unwrap();
        let path = scratch.path().join(blocker);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(&path, b"blocking file").unwrap();
        let expected = fs::create_dir_all(scratch.path().join("tests/data")).unwrap_err();
        let mut output = Vec::new();
        let error = generate_test_image(scratch.path(), &mut output).unwrap_err();
        assert_eq!(error.raw_os_error(), expected.raw_os_error());
        assert!(error.raw_os_error().is_some());
        assert_eq!(fs::read(path).unwrap(), b"blocking file");
        assert!(output.is_empty());
    }
}

#[test]
fn circle_materialization_reports_write_failure_before_notice() {
    let scratch = Scratch::new().unwrap();
    let target = scratch.path().join("tests/data/red-circle.png");
    fs::create_dir_all(&target).unwrap();
    let mut output = Vec::new();
    let error = generate_test_image(scratch.path(), &mut output).unwrap_err();
    let expected = fs::write(&target, b"probe").unwrap_err();
    assert_eq!(error.raw_os_error(), expected.raw_os_error());
    assert!(error.raw_os_error().is_some());
    assert!(target.is_dir());
    assert!(output.is_empty());
}

#[test]
fn circle_notice_failure_keeps_completed_file() {
    let scratch = Scratch::new().unwrap();
    let target = scratch.path().join("tests/data/red-circle.png");
    let notice = format!("Generated test image at: {}\n", target.display());
    for limit in [0, 7] {
        let mut output = crate::catalog_command_support::FailingWriter {
            bytes: Vec::new(),
            remaining: limit,
        };
        let error = generate_test_image(scratch.path(), &mut output).unwrap_err();
        assert_eq!(error.raw_os_error(), Some(28));
        assert_eq!(output.bytes, notice.as_bytes()[..limit]);
        assert_eq!(
            fs::read(&target).unwrap(),
            include_bytes!("data/red-circle.png")
        );
    }
}
