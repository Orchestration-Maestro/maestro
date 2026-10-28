# Workspace daemon libraries

`maestro-ipc` uses `interprocess =2.4.2` on Unix with default features disabled and
its `tokio` feature enabled, for the daemon's asynchronous API socket. The feature adds
edges to the already-locked `tokio =1.53.3` and `futures-core =0.3.34`, not new
packages. It uses the Apache-2.0 licence option.
It also uses the already-locked `rustix =1.1.5` on Unix with default features
disabled and `std` enabled, under the MIT OR Apache-2.0 licence. `rustix` supplies
the named native not-a-socket error code that separates a non-socket entry from a
wrong-protocol connection failure; it adds no package identity or version, because
the shared native feature unifier already activates it.

The locked dependency closure is:

| Package | Version | Licence |
|---|---|---|
| interprocess | 2.4.2 | Apache-2.0 |
| bitflags | 2.13.2 | MIT OR Apache-2.0 |
| doctest-file | 1.1.1 | 0BSD |
| errno | 0.3.14 | MIT OR Apache-2.0 |
| libc | 0.2.190 | MIT OR Apache-2.0 |
| linux-raw-sys | 0.12.1 | Apache-2.0 WITH LLVM-exception OR Apache-2.0 OR MIT |
| recvmsg | 1.0.0 | 0BSD |
| widestring | 1.2.1 | MIT OR Apache-2.0 |
| windows-sys | 0.61.2 | MIT OR Apache-2.0 |
| windows-link | 0.2.1 | MIT |

`recvmsg`, `widestring`, `windows-sys` and `windows-link` are inactive Windows
branches of this closure. `linux-raw-sys` and `errno` are the two backends of
`rustix`, and a target builds one of them. Exact 0BSD permissions for
doctest-file and recvmsg are recorded in the shared-CI configuration. The leaf
adds no production dependency on another domain crate. Native tests use the
existing checked test-process owner; the native feature-unifier dependency remains
API-free.

See [workspace sockets](../workspace-daemon.md#sockets) for the public operations.

## Detection manifests

`maestro-detection` directly uses the existing locked `regex =1.13.1` (default
features), `toml =1.1.6` (default features), inherited `serde =1.0.229` with
`derive`, inherited `serde_json =1.0.151` with `preserve_order` and
`float_roundtrip`, and `tracing =0.1.44` with default features disabled and `std`
enabled. It reuses the lock crate for durable byte publication. No HTTP client,
async runtime or process-based fetcher is added.

The host-resolved external closure of these five direct dependencies is listed
below. Every package was already locked; this change adds no third-party package
identity or version. Native process recognition and its existing feature unifier
and test support retain their own closures.

| Package | Version | Licence |
|---|---|---|
| aho-corasick | 1.1.5 | Unlicense OR MIT |
| allocator-api2 | 0.2.21 | MIT OR Apache-2.0 |
| equivalent | 1.0.2 | Apache-2.0 OR MIT |
| foldhash | 0.2.0 | Zlib |
| hashbrown | 0.17.1 | MIT OR Apache-2.0 |
| indexmap | 2.14.2 | Apache-2.0 OR MIT |
| itoa | 1.0.18 | MIT OR Apache-2.0 |
| memchr | 2.8.3 | Unlicense OR MIT |
| once_cell | 1.21.4 | MIT OR Apache-2.0 |
| pin-project-lite | 0.2.17 | Apache-2.0 OR MIT |
| proc-macro2 | 1.0.107 | MIT OR Apache-2.0 |
| quote | 1.0.47 | MIT OR Apache-2.0 |
| regex | 1.13.1 | MIT OR Apache-2.0 |
| regex-automata | 0.4.18 | MIT OR Apache-2.0 |
| regex-syntax | 0.8.11 | MIT OR Apache-2.0 |
| serde | 1.0.229 | MIT OR Apache-2.0 |
| serde_core | 1.0.229 | MIT OR Apache-2.0 |
| serde_derive | 1.0.229 | MIT OR Apache-2.0 |
| serde_json | 1.0.151 | MIT OR Apache-2.0 |
| serde_spanned | 1.1.1 | MIT OR Apache-2.0 |
| syn | 3.0.6 | MIT OR Apache-2.0 |
| syn | 2.0.119 | MIT OR Apache-2.0 |
| toml | 1.1.6+spec-1.1.0 | MIT OR Apache-2.0 |
| toml_datetime | 1.1.1+spec-1.1.0 | MIT OR Apache-2.0 |
| toml_parser | 1.1.3+spec-1.1.0 | MIT OR Apache-2.0 |
| toml_writer | 1.1.2+spec-1.1.0 | MIT OR Apache-2.0 |
| tracing | 0.1.44 | MIT |
| tracing-attributes | 0.1.31 | MIT |
| tracing-core | 0.1.36 | MIT |
| unicode-ident | 1.0.24 | (MIT OR Apache-2.0) AND Unicode-3.0 |
| winnow | 1.0.4 | MIT |
| zmij | 1.0.23 | MIT |

## Workspace preference validation

`maestro-daemon` uses the already-locked `time =0.3.55` with default features and
`formatting` to parse strftime formats and check that a date/time value can render
them. Status datetime producers also use these formats.

Its host-resolved external closure reuses these locked packages:

| Package | Version | Licence |
|---|---|---|
| deranged | 0.5.8 | MIT OR Apache-2.0 |
| num-conv | 0.2.2 | MIT OR Apache-2.0 |
| powerfmt | 0.2.1 | MIT OR Apache-2.0 |
| serde_core | 1.0.229 | MIT OR Apache-2.0 |
| time | 0.3.55 | MIT OR Apache-2.0 |
| time-core | 0.1.9 | MIT OR Apache-2.0 |

## Workspace preference diagnostics

The daemon activates its declared settings dependency for global projection and
reuses `tracing =0.1.44` with defaults disabled and `std` for startup warnings.
Inherited `serde_json =1.0.151` (`preserve_order`, `float_roundtrip`) is now a normal
dependency. Unknown-key capture uses the existing Serde visitor interface over
borrowed JSON values; no parser or ignored-field library is added.

Production file logging uses approved `tracing-subscriber =0.3.23` with default
features and `env-filter`. Controlled warning tests reuse it for thread-scoped
capture. Logging promotes the already-locked subscriber to a normal dependency;
no new package identity is added.
The tests drive settings futures with the already-locked `tokio =1.53.3` runtime
feature. No production runtime is introduced by preference loading.

The subscriber adds these approved locked identities:

| Package | Version | Licence |
|---|---|---|
| lazy_static | 1.5.1 | MIT OR Apache-2.0 |
| matchers | 0.2.0 | MIT |
| nu-ansi-term | 0.50.3 | MIT |
| sharded-slab | 0.1.7 | MIT |
| thread_local | 1.1.10 | MIT OR Apache-2.0 |
| tracing-log | 0.2.0 | MIT |
| tracing-subscriber | 0.3.23 | MIT |
| valuable | 0.1.1 | MIT |

Other selected subscriber dependencies reuse the detection closure above, plus
`smallvec =1.16.2` (MIT OR Apache-2.0) and `log =0.4.34` (MIT OR Apache-2.0).

## Action path components

`maestro-daemon` reuses the already-locked `sha2 =0.11.0`, with default features
disabled, for the six-byte SHA-256 suffix on long action path components. This is
the same approved dependency used by the model and package owners; it adds no
package identity or version. SHA-2 is licensed under MIT OR Apache-2.0. The shared
native feature unifier is unchanged. Windows device-stem recognition reuses
`maestro-path::win32::is_reserved`; the daemon keeps no second reserved-name list.

## Pane text matching

`maestro-daemon` directly uses the existing locked `regex =1.13.1` with default
features for retained terminal text matching. It adds no new package identity.
The dependency uses the MIT OR Apache-2.0 licence.

## Local status datetime

`maestro-daemon` uses the already-locked `chrono =0.4.45` with default features
disabled and `std` and `clock` enabled. It reads local timezone information safely
inside the asynchronous daemon process, then supplies a calendar value to the
existing `time` formatter. Chrono uses the MIT OR Apache-2.0 licence. No new
package identity or version is added. Controlled deadline tests enable Tokio's
existing `test-util` feature; production command execution reuses the process
owner's captured-child stream and group termination operations.

## Multiplexer frame library

`maestro-multiplexer` uses `ratatui =0.30.0` with default features disabled and
`underline-color` enabled, under its MIT licence. It is the only crate that uses
the frame library. The multiplexer also uses the existing locked
`unicode-width =0.2.2` with default features disabled and `tracing =0.1.44` with
default features disabled and `std` enabled. Its fixture tests use the inherited
`serde` and `serde_json` entries.

The frame library adds these package identities to the native build; every other
package of its closure was already locked:

| Package | Version | Licence |
|---|---|---|
| castaway | 0.2.4 | MIT |
| compact_str | 0.9.1 | MIT |
| indoc | 2.0.8 | MIT OR Apache-2.0 |
| instability | 0.3.14 | MIT |
| kasuari | 0.4.13 | MIT OR Apache-2.0 |
| line-clipping | 0.3.8 | MIT OR Apache-2.0 |
| lru | 0.18.5 | MIT |
| ratatui | 0.30.0 | MIT |
| ratatui-core | 0.1.2 | MIT |
| ratatui-widgets | 0.3.2 | MIT |
| unicode-truncate | 2.0.1 | MIT OR Apache-2.0 |

The lock file also lists the frame library's optional terminal backends, which no
enabled feature builds.

See [multiplexer rendering](../multiplexer.md#rendering) for the public operations.
