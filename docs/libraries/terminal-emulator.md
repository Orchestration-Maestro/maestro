# Terminal emulator libraries

The terminal emulator uses `icu_properties =2.3.0` with default features disabled
and `compiled_data` enabled. Its compiled Unicode 17 data supplies general
category, East Asian width, grapheme-cluster break, default-ignorable, emoji
modifier, modifier-base, extended-pictographic and Indic-conjunct properties.
The terminal uses these properties for scalar widths and tailored streaming
cluster boundaries. A skin-tone modifier joins a modifier base, but not arbitrary
preceding text.

`unicode/variation.rs` retains 371 emoji variation-sequence bases from Unicode
17.0 `emoji-variation-sequences.txt` as inclusive constant ranges. The compiled
property library does not expose this sequence-base property.

The fixtures under `crates/maestro-terminal/tests/fixtures/unicode/` retain
captured scalar-property runs and pairwise transitions. Each property row holds
a hexadecimal range start, scalar width, zero-width-in-cluster flag, tailored
break property and variation-base flag. Each transition row holds the incoming
state, both break properties, the boundary result and the outgoing state. The
fixtures cover the full scalar range and all five-state, seventeen-property
combinations without dropping result or state fields.

The crate is already locked in the workspace and is licensed under
Unicode-3.0. No Unicode data is downloaded or generated during a build.

## Image protocol codecs and native media

- `base64 =0.23.1` uses default features and the standard alphabet, indifferent
  padding and allowed trailing bits. The parser removes only space, tab, LF, CR
  and form feed, then rejects padded lengths not divisible by four. This retains
  omitted padding but rejects partial padding; encoded bytes count against the
  parser budget before normalization.
- `flate2 =1.1.10` disables defaults and enables `zlib-rs` for zlib inflation.
- `png =0.17.16` uses default features with `EXPAND | STRIP_16` transformations.
- Unix-only `rustix =1.1.5` retains defaults and enables `shm` for Linux named
  shared-memory access. Owned descriptors and reads replace memory mapping.

These versions are already in the workspace lockfile. Their selected licences
are MIT, Apache-2.0, Apache-2.0 WITH LLVM-exception or Zlib as applicable. No
foreign compiler or unsafe code is introduced.

## Clipboard decoding

OSC clipboard writes reuse `base64 =0.23.1` with its `STANDARD` engine: standard
alphabet, required canonical padding and rejected nonzero trailing bits.
Whitespace that reaches this decoder is rejected. This configuration is separate
from the image protocol's forgiving decoder; it does not change image handling.
