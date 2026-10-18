# Public interface commitments

Every observable behaviour may become a dependency, including error text and ordering. Expose deliberately;
do not leak implementation details and assume callers will ignore them. Tests cannot enumerate every real dependency.
Keep one version of a contract where possible, extending it rather than forcing consumers into incompatible forks.

Define the contract first: inputs, outputs, ownership, invariants, ordering and failures. Use one consistent error
strategy so callers can predict what failure means. Validate user input, configuration and external responses at
the system boundary. Internal code sharing a validated contract need not repeat that parsing at every call.
Treat external responses as untrusted even when a service usually behaves well.

Prefer additive changes to removing fields or changing existing meanings. Use consistent domain names and
respect consumers of the current surface.

## Idempotent effects

Derive an idempotency key from one intent, not each retry. Distinct legitimate intents need distinct keys.
Claim it atomically before the effect; a separate lookup followed by insertion admits concurrent duplicates.
Retain the request identity and reject the same key with a different payload. Decide whether an in-flight
duplicate receives conflict, pending status or the settled result; never let it execute the effect again.

An external call can succeed, fail or have an unknown outcome. Record intent before calling out so a crash
leaves something to reconcile. Retain keys for the longest real retry/redelivery chain rather than an arbitrary
storage-saving interval. Replaying an old intent must not silently create a second effect.
