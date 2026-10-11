## Approved completion collation libraries

Outside browser targets, `maestro-tui` uses `icu_collator =2.3.1` with default
`compiled_data` and `icu_locale_core =2.3.0` with defaults off and `alloc` for
native label comparison. Both use Unicode-3.0. Their resolved closure includes
ICU collections, locale fallback, normalization, properties, provider and compiled
data (Unicode-3.0), plus `utf16_iter =1.0.5` and `write16 =1.0.0`
(Apache-2.0 OR MIT). Existing supporting packages retain their approved licenses
and the `unicode-ident =1.0.24` hold. No third-party type enters the completion API.
Authored path operations use the shared `maestro-path` utility. Browser callers
supply their own `AutocompleteOperations`, without native filesystem or ICU edges.
