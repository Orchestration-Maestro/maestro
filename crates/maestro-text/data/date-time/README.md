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

The generator verifies SHA-256 hashes for five CLDR 48.2.0 npm archives.
Download each from
`https://registry.npmjs.org/<package>/-/<package>-48.2.0.tgz`, using the package
names and hashes in `tools/generate-date-time.py`. Then run:

```sh
python3 tools/generate-date-time.py --archives /path/to/archives
python3 tools/generate-date-time.py --archives /path/to/archives --check
```

Generation requires Node 24.20.0 with ICU 78.3 and CLDR 48.0. The package version
and runtime CLDR version are recorded separately: the archives supply the locale
domain and preferences; the runtime selects numeric patterns, widths, default
calendars, and final literals. Each locale is observed in a fresh process as its
environment default, not as an explicit locale argument. The generator reconciles
nonbreaking spaces from `formatToParts` only when `format` uses ordinary spaces.
Morning and evening parts are reconciled independently. It retains directional marks. It does not request explicit calendar overrides,
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

Ordinary builds and runtime formatting neither download archives nor start Node.
