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
`rustls =0.23.45` (defaults off, `std`, `tls12`, `aws_lc_rs`),
`rustls-platform-verifier =0.7.1` (defaults off) and `tower-service =0.3.3` (defaults off).
These are MIT or Apache-2.0 alternatives. Tokio enables `io-util` alongside its existing native
features. One TLS configuration supplies platform trust to reqwest and the SOCKS connector;
Hyper's HTTP/1 connection is driven by its request and response body, without detached work
or automatic replay. None of these declarations reaches the browser build.
