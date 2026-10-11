## Approved theme libraries

`maestro-theme` uses `jsonschema =0.58.5` (MIT; defaults off, offline) for runtime
theme admission, `indexmap =2.14.2` (default `std`) for ordered color records, and
the workspace `serde` (`derive`, decoding the brand pack), `serde_json`, `num-traits`. Outside browser targets it also uses
`icu_collator =2.3.1` and `icu_locale_core =2.3.0` (defaults off, `alloc`; Unicode-3.0)
for locale-sorted theme inventories, selecting the locale as the completion
collation does. No third-party type appears in its interface. Its production
dependency closure adds the internal `maestro-request` and `maestro-path` crates.
