# Configured credentials

`maestro-credentials` resolves credential values and ordered headers, and formats
login guidance. Credential storage and refresh are separate, not delivered here.

Non-command values use a nonempty exact-name environment value or the unchanged
literal. Values beginning with `!` execute the remainder as a command. Successful
command results and absence are cached process-wide by the complete configured
string; `clear_config_value_cache` clears them. Uncached and throwing operations
bypass that cache without replacing it.

`resolve_headers` omits empty or unresolved values. `resolve_headers_or_throw`
retains successful empty strings and stops at the first resolution error. Both
preserve literal header names and input insertion order, including `__proto__`.

Callers supply `ConfigValueOperations` for environment and command effects.
Native callers can use `ProcessConfigValueOperations` with a lazy configured-shell
selector. Unix uses the default shell without reading that selector. Windows
tries the configured shell first and falls back after selection, infrastructure
or missing-executable failure, not after a completed unsuccessful attempt.
Native execution ignores stdin/stderr, captures stdout, inherits environment and
working directory, and applies a 10-second process/output timeout. Timeout cleanup
kills and awaits the owned direct child; it does not promise descendant cleanup.
Command stdout is decoded lossily as UTF-8 and trimmed at its outer boundaries;
literal and environment values are not trimmed.

Guidance formatters take a documentation root without reading files. Authored
path joining belongs to [maestro-path](../crates/maestro-path/README.md).
See the [crate example](../crates/maestro-credentials/README.md) for a controlled
adapter. Browser callers supply their own operations; the native adapter is absent
from browser builds.
