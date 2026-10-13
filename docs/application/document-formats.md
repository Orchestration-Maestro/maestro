# Spreadsheet value primitives

The application library exposes `maestro_app::browser::ssf` for scalar spreadsheet
formatting. These operations have no I/O and do not render a workbook preview.

- `init_table` overwrites built-in format IDs in the supplied table, retains other
  entries and returns that table. `default_map` and `default_str` independently
  look up defined fallback IDs and currency/accounting patterns; holes return `None`.
- `general` accepts borrowed text, booleans, numbers and empty values. Text passes
  through unchanged; booleans become `TRUE` or `FALSE`; empty values produce empty
  text. Numbers use General's width, significant precision and exponent rules.
  NaN produces `#NUM!`; either infinity produces `#DIV/0!`.
- `frac` returns `[whole, numerator, denominator]` using bounded convergents.
  Format writers supply positive denominator limits. Mixed negative values use
  floor division rather than truncation toward zero.
- `parse_date_code` accepts a numeric serial and the effective 1904-epoch flag.
  Nonfinite values and serials outside 0 through 2,958,465 return `None` before
  normalization or epoch selection. The returned `DateCode` retains the normalized
  serial day before the epoch offset, integral seconds after carry, subsecond
  fraction and calendar components. Midnight carry increments the serial day.
  The standard epoch retains day zero and the synthetic February 29, 1900.
  Calendar conversion uses date-only arithmetic, not the host timezone.

The [numeric/calendar library inventory](../libraries/spreadsheet-formats.md)
documents the underlying primitives. Custom format evaluation, mutable format
loading, date-valued General input and alternate calendars are not exposed by
this scalar interface.
