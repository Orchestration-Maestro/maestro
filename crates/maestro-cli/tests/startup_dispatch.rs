//! Startup input acquisition through supplied readers and file effects.
use crate::support::startup;
use maestro_cli::startup::main::inputs::read_piped_stdin;
use std::{
    io,
    pin::Pin,
    task::{Context, Poll},
};
use tokio::io::{AsyncRead, ReadBuf};

/// A reader exposing one byte at a time, including split UTF-8 scalars.
struct Bytes<'a> {
    bytes: std::iter::Copied<std::slice::Iter<'a, u8>>,
}
impl AsyncRead for Bytes<'_> {
    fn poll_read(
        mut self: Pin<&mut Self>,
        _: &mut Context<'_>,
        buffer: &mut ReadBuf<'_>,
    ) -> Poll<io::Result<()>> {
        if let Some(byte) = self.bytes.next() {
            buffer.put_slice(&[byte]);
        }
        Poll::Ready(Ok(()))
    }
}

#[test]
fn piped_input_preserves_whitespace_and_utf8_boundaries() {
    for (bytes, expected) in [
        (&b""[..], None),
        (&b" \r\n\t "[..], None),
        (&b"\xef\xbb\xbftext\xef\xbb\xbf"[..], Some("text")),
        (&b"\xc2\x85text\xc2\x85"[..], Some("\u{85}text\u{85}")),
        (&b" a\r\nb\n\xc3\xa9 "[..], Some("a\r\nb\né")),
        (&b"\xffa\xe2\x82"[..], Some("\u{fffd}a\u{fffd}")),
    ] {
        let mut reader = Bytes {
            bytes: bytes.iter().copied(),
        };
        assert_eq!(
            startup::runtime()
                .block_on(read_piped_stdin(&mut reader, false))
                .unwrap()
                .as_deref(),
            expected
        );
    }
}

/// A reader that refuses any attempt to read.
struct Refused;
impl AsyncRead for Refused {
    fn poll_read(
        self: Pin<&mut Self>,
        _: &mut Context<'_>,
        _: &mut ReadBuf<'_>,
    ) -> Poll<io::Result<()>> {
        Poll::Ready(Err(io::Error::from_raw_os_error(5)))
    }
}

#[test]
fn piped_input_is_unread_for_tty_and_propagates_io_errors() {
    let mut input = &b"kept"[..];
    assert_eq!(
        startup::runtime()
            .block_on(read_piped_stdin(&mut input, true))
            .unwrap(),
        None
    );
    assert_eq!(input, b"kept");
    let error = startup::runtime()
        .block_on(read_piped_stdin(&mut Refused, false))
        .unwrap_err();
    assert_eq!(error.raw_os_error(), Some(5));
}

#[test]
fn initial_preparation_consumes_only_first_message() {
    use maestro_cli::startup::main::inputs::prepare_initial_message;
    let operations = crate::file_inputs::SuppliedFileOperations::new();
    for (messages, stdin, expected, remaining) in [
        (
            vec!["first", "second"],
            Some("stdin"),
            Some("stdinfirst"),
            vec!["second"],
        ),
        (vec![], None, None, vec![]),
        (vec![], Some(""), Some(""), vec![]),
    ] {
        let mut parsed = startup::args(&messages);
        let result = prepare_initial_message(&mut parsed, true, stdin, &operations).unwrap();
        assert_eq!(result.initial_message.as_deref(), expected);
        assert!(result.initial_images.is_none());
        assert_eq!(
            parsed
                .messages
                .iter()
                .map(String::as_str)
                .collect::<Vec<_>>(),
            remaining
        );
    }
    assert!(operations.calls.borrow().is_empty());
}

#[test]
fn file_preparation_forwards_resize_order_and_failures() {
    use maestro_cli::startup::main::inputs::prepare_initial_message;
    use maestro_test_directory::Scratch;
    let scratch = Scratch::new().unwrap();
    let fixture = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/files");
    let text = scratch.path().join("one.txt");
    let second = scratch.path().join("two.txt");
    std::fs::write(&text, "one").unwrap();
    std::fs::write(&second, "two").unwrap();
    let image = fixture.join("tiny.png");
    let wide = fixture.join("wide.png");
    for resize in [false, true] {
        let operations = crate::file_inputs::SuppliedFileOperations::new();
        let mut parsed = startup::args(&["first", "second"]);
        parsed.file_args = [&text, &image, &second, &wide]
            .map(|path| path.to_str().unwrap().to_owned())
            .to_vec();
        let result =
            prepare_initial_message(&mut parsed, resize, Some("stdin"), &operations).unwrap();
        let message = result.initial_message.unwrap();
        assert!(message.starts_with(&format!(
            "stdin<file name=\"{}\">\none\n</file>\n",
            text.display()
        )));
        assert!(message.ends_with("first"));
        assert!(message.find("\none\n").unwrap() < message.find("\ntwo\n").unwrap());
        assert_eq!(result.initial_images, Some(image_expectations(resize)));
        assert_eq!(
            parsed
                .messages
                .iter()
                .map(String::as_str)
                .collect::<Vec<_>>(),
            ["second"]
        );
        assert_eq!(
            operations
                .calls
                .borrow()
                .iter()
                .filter(|call| **call == "resize_image")
                .count(),
            if resize { 2 } else { 0 }
        );
    }
    let operations = crate::file_inputs::RefusingOperations::new(crate::file_inputs::Refusal::Size);
    let mut parsed = startup::args(&["first", "second"]);
    parsed.file_args = vec![text.to_str().unwrap().to_owned()];
    let error = prepare_initial_message(&mut parsed, true, None, &operations)
        .err()
        .unwrap();
    assert_eq!(error.to_string(), "refused by the test route");
    assert_eq!(
        parsed
            .messages
            .iter()
            .map(String::as_str)
            .collect::<Vec<_>>(),
        ["first", "second"]
    );
}

/// Recorded, distinct attachments in the authored order under each resize policy.
fn image_expectations(resize: bool) -> Vec<maestro_app::ImageContent> {
    let cases = if resize {
        ["files-182", "files-190"]
    } else {
        ["files-183", "files-191"]
    };
    cases
        .into_iter()
        .flat_map(|name| {
            crate::file_inputs::expected_images(&crate::support::case_by_title("files", name))
        })
        .collect()
}
