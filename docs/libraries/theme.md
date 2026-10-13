## Approved theme libraries

`maestro-theme` uses `jsonschema =0.58.5` (MIT; defaults off, offline) for runtime
theme admission, `indexmap =2.14.2` (default `std`) for ordered color records, and
the workspace `serde` (`derive`, decoding theme records), `serde_json`, `num-traits`. Outside browser targets it also enables the `collation` feature of `maestro-text`
([text libraries](text.md)) for locale-sorted theme inventories. No third-party type appears in its interface. Its production
dependency closure adds the internal `maestro-request` and `maestro-path` crates.
