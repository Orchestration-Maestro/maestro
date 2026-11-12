## Approved tool-argument libraries

`maestro-models` may use the following exact native primitive pins for argument
checking, without an internal crate edge or an additional public schema API:

- `num-bigint =0.4.8`, `num-traits =0.2.19`,
  `unicode-segmentation =1.13.3`, `regress =0.12.0`, `url =2.5.8`,
  `percent-encoding =2.3.2` and `idna =1.1.0`, with their default features;
  all are MIT OR Apache-2.0.
- `ryu-js =1.0.3`, with defaults, Apache-2.0 OR BSL-1.0; use Apache-2.0.
- `chrono =0.4.45`, defaults off, only `std`, MIT OR Apache-2.0.
- `icu_normalizer =2.3.0`, defaults off, only `compiled_data`, Unicode-3.0.
  This direct dependency is already in the IDNA closure and supplies NFC without
  replacing the case-sensitive hostname context checks.

The IDNA dependency closure also contains Unicode-3.0 ICU components. Keep every
pin exact and inspect the resolved licence and feature closure after changes.
