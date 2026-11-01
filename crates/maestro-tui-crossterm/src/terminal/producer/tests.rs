//! Console text translation and redirected byte streams.

use std::io::{self, Read};

use super::{Arrival, Record, Text};

/// A key-down record with no modifiers.
fn press(unit: u16) -> Record {
    Record::Key {
        down: true,
        key: 0,
        unit,
        repeat: 1,
        control: 0,
    }
}

/// The key presses of `text`, one per UTF-16 unit.
fn typed(text: &str) -> Vec<Record> {
    text.encode_utf16().map(press).collect()
}

/// Arrivals as strings, keeping resize notices visible.
fn strings(arrivals: Vec<Arrival>) -> Vec<String> {
    arrivals
        .into_iter()
        .map(|arrival| match arrival {
            Arrival::Bytes(bytes) => String::from_utf8(bytes).unwrap(),
            Arrival::Resize => "<resize>".into(),
            Arrival::Failed(error) => panic!("unexpected error: {error}"),
        })
        .collect()
}

#[test]
fn console_records_arrive_as_utf8_text_and_resizes_in_order() {
    let emoji: Vec<u16> = "😀".encode_utf16().collect();
    let mut first = typed("a\x1b[Z");
    first.extend([
        Record::Key {
            down: false,
            key: 0x41,
            unit: u16::from(b'a'),
            repeat: 1,
            control: 0,
        },
        Record::Key {
            down: true,
            key: 0x10,
            unit: 0,
            repeat: 1,
            control: 0x10,
        },
        Record::Other,
        press(emoji[0]),
    ]);
    let second = vec![
        press(emoji[1]),
        Record::Key {
            down: true,
            key: 0x42,
            unit: u16::from(b'b'),
            repeat: 3,
            control: 0,
        },
        Record::Resize,
        press(0xdc00),
        Record::Key {
            down: false,
            key: super::VK_MENU,
            unit: u16::from(b'e'),
            repeat: 0,
            control: 0,
        },
    ];
    let mut text = Text::default();
    assert_eq!(strings(text.deliver(first)), ["a\x1b[Z"]);
    assert_eq!(
        strings(text.deliver(second)),
        ["😀bbb", "<resize>", "\u{fffd}e"]
    );
}

#[test]
fn nul_is_text_but_modifier_only_records_are_not() {
    let mut records = Vec::new();
    for control in [0x0004, 0x0008] {
        for key in [0x20, 0x32] {
            records.push(Record::Key {
                down: true,
                key,
                unit: 0,
                repeat: 1,
                control,
            });
        }
    }
    for key in [0x10, 0x11, 0x12, 0x25, 0x70, 0x20] {
        records.push(Record::Key {
            down: true,
            key,
            unit: 0,
            repeat: 1,
            control: 0,
        });
    }
    records.push(Record::Key {
        down: true,
        key: 0x11,
        unit: 0,
        repeat: 1,
        control: 0x0008,
    });
    records.push(press(u16::from(b'x')));
    assert_eq!(strings(Text::default().deliver(records)), ["\0\0\0\0x"]);
}

#[test]
fn redirected_bytes_are_not_decoded_or_filtered_and_eof_is_observed() {
    let bytes = b"\0\xff\x1b[200~paste\x1b[201~";
    let mut read = super::pipe(io::Cursor::new(bytes));
    let arrivals = read().unwrap().unwrap();
    assert!(matches!(&arrivals[..], [Arrival::Bytes(actual)] if actual == bytes));
    assert!(read().unwrap().is_none());
}

/// A redirected input source that fails its first read.
struct Failed;

impl Read for Failed {
    fn read(&mut self, _: &mut [u8]) -> io::Result<usize> {
        Err(io::ErrorKind::BrokenPipe.into())
    }
}

#[test]
fn redirected_read_errors_keep_their_kind() {
    assert_eq!(
        super::pipe(Failed)().unwrap_err().kind(),
        io::ErrorKind::BrokenPipe
    );
}
