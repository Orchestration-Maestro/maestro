# Local date-time recipes

These assets contain one numeric date-plus-time profile. They derive from
Unicode CLDR JSON 48.2.0, under the Unicode-3.0 licence in `LICENSE`.

`index.tsv` maps locale IDs to zero-based rows in `recipes.tsv`. Each recipe
contains seven tab-separated fields: calendar, numbering system, hour cycle,
pattern, AM text, PM text, and era text. Empty trailing fields are significant.
Tokens are `%y` (year), `%M` (month), `%d` (day), `%H` (hour), `%m` (minute),
`%s` (second), `%p` (day period), and `%e` (era). Numeric tokens may have a
`2` before their letter for a two-digit minimum width. All other text is literal.
`hours.tsv` holds CLDR territory hour-cycle preferences. `calendars.tsv` lists
territories whose first calendar preference is not Gregorian.

## Regenerate the tables

Run the Rust development command from the repository root:

```sh
cargo run --locked -p maestro-tooling --bin development -- date-time-data
cargo run --locked -p maestro-tooling --bin development -- date-time-data --check
```

Both commands use only the committed `inputs/` directory. Generation requires
neither Node nor archive extraction nor network access. Check mode reports stale
assets without writing them. Every input is verified before generation writes
any asset; a missing file or SHA-256 mismatch stops the command.

`inputs/manifest.json` records the SHA-256 of each input, the five source archive
hashes, and the runtime versions. The Rust command pins the manifest's own hash,
so changing an input and its manifest together still fails verification. To
replace the recorded data deliberately, update the inputs, their manifest and
the manifest hash in `maestro-tooling::date_time_data` together.

## Recorded inputs

The recording verified five CLDR JSON 48.2.0 npm archives from
`https://registry.npmjs.org/<package>/-/<package>-48.2.0.tgz` against the hashes
in the manifest. Only `timeData.json`, `calendarPreferenceData.json` and the
licence are extracted from the core archive. `source-locales.json` records the
766 Gregorian locale directory names from the dates archive. The number,
Buddhist and Persian archives were verified; their contents are not needed by
the fixed-profile generator.

`oracle.json` records numeric patterns, widths, default calendars and final
literals selected once with Node 24.20.0, ICU 78.3 and runtime CLDR 48.0. The
archive and runtime CLDR versions are distinct. Each locale was observed in a
fresh process as its environment default, rather than as an explicit locale
argument. The recording reconciles nonbreaking spaces from `formatToParts`
only when `format` uses ordinary spaces, independently for morning and evening.
It preserves directional marks and requests no explicit calendar overrides,
which can add an era absent from the default output.

The index includes the 766 source locales and their US, FR, IR, and TH region
representatives, deduplicated to 2,301 rows and 444 recipes. These representatives
supply twelve-hour Gregorian, twenty-four-hour Gregorian, Persian, and Buddhist
preferences for parent fallback. They are not additional CLDR locales.

Persian conversion is translated from Unicode ICU's `persncal.cpp`, inspected at
SHA-256 `31e7e93bb7dc1e4436c58ffd44fd1ed2686ceb79228b8d989f6573ec15ed5f45`.
Its notice reads: © 2016 and later: Unicode, Inc. and others;
Copyright (C) 2003–2013, International Business Machines Corporation and others.
All Rights Reserved. The Unicode licence beside these assets covers the translation.

Ordinary builds, data generation and runtime formatting neither download archives
nor start Node.
