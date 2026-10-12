## Approved completion collation libraries

Outside browser targets, `maestro-tui` enables the `collation` feature of
`maestro-text` for native label comparison; [text libraries](text.md) names its ICU
packages. Their resolved closure includes
ICU collections, locale fallback, normalization, properties, provider and compiled
data (Unicode-3.0), plus `utf16_iter =1.0.5` and `write16 =1.0.0`
(Apache-2.0 OR MIT). Existing supporting packages retain their approved licenses
and the `unicode-ident =1.0.24` hold. No third-party type enters the completion API.
Authored path operations use the shared `maestro-path` utility. Browser callers
supply their own `AutocompleteOperations`, without native filesystem or ICU edges.
