//! APC routing through the retained image command parser.

use maestro_terminal::{apc::Handler, kitty::graphics::command::Control};

#[test]
fn valid_image_command() {
    let mut h = Handler::default();
    h.start();
    for &byte in b"Gf=24,s=10,v=20,hello=world" {
        h.feed(byte);
    }
    let command = h.end().unwrap();
    assert!(matches!(command.control, Control::Transmit(_)));
}

#[test]
fn unknown_apc_command() {
    let mut h = Handler::default();
    h.start();
    for &byte in b"Xabcdef1234" {
        h.feed(byte);
    }
    assert!(h.end().is_none());
}

#[test]
fn garbage_image_command() {
    let mut h = Handler::default();
    h.start();
    h.feed_slice(b"Gabcdef1234");
    assert!(h.end().is_none());
}

#[test]
fn image_command_with_overflow_u32() {
    let mut h = Handler::default();
    h.start();
    h.feed_slice(b"Ga=p,i=10000000000");
    assert!(h.end().is_none());
}

#[test]
fn image_command_with_overflow_i32() {
    let mut h = Handler::default();
    h.start();
    h.feed_slice(b"Ga=p,i=1,z=-9999999999");
    assert!(h.end().is_none());
}

#[test]
fn image_feed_error_releases_parser() {
    let mut h = Handler::default();
    h.start();
    for &byte in b"Ga=p,i=10000000000;" {
        h.feed(byte);
    }
    assert!(matches!(h.state, maestro_terminal::apc::State::Ignore));
    assert!(h.end().is_none());
    h.start();
    h.feed_slice(b"Ga=q,i=1;");
    assert!(matches!(h.end().unwrap().control, Control::Query(_)));
}

#[test]
fn image_max_bytes_exceeded() {
    let mut h = Handler::default();
    h.max_bytes = 4;
    h.start();
    for &byte in b"Ga=t;abcd" {
        h.feed(byte);
    }
    assert!(matches!(h.state, maestro_terminal::apc::State::Kitty(_)));
    h.feed(b'e');
    assert!(matches!(h.state, maestro_terminal::apc::State::Ignore));
    assert!(h.end().is_none());
}

#[test]
fn identify_with_unrecognized_command() {
    let mut h = Handler::default();
    h.start();
    h.feed_slice(b"abcd;payload");
    assert!(h.end().is_none());
}

#[test]
fn identify_buffer_overflow() {
    let mut h = Handler::default();
    h.start();
    h.feed_slice(b"abcde;payload");
    assert!(h.end().is_none());
}

#[test]
fn identify_with_no_input() {
    let mut h = Handler::default();
    h.start();
    assert!(h.end().is_none());
}

#[test]
fn identify_with_unknown_partial_input() {
    let mut h = Handler::default();
    h.start();
    h.feed_slice(b"25a");
    assert!(h.end().is_none());
}

#[test]
fn feed_slice_valid_image_command() {
    let mut h = Handler::default();
    h.start();
    h.feed_slice(b"Gf=24,s=10,v=20;aGVsbG8=");
    assert_eq!(h.end().unwrap().data, b"hello");
}

#[test]
fn feed_slice_identify_split_across_slices() {
    let mut h = Handler::default();
    h.start();
    h.feed_slice(b"G");
    h.feed_slice(b"f=24,s=10,");
    h.feed_slice(b"v=20;aGVsbG8=");
    let command = h.end().unwrap();
    assert_eq!(command.data, b"hello");
    let Control::Transmit(transmission) = command.control else {
        panic!("expected transmission");
    };
    assert_eq!((transmission.width, transmission.height), (10, 20));
}

#[test]
fn feed_slice_unknown_apc_command_is_ignored() {
    let mut h = Handler::default();
    h.start();
    h.feed_slice(b"Xabcdef1234");
    assert!(matches!(h.state, maestro_terminal::apc::State::Ignore));
    h.feed_slice(b"more data that is dropped");
    assert!(h.end().is_none());
}

#[test]
fn feed_slice_image_max_bytes_exceeded() {
    let mut h = Handler::default();
    h.max_bytes = 4;
    h.start();
    h.feed_slice(b"Ga=t;abcd");
    assert!(matches!(h.state, maestro_terminal::apc::State::Kitty(_)));
    h.feed_slice(b"e");
    assert!(matches!(h.state, maestro_terminal::apc::State::Ignore));
    assert!(h.end().is_none());
}

#[test]
fn disabled_glyph_command_is_ignored() {
    let mut h = Handler::default();
    h.start();
    h.feed_slice(b"25a1;q;cp=e0a0");
    assert!(h.end().is_none());
}

#[test]
fn disabling_image_protocol_only_affects_future_strings() {
    use maestro_terminal::apc::Protocol;
    let mut h = Handler::default();
    h.start();
    h.feed_slice(b"G");
    h.enable(Protocol::Kitty, false);
    h.feed_slice(b"a=q,i=1;");
    assert!(matches!(h.end().unwrap().control, Control::Query(_)));
    h.start();
    h.feed_slice(b"Ga=q,i=1;");
    h.enable(Protocol::Kitty, true);
    assert!(h.end().is_none());
    h.start();
    h.feed_slice(b"Ga=q,i=2;");
    assert_eq!(h.end().unwrap().transmission().unwrap().image_id, 2);
}

#[test]
fn restart_releases_unfinished_payload_and_resamples_limit() {
    let mut h = Handler::default();
    h.max_bytes = 4;
    h.start();
    h.feed_slice(b"Ga=t;abcd");
    h.start();
    h.max_bytes = 0;
    h.feed_slice(b"Ga=t;");
    h.max_bytes = 4;
    h.feed_slice(b"YQ==");
    assert!(h.end().is_none());
    h.start();
    h.feed_slice(b"Ga=t;YQ==");
    assert_eq!(h.end().unwrap().data, b"a");
}

#[test]
fn terminal_write_routes_split_apc_without_printing_payload() {
    use super::support::terminal::assert_text;
    use maestro_terminal::{Terminal, apc::State};
    let mut terminal = Terminal::new(30, 2, 0).unwrap();
    terminal.write(b"before\x1b_Ga=q,i=1;");
    assert!(matches!(terminal.apc_handler.state, State::Kitty(_)));
    terminal.write(b"aGVsbG8=");
    assert!(matches!(terminal.apc_handler.state, State::Kitty(_)));
    terminal.write(b"\x1b");
    assert!(matches!(terminal.apc_handler.state, State::Inactive));
    terminal.write(b"\\after");
    assert!(matches!(terminal.apc_handler.state, State::Inactive));
    assert_text(&terminal, "beforeafter");
    assert!(terminal.take_pty_reply().is_empty());
}

#[test]
fn terminal_write_honours_apc_limits_and_disabled_protocol() {
    use super::support::terminal::assert_text;
    use maestro_terminal::{
        Terminal,
        apc::{Protocol, State},
    };
    let mut terminal = Terminal::new(30, 2, 0).unwrap();
    terminal.apc_handler.max_bytes = 4;
    terminal.write(b"A\x1b_Ga=t;YWJj");
    assert!(matches!(terminal.apc_handler.state, State::Kitty(_)));
    terminal.write(b"Z");
    assert!(matches!(terminal.apc_handler.state, State::Ignore));
    terminal.write(b"\x1b\\B");
    terminal.apc_handler.enable(Protocol::Kitty, false);
    terminal.write(b"\x1b_Ga=q,i=1;");
    assert!(matches!(terminal.apc_handler.state, State::Ignore));
    terminal.write(b"\x1b\\C\x1b_unknown;data\x1b\\D");
    assert_text(&terminal, "ABCD");
    assert!(terminal.take_pty_reply().is_empty());
}
