# Spreadsheet formatting libraries

`maestro-app` uses these direct dependencies for its
[spreadsheet values and workbooks](../application/document-formats.md):

| Library | Pin | Requested features | License used |
| --- | --- | --- | --- |
| calamine | `=0.36.1` | Defaults off; no optional features | MIT |
| ryu-js | `=1.0.3` | Existing defaults; no optional features | Apache-2.0 |
| chrono | `=0.4.45` | Defaults off; `std` (includes `alloc`) | Apache-2.0 |
| num-traits | `=0.2.19` | Existing defaults and `std` | Apache-2.0 |

[ryu-js](https://docs.rs/ryu-js/1.0.3/ryu_js/) supplies shortest and fixed numeric
spelling. General significant/exponential formatting, serial normalization and
numeric-pattern scientific/engineering writers share an application-owned decimal
adapter. Numeric-pattern writers also reuse bounded fraction approximation;
the date writer uses supplied calendar observations. Numeric comma/period
separators and placeholder precision are pattern-defined, not locale-number
formatting.
[chrono::NaiveDate](https://docs.rs/chrono/0.4.45/chrono/struct.NaiveDate.html)
supplies date-only calendar arithmetic. No clock, timezone or browser-binding
feature is requested by this application dependency.
[num-traits](https://docs.rs/num-traits/0.2.19/num_traits/) supplies checked numeric
conversions on native and browser targets.

The isolated dependency closure also includes build-time `autocfg =1.5.1`, used
under Apache-2.0. Other workspace consumers can unify additional features; the
requested features above describe this edge, not the whole workspace.

Derived format data and policy retain the
[formatting source attribution](../../THIRD-PARTY-NOTICES.md#spreadsheet-formatting-source).
[calamine](https://docs.rs/calamine/0.36.1/calamine/) supplies native XLSX cell
values through its streaming reader and legacy XLS values through its owned
range reader. The application reads style indices, raw
error labels, dimensions and merges through the existing indexed ZIP/XML owners;
it uses its own number-format evaluator, not the parser's display text. Legacy
XLS reading reuses the scalar projection with General text, populated bounds,
ordered sheets and merges. Raw style metadata, format-table effects, declared blank
dimensions and binary formula metadata remain tracked in
[legacy workbook metadata](https://github.com/Orchestration-Maestro/maestro/issues/1025).

The parser's dependency closure uses MIT or Apache-2.0 where offered, with
BSD-3-Clause for encoding_rs, Unicode-3.0 for unicode-ident and Zlib for zlib-rs.
Its ZIP and XML dependencies are implementation details; no additional direct
ZIP/XML dependency or optional date feature is requested.
