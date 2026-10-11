## Approved resource parsing libraries

The resources crate uses `yaml-rust2 =0.13.0` with defaults off for native
YAML parsing events, pinned to the patched copy in `vendor/yaml-rust2` until
[#381](https://github.com/Orchestration-Maestro/maestro/issues/381) brings the
correction into an upstream release, and `ryu-js =1.0.3` with defaults for numeric scalar-key
spelling. Their selected license branches are MIT OR Apache-2.0 and
Apache-2.0, respectively. Discovery uses the shared `maestro-pattern` ignore rules; its dependencies
are documented in [file-pattern libraries](package-pattern.md). Authored path operations use the shared `maestro-path`
utility. Radix scalar resolution uses `num-bigint =0.4.8` and
`num-traits =0.2.19` with defaults (MIT OR Apache-2.0) for arbitrary-width integers
and correctly rounded floating-point conversion. Resources may depend internally
only on the shared request-record, path and file-pattern owners.
