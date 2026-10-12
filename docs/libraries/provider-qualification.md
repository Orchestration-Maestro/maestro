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
(features `behavior-version-latest`, `default-https-client`, `rt-tokio`, `credentials-process` and
`sso`) for credential lookup, signing, retries and event decoding, plus `aws-credential-types =1.3.0` (the credential error and future types the selected-profile
wrapper returns), `aws-smithy-runtime-api
=1.19.0` (`client`, `http-1x`) and `aws-smithy-types =1.8.1` (`http-body-1-x`) for the transport
bridge that sends through the shared HTTP seam. All five are Apache-2.0 and are declared only for
non-browser targets; the browser dependency set gains nothing.

The resolved lockfile gains 42 packages: 22 under Apache-2.0, 19 under MIT or Apache-2.0
expressions (or MIT alone) and `ryu =1.0.23` under Apache-2.0 OR BSL-1.0, used under
Apache-2.0. No unapproved crate beyond these AWS packages is added: the byte and frame types
come from the existing `hyper` dependency, and tests encode their own event-stream frames.
