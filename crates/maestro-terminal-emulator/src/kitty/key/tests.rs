//! Keyboard stack replay preserves nested and disabled states.
use super::{FlagStack, Flags, SetMode};

#[test]
fn keyboard_stack_replay_retains_pushes_and_zero_entries() {
    let mut stack = FlagStack::default();
    stack.push(Flags::DISAMBIGUATE);
    stack.push(Flags::DISABLED);
    stack.push(Flags::from_int(5).unwrap());
    let mut replay = String::new();
    stack.replay_ansi(&mut replay).unwrap();
    assert_eq!(replay, "\x1b[>1u\x1b[>0u\x1b[>5u");
    stack.pop(2);
    replay.clear();
    stack.replay_ansi(&mut replay).unwrap();
    assert_eq!(replay, "\x1b[>1u");
    stack.pop(1);
    stack.set(SetMode::Set, Flags::REPORT_ALL);
    replay.clear();
    stack.replay_ansi(&mut replay).unwrap();
    assert_eq!(replay, "\x1b[=8u");
}
