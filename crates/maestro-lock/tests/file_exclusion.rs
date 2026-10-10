//! Native owned-file exclusion.

use std::fs::{File, OpenOptions, TryLockError};
use std::io::{Read, Seek, SeekFrom, Write};

#[test]
fn acquired_file_preserves_handle_and_exclusion_until_release() {
    let path = std::env::temp_dir().join(format!("maestro-lock-{}", std::process::id()));
    let mut original = OpenOptions::new()
        .create_new(true)
        .read(true)
        .write(true)
        .open(&path)
        .unwrap();
    original.write_all(b"retained contents").unwrap();
    original.seek(SeekFrom::Start(3)).unwrap();
    #[cfg(unix)]
    let descriptor = std::os::fd::AsRawFd::as_raw_fd(&original);
    let mut acquired = maestro_lock::acquire(original).unwrap();
    #[cfg(unix)]
    assert_eq!(std::os::fd::AsRawFd::as_raw_fd(&acquired), descriptor);
    assert_eq!(acquired.stream_position().unwrap(), 3);
    let mut contents = String::new();
    acquired.read_to_string(&mut contents).unwrap();
    assert_eq!(contents, "ained contents");
    let independent = File::options().read(true).write(true).open(&path).unwrap();
    assert!(matches!(
        independent.try_lock(),
        Err(TryLockError::WouldBlock)
    ));
    acquired.unlock().unwrap();
    let later = maestro_lock::acquire(independent).unwrap();
    assert!(matches!(acquired.try_lock(), Err(TryLockError::WouldBlock)));
    drop(later);
    acquired.try_lock().unwrap();
    drop(acquired);
    std::fs::remove_file(path).unwrap();
}
