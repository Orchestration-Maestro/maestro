//! Device attribute response encoding.

use maestro_terminal::device_attributes::{Primary, Secondary, Tertiary};

#[test]
fn primary_default() {
    let mut bytes = Vec::new();
    Primary::default().encode(&mut bytes).unwrap();
    assert_eq!(bytes, b"\x1b[?62;22c");
}

#[test]
fn primary_custom() {
    let mut bytes = Vec::new();
    Primary {
        conformance_level: 64,
        features: vec![1, 6, 22],
    }
    .encode(&mut bytes)
    .unwrap();
    assert_eq!(bytes, b"\x1b[?64;1;6;22c");
}

#[test]
fn primary_no_features() {
    let mut bytes = Vec::new();
    Primary {
        conformance_level: 1,
        features: vec![],
    }
    .encode(&mut bytes)
    .unwrap();
    assert_eq!(bytes, b"\x1b[?1c");
}

#[test]
fn secondary_default() {
    let mut bytes = Vec::new();
    Secondary::default().encode(&mut bytes).unwrap();
    assert_eq!(bytes, b"\x1b[>1;0;0c");
}

#[test]
fn secondary_custom() {
    let mut bytes = Vec::new();
    Secondary {
        device_type: 65,
        firmware_version: 123,
        rom_cartridge: 7,
    }
    .encode(&mut bytes)
    .unwrap();
    assert_eq!(bytes, b"\x1b[>65;123;7c");
}

#[test]
fn tertiary_default() {
    let mut bytes = Vec::new();
    Tertiary::default().encode(&mut bytes).unwrap();
    assert_eq!(bytes, b"\x1bP!|00000000\x1b\\");
}

#[test]
fn tertiary_custom() {
    let mut bytes = Vec::new();
    Tertiary {
        unit_id: 0x12ab_cdef,
    }
    .encode(&mut bytes)
    .unwrap();
    assert_eq!(bytes, b"\x1bP!|12ABCDEF\x1b\\");
}

#[test]
fn primary_with_clipboard() {
    let mut bytes = Vec::new();
    Primary {
        features: vec![22, 52],
        ..Primary::default()
    }
    .encode(&mut bytes)
    .unwrap();
    assert_eq!(bytes, b"\x1b[?62;22;52c");
}
