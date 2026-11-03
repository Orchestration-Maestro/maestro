//! The external editor and the Wayland clipboard reader of the interactive mode.

use std::ffi::OsString;
use std::io::{self, Write};
use std::path::Path;

use maestro_test_process::child::{leave, write_stdout};

/// Record every operand, keep the edited file as it was, replace it and end with `status`.
///
/// The edited file is the last operand. `report` receives `argv`, one operand per line, and
/// `before`, a copy of the file that keeps its permission bits.
pub(super) fn edit(
    report: &str,
    content: &str,
    status: u8,
    operands: &[OsString],
) -> io::Result<()> {
    let file = operands
        .last()
        .ok_or_else(|| io::Error::other("no file to edit"))?;
    let mut argv = std::fs::File::create(Path::new(report).join("argv"))?;
    for operand in operands {
        argv.write_all(operand.as_encoded_bytes())?;
        argv.write_all(b"\n")?;
    }
    std::fs::copy(file, Path::new(report).join("before"))?;
    std::fs::write(file, content)?;
    leave(i32::from(status))
}

/// List the offered types for `--list-types` and print the image for any other request.
pub(super) fn paste(types: &str, image: &[u8], operands: &[OsString]) -> io::Result<()> {
    if operands
        .first()
        .is_some_and(|first| first == "--list-types")
    {
        write_stdout(types.as_bytes())?;
    } else {
        write_stdout(image)?;
    }
    leave(0)
}
