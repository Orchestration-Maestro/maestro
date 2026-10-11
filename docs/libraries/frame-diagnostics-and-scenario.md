## Approved frame-diagnostics and scenario libraries

`maestro-tui` may use `serde =1.0.229` (defaults, `derive`) and the workspace
`serde_json =1.0.151` (defaults, `preserve_order`) to serialize its typed
diagnostic records; both are MIT OR Apache-2.0 and no type of either appears in a
public interface. The added closure is `serde_core 1.0.229`, `serde_derive 1.0.229`,
`indexmap 2.14.2`, `equivalent 1.0.2`, `hashbrown 0.17.1`, `itoa 1.0.18`,
`zmij 1.0.23` (MIT), `proc-macro2 1.0.107`, `quote 1.0.47`, `syn 3.0.6` and
`unicode-ident 1.0.24`, which keeps its lockfile hold.

`maestro-test-terminal` alone may use `vt100 =0.16.2` (MIT, default features) as a
development dependency to replay the frame writer's bytes into an emulated screen,
with dev-only `serde =1.0.229` and `serde_json` as above to read typed fixtures. The
emulator's closure is `vte 0.15.0` (`default`, `std`; Apache-2.0 OR MIT),
`arrayvec 0.7.8` (MIT OR Apache-2.0), `itoa 1.0.18`, `memchr 2.8.3` (`alloc`, `std`;
choose MIT) and `unicode-width 0.2.2` (`default`, `cjk`). The emulator never enters
a product crate, and no crate depends on the harness.
