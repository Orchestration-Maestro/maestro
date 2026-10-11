//! Native device failures through public output operations.
#![cfg(target_os = "linux")]

use super::{BufWriter, File, OutputAccumulator, OutputAccumulatorOptions};

#[test]
fn buffered_artifact_flush_failure_is_returned() {
    let mut output = OutputAccumulator::new(OutputAccumulatorOptions::default());
    output.writer = Some(BufWriter::with_capacity(
        8,
        File::options().write(true).open("/dev/full").unwrap(),
    ));
    output.append(b"small").unwrap();
    assert_eq!(
        output.close_temp_file().unwrap_err().raw_os_error(),
        Some(28)
    );
}

#[test]
fn direct_artifact_write_failure_is_returned() {
    let mut output = OutputAccumulator::new(OutputAccumulatorOptions::default());
    output.writer = Some(BufWriter::with_capacity(
        2,
        File::options().write(true).open("/dev/full").unwrap(),
    ));
    assert_eq!(
        output.append(b"larger").unwrap_err().raw_os_error(),
        Some(28)
    );
}
