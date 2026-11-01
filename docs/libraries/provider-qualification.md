# Provider qualification libraries

Native message qualification in `maestro-models` uses the test-only dependency
`icu_collator =2.3.1` with default features, including `compiled_data`, under
Unicode-3.0. It supplies locale comparisons for source-owned provider probe
selection; it does not change catalog enumeration or runtime model routing.
The dependency version is already resolved elsewhere in the workspace.

The resolved collation closure contains 36 packages including the root, with
Unicode-3.0, MIT and Apache-2.0 license expressions. Existing request capture,
message framing, runtime driving and isolated test processes are reused;
no additional transport, parser, scheduler or normal dependency is added.

See [controlled conformance](../models/conformance.md#controlled-message-conformance)
for the exercised inputs and service-qualification limits.

## Cloud conversation libraries

Native Amazon Bedrock conversations in `maestro-models` use `aws-sdk-bedrockruntime =1.148.0`
(features `default-https-client` and `rt-tokio`, no legacy `rustls` stack) and `aws-config =1.12.0`
(features `behavior-version-latest`, `default-https-client`, `rt-tokio`, `credentials-process`,
`sso` and `credentials-login`) for credential lookup, signing, retries and event decoding, plus
`aws-sdk-signin =1.22.0` (default features off) only for the typed token-refresh error that
decides whether a failed login refresh ends the chain, plus `aws-smithy-runtime-api
=1.19.0` (`client`, `http-1x`) and `aws-smithy-types =1.8.1` (`http-body-1-x`) for the transport
bridge that sends through the shared HTTP seam, and `aws-credential-types =1.3.0` (default
features off) for the typed credential errors and futures that compose the selected-profile
sources. All six are Apache-2.0 and are declared only for non-browser targets; the browser
dependency set gains nothing. `aws-credential-types` was already resolved for `aws-config`, so it
adds no package to the lockfile.

Native connector tests enable `aws-smithy-runtime-api`'s `test-util` feature only through a
same-version dev-dependency, for its runtime-component builder.

The resolved lockfile gains 42 packages: 22 under Apache-2.0, 19 under MIT or Apache-2.0
expressions (or MIT alone) and `ryu =1.0.23` under Apache-2.0 OR BSL-1.0, used under
Apache-2.0. The AWS bridge uses the existing `hyper` byte and frame types; tests encode their own
event-stream frames.

The login source adds 24 packages to the lockfile. `aws-sdk-signin =1.22.0` is Apache-2.0. Under
MIT or Apache-2.0 (either alone or as alternatives) are `base16ct =0.2.0`, `base64ct =1.8.3`,
`const-oid =0.9.6`, `crypto-bigint =0.5.5`, `der =0.7.10`, `ecdsa =0.16.9`, `elliptic-curve
=0.13.8`, `ff =0.13.1`, `group =0.13.0`, `hmac =0.12.1`, `http =0.2.12`, `p256 =0.13.2`,
`pem-rfc7468 =0.7.0`, `pkcs8 =0.10.2`, `ppv-lite86 =0.2.21`, `primeorder =0.13.6`, `rand =0.8.8`,
`rand_chacha =0.3.1`, `rand_core =0.6.4`, `rfc6979 =0.4.0`, `sec1 =0.7.3`, `signature =2.2.0` and
`spki =0.7.3`; no direct dependency on them is declared.

The native SOCKS connector adds `tokio-socks =0.5.3` (MIT, defaults off, `tokio`), plus
`thiserror =1.0.69` and `thiserror-impl =1.0.69` (MIT OR Apache-2.0) transitively. It retains
reqwest `=0.13.5` with defaults and `stream`, without enabling reqwest's `socks` feature.
Native direct declarations reuse already-locked `hyper =1.11.1` (`client`, `server`, `http1`),
`hyper-util =0.1.21` (`tokio`), `hyper-rustls =0.27.10` (defaults off, `http1`),
`rustls =0.23.46` (defaults off, `std`, `tls12`, `aws_lc_rs`),
`rustls-platform-verifier =0.7.1` (defaults off) and `tower-service =0.3.3` (defaults off).
These are MIT or Apache-2.0 alternatives. Tokio enables `io-util` alongside its existing native
features. One TLS configuration supplies platform trust to reqwest and the SOCKS connector;
Hyper's HTTP/1 connection is driven by its request and response body, without detached work
or automatic replay. None of these declarations reaches the browser build.

For SOCKS4, the native library handles empty IDs through its anonymous operation and
nonempty IDs through 255 UTF-8 bytes. Its domain frames fit only when the ID and hostname
together occupy at most 503 bytes. Longer IDs, larger frames and locally resolved IPv6
addresses use the existing owned exchange; these adapter restrictions do not limit accepted
proxy URLs or truncate transmitted fields. No library fork is used.

## Proxy script libraries

Native proxy script routing uses `boa_engine =0.22.0` with defaults off and
`system-time-zone`, under MIT. A retained interpreter runs on a blocking worker;
DNS uses the supplied asynchronous runtime while that worker waits. Existing
`regress =0.12.0` handles shell patterns; existing `chrono =0.4.45` (defaults
off, `std`, MIT OR Apache-2.0) validates dated ranges. No source rewriting or additional HTTP client is used.

`if-addrs =0.15.0` (defaults off, `link-local`, MIT OR BSD-3-Clause, used under MIT)
reads native interface addresses only after the standard-library UDP route lookup
fails. The feature preserves link-local addresses rather than filtering them.
The dependency owns platform-specific system calls; the caller uses its safe API.
Linux interface order was exercised; macOS and Windows were inspected, not run.

`tracing =0.1.44` (defaults off, `std`, MIT) carries exact script alert messages to
the caller's subscriber. The interpreter retains that dispatch across its worker
boundary; routing adds no printing or logger option.

The lockfile adds 40 package identities for the interpreter and interface adapter,
all offering MIT; no existing identity is removed. The interface adapter accounts
for one new identity, `if-addrs`; its platform dependencies were already locked.
All three direct declarations are native-only.

FTP script sources add no FTP dependency: `maestro-models` serves the plain-FTP subset
its source loader uses, over the already-locked Tokio socket, clock and I/O facilities,
`regex =1.13.1` for DOS and Unix listing recognition and `chrono =0.4.45` for
machine-listing instants. That subset is connecting and greeting, logging in, selecting
binary and file-structure settings, preparing an extended or classic passive data
connection, reading the modification time, listing one directory and downloading one
file. Each connection keeps its own byte-activity window and one sender frames every
command, so a password never reaches a diagnostic. The implementation is native-only, so
the browser build gains nothing. The earlier client dependency removed `suppaftp`,
`lazy-regex =3.6.2` and `lazy-regex-proc_macros =3.6.2` from the lockfile and added no
package identity.

## Native credentials

Native application-default credentials in `maestro-models` use `google-cloud-auth =1.17.0` with
its default features (`default-idtoken-backend`, `default-rustls-provider`); `idtoken`,
`jsonwebtoken` and `gdch` stay off. It is Apache-2.0, declared only for non-browser targets,
and adds 34 packages to the lockfile. Its typed user, service-account, external-account and
impersonated builders handle the ordinary file forms and nested impersonation over a retained
source. Its discovery and metadata resolver are never called: it cannot honor the metadata
endpoint precedence, and it lacks external-account authorized-user and certificate-sourced
forms, which `maestro-models` implements over the shared HTTP sender with one demand-driven
token cache. The library has no HTTP-client injection, so its nested clients keep their own
transport. `maestro-path` is added as the native path-join utility edge.

## Fixed image fixture qualification

Native `maestro-models` tests use `image =0.24.9` with defaults off and only the
`png` feature to decode the fixed circle fixture. They check the encoded PNG
and all decoded RGBA bytes against independently recorded SHA-256 digests using
the existing `sha2 =0.11.0` dependency. Production materialization embeds the
original PNG and uses only `std`; it does not encode or rasterize images.

The PNG-only closure contains 17 already-resolved packages, each permitting MIT
or Apache-2.0 except Zlib-only `zlib-rs`. This native dev-dependency introduces no
new resolved package or browser dependency. See the
[fixture command](../models/generation.md#fixed-image-fixture).
