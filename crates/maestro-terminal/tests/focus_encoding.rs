//! Focus reports sent to pane programs.

use maestro_terminal::focus::{Event, encode};

#[test]
fn focus_events_encode_gain_and_loss() {
    assert_eq!(encode(Event::Gained), b"\x1b[I");
    assert_eq!(encode(Event::Lost), b"\x1b[O");
}
