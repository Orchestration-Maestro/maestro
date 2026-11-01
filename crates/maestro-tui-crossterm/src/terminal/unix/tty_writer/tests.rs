//! Bounded write requests and exact advancement of the shared byte queue.

use std::num::NonZeroUsize;

use maestro_process::byte_queue::{WriteStatus, Writes};

#[test]
fn native_output_bounds_each_write() {
    for length in [0, 1, 16383, 16384, 16385, 32775, 65536] {
        let data = vec![b'x'; length];
        let mut queue = Writes::default();
        queue.enqueue(data.clone());
        let mut requests = Vec::new();
        let mut observed = Vec::new();
        assert!(matches!(
            queue.flush_bounded(NonZeroUsize::new(16384).unwrap(), |bytes| {
                requests.push(bytes.len());
                observed.extend_from_slice(bytes);
                Ok(bytes.len())
            }),
            WriteStatus::Complete
        ));
        assert_eq!(observed, data);
        assert!(requests.iter().all(|size| *size <= 16384));
        if length == 32775 {
            assert_eq!(requests, [16384, 16384, 7]);
        }
        assert_eq!(queue.pending(), 0);
    }
}

#[test]
fn native_output_partial_interrupt_and_zero_results() {
    use std::io;
    let mut queue = Writes::default();
    queue.enqueue(b"abcdefgh".to_vec());
    let mut requests = Vec::new();
    let mut progress = Vec::new();
    let mut observed = Vec::new();
    assert!(matches!(
        queue.flush(|bytes| {
            requests.push(bytes.len());
            Err(io::ErrorKind::Interrupted.into())
        }),
        WriteStatus::Blocked
    ));
    assert_eq!(queue.pending(), 8);
    assert!(matches!(
        queue.flush(|bytes| {
            requests.push(bytes.len());
            let count = bytes.len().min(3);
            progress.push(count);
            observed.extend_from_slice(&bytes[..count]);
            Ok(count)
        }),
        WriteStatus::Complete
    ));
    assert_eq!(requests, [8, 8, 5, 2]);
    assert_eq!(progress, [3, 3, 2]);
    assert_eq!(observed, b"abcdefgh");
    queue.enqueue(b"x".to_vec());
    let zero = super::write_result(queue.flush(|_| Ok(0))).unwrap_err();
    assert_eq!(zero.kind(), io::ErrorKind::WriteZero);
    let broken =
        super::write_result(queue.flush(|_| Err(rustix::io::Errno::PIPE.into()))).unwrap_err();
    assert_eq!(broken.kind(), io::ErrorKind::BrokenPipe);
    assert_eq!(
        broken.raw_os_error(),
        Some(rustix::io::Errno::PIPE.raw_os_error())
    );
    assert_eq!(queue.pending(), 0);
}
