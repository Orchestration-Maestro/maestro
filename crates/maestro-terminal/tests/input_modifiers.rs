//! Modifier masks retain keys, locks and independent side observations.

use maestro_terminal::input::key_mods::{Mod, Mods, Side};

#[test]
fn modifier_keys_locks_and_sides_have_independent_bits() {
    let mods = Mods::SHIFT
        .union(Mods::CTRL)
        .union(Mods::CAPS_LOCK)
        .union(Mods::NUM_LOCK)
        .with_side(Mod::Shift, Side::Right);
    assert_eq!(mods.int(), 0b0111_0011);
    assert_eq!(mods.binding(), Mods::SHIFT.union(Mods::CTRL));
    assert_eq!(mods.without_locks().int(), 0b0100_0011);
    assert_eq!(mods.side(Mod::Shift), Side::Right);
    assert_eq!(mods.side(Mod::Ctrl), Side::Left);
    assert!(mods.contains(Mods::SHIFT));
    assert!(!mods.contains(Mods::ALT));
    assert!(Mods::default().empty());
    assert!(!Mods::default().with_side(Mod::Alt, Side::Right).empty());
}

#[test]
fn modifier_masks_preserve_all_sides_when_keys_are_removed() {
    let mods = Mods::from_int(0b0011_1111)
        .with_side(Mod::Shift, Side::Right)
        .with_side(Mod::Ctrl, Side::Right)
        .with_side(Mod::Alt, Side::Right)
        .with_side(Mod::Super, Side::Right);
    assert_eq!(mods.int(), 0x03ff);
    assert_eq!(mods.unset(Mods::SHIFT.union(Mods::ALT)).int(), 0x03fa);
    assert_eq!(mods.with_side(Mod::Alt, Side::Left).int(), 0x02ff);
    assert_eq!(Mods::ALT.int(), 4);
    assert_eq!(Mods::SUPER.int(), 8);
    assert_eq!(Mods::from_int(0xffff).int(), 0xffff);
}
