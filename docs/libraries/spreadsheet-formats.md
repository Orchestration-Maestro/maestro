# Spreadsheet formatting libraries

`maestro-app` uses these direct dependencies for its
[spreadsheet values and workbooks](../application/document-formats.md):

| Library | Pin | Requested features | License used |
| --- | --- | --- | --- |
| codepage | `=0.1.3` | No features | Apache-2.0 |
| encoding_rs | `=0.8.42` | Defaults | Apache-2.0 AND BSD-3-Clause |
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
The application owns its XLS compound-file and record reader. XLSX scalars,
styles, dimensions and merges use the existing indexed ZIP/XML owners in one
pass; neither path delegates display text to another parser.
[codepage](https://docs.rs/codepage/0.1.3/codepage/) maps declared legacy code pages
to [encoding_rs](https://docs.rs/encoding_rs/0.8.42/encoding_rs/) decoders.
Unmapped pages preserve each byte as the same Unicode scalar. Compressed BIFF8
strings decode as UTF-16 units independently of the declared code page.

Legacy cells use the shared formatter, live format table and detached snapshots.
Declared blank bounds do not allocate blank cells. Binary formula expressions,
arrays, rich text and hyperlinks remain tracked in
[legacy workbook metadata](https://github.com/Orchestration-Maestro/maestro/issues/1175).
