//! Keyboard protocol flags retained independently by each terminal screen.

use maestro_terminal::kitty::key::{FlagStack, Flags, SetMode};

#[test]
fn keyboard_push_pop() {
    let mut stack = FlagStack::default();
    stack.push(Flags::DISAMBIGUATE);
    assert_eq!(stack.current(), Flags::DISAMBIGUATE);
    stack.pop(1);
    assert_eq!(stack.current(), Flags::DISABLED);
}

#[test]
fn keyboard_flag_bits_match_protocol_positions() {
    assert_eq!(Flags::DISAMBIGUATE.int(), 0b1);
    assert_eq!(Flags::REPORT_EVENTS.int(), 0b10);
    assert_eq!(Flags::REPORT_ALTERNATES.int(), 0b100);
    assert_eq!(Flags::REPORT_ALL.int(), 0b1000);
    assert_eq!(Flags::REPORT_ASSOCIATED.int(), 0b10000);
    assert_eq!(Flags::TRUE.int(), 0b11111);
    assert_eq!(Flags::DISABLED.int(), 0);
}

#[test]
fn keyboard_pop_big_number() {
    let mut stack = FlagStack::default();
    stack.pop(100);
    assert_eq!(stack.current(), Flags::DISABLED);
}

#[test]
fn keyboard_ring_index_wraps_both_directions() {
    let mut stack = maestro_terminal::kitty::KeyFlagStack::default();
    stack.set(SetMode::Set, Flags::REPORT_EVENTS);
    for _ in 0..7 {
        stack.push(Flags::DISAMBIGUATE);
    }
    stack.push(Flags::REPORT_ALL);
    assert_eq!(stack.current(), Flags::REPORT_ALL);
    stack.pop(1);
    assert_eq!(stack.current(), Flags::DISAMBIGUATE);
    stack.push(Flags::REPORT_ASSOCIATED);
    stack.pop(7);
    assert_eq!(stack.current(), Flags::DISAMBIGUATE);
    stack.pop(1);
    assert_eq!(stack.current(), Flags::DISABLED);
}

#[test]
fn keyboard_full_ring_and_pop_boundaries() {
    let mut stack = FlagStack::default();
    stack.set(SetMode::Set, Flags::REPORT_EVENTS);
    let values = [
        Flags::DISAMBIGUATE,
        Flags::REPORT_EVENTS,
        Flags::REPORT_ALTERNATES,
        Flags::REPORT_ALL,
        Flags::REPORT_ASSOCIATED,
        Flags::TRUE,
        Flags::REPORT_EVENTS,
        Flags::REPORT_ALL,
        Flags::REPORT_ASSOCIATED,
    ];
    for flags in values {
        stack.push(flags);
    }
    stack.pop(0);
    assert_eq!(stack.current(), Flags::REPORT_ASSOCIATED);
    stack.pop(7);
    assert_eq!(stack.current(), Flags::REPORT_EVENTS);
    stack.pop(1);
    assert_eq!(stack.current(), Flags::DISABLED);

    for count in [8, usize::MAX] {
        for flags in values {
            stack.push(flags);
        }
        stack.pop(count);
        assert_eq!(stack.current(), Flags::DISABLED);
        for _ in 0..8 {
            stack.pop(1);
            assert_eq!(stack.current(), Flags::DISABLED);
        }
    }
    stack.set(SetMode::Set, Flags::TRUE);
    stack.set(SetMode::Not, Flags::TRUE);
    assert_eq!(stack.current(), Flags::DISABLED);
    stack.set(SetMode::Or, Flags::TRUE);
    assert_eq!(stack.current().int(), 0b11111);
    stack.set(SetMode::Set, Flags::DISAMBIGUATE);
    assert_eq!(stack.current(), Flags::DISAMBIGUATE);
}

#[test]
fn keyboard_set() {
    let mut stack = FlagStack::default();
    stack.set(SetMode::Set, Flags::DISAMBIGUATE);
    assert_eq!(stack.current(), Flags::DISAMBIGUATE);
    stack.set(SetMode::Or, Flags::REPORT_EVENTS);
    assert_eq!(stack.current().int(), 0b11);
    stack.set(SetMode::Not, Flags::REPORT_EVENTS);
    assert_eq!(stack.current(), Flags::DISAMBIGUATE);
}
