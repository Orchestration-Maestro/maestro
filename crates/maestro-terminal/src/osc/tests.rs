//! Rejection state at command finalization.

use super::{Parser, State};

#[test]
fn failed_finalization_marks_strict_capture_invalid() {
    for input in [b"0".as_slice(), b"52;bad", b"8;id=x;", b"133;"] {
        let mut parser = Parser::init(false);
        for byte in input {
            parser.next(*byte);
        }
        assert_eq!(parser.end(None), None);
        assert_eq!(parser.state, State::Invalid);
    }
}
