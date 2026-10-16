//! Recreate the fixed circle image used by native model tests.
use std::{fs, io, io::Write, path::Path};

/// Immutable PNG data, embedded so materialization needs no runtime source file.
const IMAGE: &[u8] = include_bytes!("../../tests/data/red-circle.png");

/// Write the fixed image under `package_dir`, then report its selected path.
///
/// # Errors
/// Returns the original directory-creation, file-write or notice-write error.
/// A notice failure leaves the completed image in place.
pub fn generate_test_image(package_dir: &Path, output: &mut dyn Write) -> io::Result<()> {
    let path = package_dir.join("tests/data/red-circle.png");
    fs::create_dir_all(package_dir.join("tests/data"))?;
    fs::write(&path, IMAGE)?;
    writeln!(output, "Generated test image at: {}", path.display())
}
