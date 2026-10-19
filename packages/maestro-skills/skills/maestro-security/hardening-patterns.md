# Hardening patterns

## Queries, output and resource access

- Pass query parameters separately from SQL or NoSQL syntax. Pass shell arguments as data rather than
  building an executable string. Input validation does not replace parameterization.
- Use the renderer's escaping for text. If authored HTML is necessary, sanitize with an allowlist before
  rendering it. Never put model output directly into HTML, a shell, a query or an executable evaluator.
- Authenticate every protected endpoint. Check ownership or the required role on every resource operation,
  including reads and administrative routes. Knowing an object's identifier grants no permission.
- Scope API keys to their necessary permissions. Validate token signatures, expiration and issuer.

## Passwords, sessions and CSRF

Hash passwords with argon2, scrypt or bcrypt (at least 12 rounds). Load the session secret from the
configured secret source, not code. Session cookies need `httpOnly`, `secure`, a bounded lifetime and
`sameSite` lax or strict. `sameSite=none` permits cross-site cookie delivery; do not call it CSRF protection.
Protect cookie-authenticated state changes against CSRF. CORS is not a substitute for that protection.
Reset tokens should be single-use and expire within an hour. Consider MFA for sensitive operations and
notification on account lockout. Limit authentication attempts more strictly than ordinary API calls;
the source guidance is about ten attempts per fifteen minutes. Multiple serving processes need a shared
counter store, or the effective limit multiplies by instance count and may vanish on ephemeral workers.

## Headers, CORS and response fields

Set CSP, HSTS, frame restrictions, `X-Content-Type-Options: nosniff`, referrer policy and permissions policy
through the stack's supported middleware. Begin CSP at `default-src 'self'` and tighten for actual needs.
Disable obsolete XSS filtering rather than relying on it instead of CSP. Restrict CORS to configured origins,
methods and headers; never combine wildcard origins with credentials.

Return allowlisted public fields, not a stored user record minus a guessed secret field. Exclude password
hashes and reset tokens. Return generic internal-error bodies; keep diagnostic details in protected server
logs without secrets or personal data. Encrypt regulated data and backups as required.

## Typed validation and uploads

Parse once at the system boundary into a validated typed value. Allowlist shape, enums, string lengths,
numeric ranges and formats. Use proper date, URL and email parsers. Give structured validation details
(the HTTP guidance uses 422), without reflecting sensitive values. Downstream code consumes the parsed
value, not the original input. Client checks improve usability but do not authorize server operations.

For uploads, allowlist MIME types, cap size and verify content or magic bytes where relevant. An extension
or client-provided MIME type alone does not prove content. Validate redirect destinations too.

## Server-side fetches

User-influenced webhooks, imports and image proxies can target internal services. Allowlist scheme and
host. Resolve every DNS record and refuse the URL if any address is private or reserved, including loopback,
link-local metadata addresses and IPv6 unique-local space. Forbid redirects that bypass the check.

A second DNS lookup during connection leaves a rebinding race. On high-risk routes, pin the resolved
address for the connection or use a filtering transport. A hostname check alone is insufficient.

## Destructive paths

Before deleting, moving or overwriting a derived target:

1. Resolve symlinks and compare the target against a resolved allowlisted root using path components.
2. Require at least one level below the root; the root itself is never a cleanup target.
3. Read ownership evidence before the operation and before teardown removes it. The expected owner comes
   from authenticated state. A marker writable by the attacker is self-attestation, not authorization;
   protect its integrity with restrictive ownership or authentication.

On refusal, record a redacted target and stop. Never fall back to a broader directory. Resolving then
operating by name races ancestor replacement; use descriptor-based, no-follow, beneath-root operations
or ensure that the hierarchy cannot change for the transaction.

## Secrets

Use environment or secret-store values. Commit only placeholder templates; ignore real environment and
key files. Inspect the staged diff with a redacting secret scanner. Report rule and location, never the
value. If a secret reaches a remote, revoke and rotate it first, then purge history; deleting the line
alone cannot recover confidentiality.

## Supply chain and audit triage

Find the installation boundary that owns the manifest and lockfile. A nested project is independent only
outside the parent workspace. Corroborate the manager version, lockfile and CI; stop on competing lockfiles
or disagreement. Use that pinned manager's documented frozen or immutable install and native audit.

Block dependency lifecycle scripts before the first install executes them. Inspect the exact pending
script and package version, approve the narrowest native policy, commit it, and verify with a clean locked
install. Check current official documentation for the pinned manager; do not assume its defaults fail closed
or blanket-approve builds. CI must preserve the authoritative lockfile rather than rewrite it.

Audit findings describe known advisories, not general trustworthiness:

| Finding | Disposition |
| --- | --- |
| Critical/high, reachable in runtime, build, tests or deployment | Fix immediately by update, patch or replacement |
| Critical/high, proven unreachable across those paths | Fix soon; document the reachability proof |
| No available fix | Evaluate mitigation or replacement; record any deferral and review date |
| Moderate, production-reachable | Fix in the next release cycle |
| Moderate development-only, or low | Track for regular dependency maintenance |

Preview remediation diffs and changelogs; never force upgrades automatically. Review ownership, maintenance,
release age, provenance, transitive dependencies and typosquatting. Verify registry signatures where supported;
an absent signature calls for investigation, not an automatic compromise claim.

## Privacy and AI features

Classify every new field: non-personal aggregates, personal identifiers or sensitive health/financial/location/
biometric/minor data. Collect only for a stated purpose. Record access policy, retention and deletion across
primary stores, backups, caches, indexes and analytics. Support required export, correction and erasure rights.
Consent gates collection and third-party sharing and must be auditable. Analytics, ads and model vendors
receiving personal data need the applicable processing agreement. Keep region a configurable policy.

For model features, parse output defensively, validate schemas and encode at the consuming boundary.
Enforce permissions in code; a system prompt cannot stop prompt injection. Keep secrets, other tenants'
data and confidential prompts out of context. Scope tools, validate arguments and confirm destructive actions.
Bound tokens, requests and loop depth. Partition retrieval embeddings per tenant and validate documents
before indexing. Vet models, datasets and plugins for integrity; ground critical claims in cited sources
and human review rather than treating generated assertions as fact.

## Review before release

Check authentication and authorization, rejected boundary inputs, headers and restricted CORS, secret-free
responses and logs, shared auth limiting, safe fetches and destructive paths, dependency/script policy,
and complete export/deletion. Map applicable OWASP risks to these controls: access, cryptography, injection,
design, configuration, vulnerable components, authentication, integrity, logging and SSRF. For AI, include
prompt injection, disclosure, supply chain, poisoning, output handling, excessive agency, prompt leakage,
embedding weaknesses, misinformation and unbounded consumption.
