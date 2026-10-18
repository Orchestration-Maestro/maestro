//! Terminal emulator behaviour through its public interface.
//!
//! The run-length fixture projects the width column of the upstream engine's
//! `unicode/props_uucode.zig` generated table, captured from its native build.
//! The original table has SHA-256
//! `e923e9c83a3f6a4a2d954a4b0e0a2bd7de3d7bed248931a6f350afbe35b07818`.
//! Each row gives a hexadecimal inclusive start and its cell width; the final
//! run ends at U+10FFFF. Surrogates are retained in the fixture but cannot be
//! passed to the Rust interface.
#![cfg(test)]

use maestro_terminal::unicode::codepoint_width;

#[test]
fn codepoint_width_narrow() {
    assert_eq!(codepoint_width('a'), 1);
}

#[test]
fn codepoint_width_wide() {
    assert_eq!(codepoint_width('\u{4E00}'), 2);
}

#[test]
fn codepoint_width_zero() {
    assert_eq!(codepoint_width('\u{0301}'), 0);
}

#[test]
fn codepoint_width_matches_terminal_cells() {
    for c in ['a', ' ', '\u{10FFFF}'] {
        assert_eq!(codepoint_width(c), 1, "U+{:04X}", u32::from(c));
    }
    for c in [
        '\0', '\u{7}', '\u{1B}', '\u{7F}', '\u{80}', '\u{301}', '\u{200B}', '\u{200D}', '\u{FE0F}',
    ] {
        assert_eq!(codepoint_width(c), 0, "U+{:04X}", u32::from(c));
    }
    for c in [
        '\u{4E00}',
        '\u{FF21}',
        '\u{AC00}',
        '\u{1F600}',
        '\u{1F1E6}',
        '\u{2E3B}',
    ] {
        assert_eq!(codepoint_width(c), 2, "U+{:04X}", u32::from(c));
    }
}

#[test]
fn codepoint_widths_match_engine_table() {
    let runs: Vec<(u32, u8)> = include_str!("fixtures/codepoint-widths.txt")
        .lines()
        .map(|line| {
            let (start, width) = line.split_once(' ').unwrap();
            (
                u32::from_str_radix(start, 16).unwrap(),
                width.parse().unwrap(),
            )
        })
        .chain(std::iter::once((0x11_0000, 0)))
        .collect();
    assert_eq!(runs[0].0, 0);
    let mut scalars = 0;
    for pair in runs.windows(2) {
        let (start, width) = pair[0];
        let end = pair[1].0;
        assert!(start < end && end <= 0x11_0000);
        assert!(width <= 2);
        for cp in start..end {
            if let Some(c) = char::from_u32(cp) {
                assert_eq!(codepoint_width(c), width, "U+{cp:04X}");
                scalars += 1;
            }
        }
    }
    assert_eq!(scalars, 0x11_0000 - 0x800);
}

#[test]
fn character_widths_match_screen_cells() {
    assert_eq!(codepoint_width('a'), 1);
    assert_eq!(codepoint_width('\u{0301}'), 0);
    assert_eq!(codepoint_width('界'), 2);
}

mod graphics_commands;
mod graphics_media;

mod page_storage;

mod character_sets;
mod vt_sequences;

mod terminal_colors;

mod sgr_attributes;

mod osc_commands;

mod support;
