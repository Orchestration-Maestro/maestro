# Text

`maestro-text` is a core leaf below layer 0 with no workspace dependencies. It holds
four stateless decisions; each caller keeps its own trimming, any-or-all semantics,
stable sorting and comparator lifetime.

- **Whitespace.** `WHITESPACE` lists the 25 scalars once: Unicode white space except
  U+0085, plus U+FEFF. `is_whitespace_scalar` tests membership. HTTP edge
  whitespace and JSON token whitespace are different rules and stay with their owners.
  The model overflow pattern builds its character class from `WHITESPACE`.
- **Lexical order.** `utf16_order` compares UTF-16 code units, so supplementary
  scalars sort before U+E000.
- **Markup.** `markup::escape` and `markup::escape_into` replace `&`, `<`, `>` and `"`
  in one pass and never rescan emitted entities. `EscapeMode` picks the apostrophe
  spelling: `Xml` writes `&apos;`, `HtmlDecimal` and `HtmlHex` write numeric references.
- **Collation** (feature `collation`). `collation::Collator::new` reads the first
  present of `LC_ALL`, `LC_MESSAGES` and `LANG` through the supplied reader, empty
  values included, and none means `en_US`. Encoding and modifier suffixes are
  dropped, `C` and `POSIX` mean `en-US`, an empty name means `und`, and text that is
  not a locale selects the library default. The collator keeps the chosen locale,
  never the reader. `maestro_theme::Collator` re-exports it.

```rust
use maestro_text::{markup::{EscapeMode, escape}, is_whitespace_scalar, utf16_order};
use std::cmp::Ordering;

assert!(is_whitespace_scalar('\u{feff}') && !is_whitespace_scalar('\u{85}'));
assert_eq!(utf16_order("\u{1f600}", "\u{e000}"), Ordering::Less);
assert_eq!(escape("'", EscapeMode::HtmlDecimal).len(), 5);
```

Dependencies are listed in [text libraries](libraries/text.md).
