# Package discovery libraries

`maestro-packages` uses the character-based ignore rules owned by
`maestro-pattern`. The ignore-rule source is MIT; its complete attribution is in
[third-party notices](../../THIRD-PARTY-NOTICES.md#package-ignore-rules).
No additional dependency or feature is required.

Source parsing and process dependencies are documented in
[package-source libraries](package-source.md); the shared matcher dependencies
and features in [file-pattern libraries](package-pattern.md).

Manifest expansion owns a synchronous walk over the shared compiled matcher and
existing resource filesystem operations. Its traversal derives from glob 13.0.6
and path-scurry 2.0.2 under BlueOak-1.0.0; attribution is in
[third-party notices](../../THIRD-PARTY-NOTICES.md#package-filesystem-expansion).
Neither is a runtime dependency. No asynchronous runtime or additional library
feature is introduced for expansion.
