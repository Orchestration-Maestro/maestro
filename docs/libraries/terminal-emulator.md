# Terminal emulator libraries

The terminal emulator uses `icu_properties =2.3.0` with default features disabled
and `compiled_data` enabled. Its compiled Unicode 17 data supplies general
category, East Asian width, grapheme-cluster break, default-ignorable and emoji
modifier properties for codepoint cell widths.

The crate is already locked in the workspace and is licensed under
Unicode-3.0. No Unicode data is downloaded or generated during a build.
