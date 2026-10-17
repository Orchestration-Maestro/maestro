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
it uses its own number-format evaluator, not the parser's display text. The local
[calamine patch](../../vendor/calamine/src/xls.rs) retains ordered raw FORMAT
records, resolved per-cell format selections and exclusive declared dimensions.
It reuses the existing binary, codepage and scalar decoders with no new dependency
or optional feature. Legacy cells use the same shared formatter, live format table
and detached snapshots as application callers. Declared blank bounds do not
allocate blank cells. Binary formula expressions, arrays, rich text and hyperlinks
remain tracked in
[legacy workbook metadata](https://github.com/Orchestration-Maestro/maestro/issues/1175).

The parser's dependency closure uses MIT or Apache-2.0 where offered, with
BSD-3-Clause for encoding_rs, Unicode-3.0 for unicode-ident and Zlib for zlib-rs.
Its ZIP and XML dependencies are implementation details; no additional direct
ZIP/XML dependency or optional date feature is requested.

## Local parser patch

The published calamine 0.36.1 source is retained in
[vendor/calamine](../../vendor/calamine), outside workspace membership, under
its [MIT licence](../../vendor/calamine/LICENSE-MIT.md). The published archive's
SHA-256 is `5fa68281b1a76b54a62156474adb06bb380a67e07dd60656e3217152b42183f3`.
The root crates-io patch pins the ZIP requirement to `=9.0.3`
(defaults off, `deflate`) and propagates fallible archive names through the
existing path cache and XLSX/XLSB constructors, including its unit-test setup.
No name is skipped or decoded with a fallback. The same copy also retains raw
legacy FORMAT strings, resolved cell-format selections and exclusive-end declared
dimensions in the existing XLS parser. Older FORMAT strings reuse its codepage
decoder. The original manifest and source revision metadata remain alongside it.
