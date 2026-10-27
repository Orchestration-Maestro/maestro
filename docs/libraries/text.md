# Approved text libraries

`maestro-text` has no dependency by default. Its `collation` feature enables
`icu_collator =2.3.1` with default features, including `compiled_data`, and
`icu_locale_core =2.3.0` with defaults off and `alloc`. Both use Unicode-3.0. The
resolved closure is 36 existing packages; no new package was added when the
direct declarations moved from the catalog, theme and toolkit crates here. The
comparator accepts a caller-supplied environment reader and exposes no ICU type.
`maestro-models` keeps `icu_collator` only as a test dependency. Test code
reads fixtures with the workspace `serde` and `serde_json`.
See [text](../text.md).

## Locale numbers

The optional `number-format` feature enables `icu_decimal =2.3.0` with its default
features plus `alloc` and `ryu`, and `fixed_decimal =0.7.2` with defaults off. It
shares the existing `icu_locale_core` declaration with collation. Public methods
return I/O results without exposing foreign formatter types. Decimal conversion
uses round-trip precision, explicit half-expand rounding at magnitude -3, then
fractional-zero trimming.

The resolved active normal/build formatter closure is 27 packages:

- `displaydoc =0.2.7`.
- `fixed_decimal =0.7.2`.
- `icu_decimal =2.3.0`.
- `icu_decimal_data =2.3.0`.
- `icu_locale_core =2.3.0`.
- `icu_locale_fallback =2.3.0`.
- `icu_locale_fallback_data =2.3.0`.
- `icu_provider =2.3.1`.
- `litemap =0.8.3`.
- `potential_utf =0.1.6`.
- `proc-macro2 =1.0.107`.
- `quote =1.0.47`.
- `ryu =1.0.23`.
- `smallvec =1.16.2`.
- `stable_deref_trait =1.2.1`.
- `syn =3.0.6`.
- `synstructure =0.14.0`.
- `tinystr =0.8.4`.
- `unicode-ident =1.0.24`.
- `writeable =0.6.4`.
- `yoke =0.8.3`.
- `yoke-derive =0.8.4`.
- `zerofrom =0.1.8`.
- `zerofrom-derive =0.1.8`.
- `zerotrie =0.2.5`.
- `zerovec =0.11.8`.
- `zerovec-derive =0.11.6`.

The ICU and supporting data crates use Unicode-3.0. `displaydoc`, `proc-macro2`,
`quote`, `syn`, `smallvec` and `stable_deref_trait` offer MIT OR Apache-2.0;
`synstructure` uses MIT. `unicode-ident` requires Unicode-3.0 together with the
MIT/Apache alternative. `ryu` is used under its Apache-2.0 option.

## Local date-times

The optional `date-time-format` feature reuses the number feature's decimal
engine, with grouping disabled, and directly enables the already-locked
`icu_locale_fallback =2.3.0` with default compiled data. No calendar or date-time
runtime library is added. The formatter renders committed CLDR JSON 48.2.0
recipes, under Unicode-3.0, and converts the default Gregorian, Buddhist and
Persian calendars in Maestro code. The data and translated Persian algorithm's
notices are in `crates/maestro-text/data/date-time/`.
