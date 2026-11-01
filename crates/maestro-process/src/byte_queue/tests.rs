//! Consuming fronts retain offsets established by the existing write path.
use super::{WriteStatus, Writes};

#[test]
fn retained_front_reads_only_unwritten_bytes() {
    let mut queue = Writes::default();
    queue.enqueue(b"abc".to_vec());
    queue.enqueue(b"de".to_vec());
    let mut first = true;
    let result = queue.flush(|_| {
        if std::mem::take(&mut first) {
            Ok(1)
        } else {
            Err(std::io::ErrorKind::WouldBlock.into())
        }
    });
    assert!(matches!(result, WriteStatus::Blocked));
    assert_eq!(queue.pending(), 4);
    assert_eq!(queue.take_front(), Some(b"bc".to_vec()));
    assert_eq!(queue.pending(), 2);
    assert_eq!(queue.take_front(), Some(b"de".to_vec()));
    assert_eq!(queue.pending(), 0);
    assert_eq!(queue.take_front(), None);
}
