//! Character mappings through the terminal crate root.

use maestro_terminal::{Charset, CharsetActiveSlot, CharsetSlot, charset_table};
use serde::Deserialize;

/// One complete byte-to-scalar mapping recorded from the terminal engine.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Mapping {
    /// Fixture family, checked before consuming the mapping.
    kind: String,
    /// Character set name.
    input: String,
    /// Scalar at each byte position.
    value: Vec<u16>,
}

#[test]
fn character_mappings_preserve_every_entry() {
    let rows: Vec<Mapping> =
        serde_json::from_str(include_str!("fixtures/character-sets.json")).unwrap();
    let sets = [Charset::Ascii, Charset::British, Charset::DecSpecial];
    for (row, (name, set)) in rows
        .iter()
        .zip(["ascii", "british", "dec_special"].into_iter().zip(sets))
    {
        assert_eq!(row.kind, "charset");
        assert_eq!(row.input, name);
        assert_eq!(charset_table(set).unwrap().as_slice(), row.value);
    }
    assert_eq!(rows.len(), sets.len());
    assert_eq!(charset_table(Charset::Utf8), None);
    let slots = [
        CharsetSlot::G0,
        CharsetSlot::G1,
        CharsetSlot::G2,
        CharsetSlot::G3,
    ];
    for (i, slot) in slots.iter().enumerate() {
        assert!(!slots[..i].contains(slot));
    }
    assert_ne!(CharsetActiveSlot::Gl, CharsetActiveSlot::Gr);
}
