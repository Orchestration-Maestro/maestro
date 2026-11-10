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

#[test]
fn rolling_trim_releases_oversized_allocation() {
    let mut output = OutputAccumulator::new(OutputAccumulatorOptions {
        max_bytes: Some(1024),
        ..Default::default()
    });
    output.append(&vec![b'x'; 1024 * 1024]).unwrap();
    assert_eq!(output.tail.len(), 2048);
    assert!(output.tail.capacity() <= 4096);
    output.append(b"y").unwrap();
    assert!(output.tail.capacity() <= 4096);
    let snapshot = output.snapshot(false).unwrap();
    assert_eq!(snapshot.content, format!("{}y", "x".repeat(1023)));
    assert_eq!(snapshot.truncation.total_bytes, 1024 * 1024 + 1);
    let path = snapshot.full_output_path.unwrap();
    output.close_temp_file().unwrap();
    std::fs::remove_file(path).unwrap();
}
