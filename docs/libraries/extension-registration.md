# Extension registration libraries

Owned registration uses the existing approved `indexmap =2.14.2` with default
features for insertion-ordered replacement. No additional feature is enabled.
Its license choice is MIT. The current native workspace feature union includes
`equivalent =1.0.2` (MIT), `hashbrown =0.17.1` (MIT), `foldhash =0.2.0` (Zlib),
`allocator-api2 =0.2.21` (MIT) and `serde_core =1.0.229` (MIT).

The used lower owners are `maestro-models` (boxed futures), `maestro-resources`
(canonical provenance and diagnostics) and `maestro-tui` (completion and resolved
keybinding records). Their library pins, feature policies and licenses remain in
the existing area documents; this registration API selects no provider or runtime.
