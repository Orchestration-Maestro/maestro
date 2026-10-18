# Terminal emulator libraries

The terminal emulator uses `icu_properties =2.3.0` with default features disabled
and `compiled_data` enabled. Its compiled Unicode 17 data supplies general
category, East Asian width, grapheme-cluster break, default-ignorable and emoji
modifier properties for codepoint cell widths.

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
