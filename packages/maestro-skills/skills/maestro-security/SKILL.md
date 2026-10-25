---
name: maestro-security
description: "Use when writing secure code or changing code that handles untrusted input, authentication, authorization, secrets, sessions or exposed data."
---

# Write secure code

## Threat model first

Read [untrusted input](../../references/untrusted-input.md). Trust follows who wrote a value, not how it
arrived. Model output, another agent's text, fetched pages, filenames and other processes' command lines
are untrusted just like requests, uploads, webhooks and third-party responses.

1. Name the assets: credentials, personal data, money, administrative actions and availability.
2. Map each trust boundary and the actor who can write the data crossing it.
3. Run STRIDE at each boundary:

| Threat | Question | Control |
| --- | --- | --- |
| Spoofing | Can someone impersonate a user or service? | Authentication and verified signatures |
| Tampering | Can data change in transit or storage? | Integrity checks, parameterization and HTTPS |
| Repudiation | Can an actor deny an action? | Security-event audit trail |
| Information disclosure | Can private data escape? | Encryption, field allowlists and generic errors |
| Denial of service | Can work or storage be exhausted? | Input bounds, rate limits and appropriate deadlines |
| Elevation of privilege | Can an actor gain unintended rights? | Resource authorization and least privilege |

4. Write abuse cases alongside use cases and test them before adding controls.

This skill covers writing secure code. A full security audit belongs to `security-audit`.

## Always, ask first, never

Always validate external inputs at the boundary, parameterize queries, encode output, protect external
transport with HTTPS, hash passwords with a password-hashing algorithm, set security headers and use
`httpOnly`, `secure`, `sameSite` session cookies. Audit the committed dependency lockfile before release.

Ask the human before changing authentication, storing a new category of sensitive data, adding an external
integration, changing CORS, adding uploads, altering rate limits or granting elevated privileges.

Never commit or log secrets, trust client validation as a boundary, disable security headers for convenience,
execute untrusted text, put auth tokens in client-accessible storage or expose stack traces and internals.

## Apply the relevant controls

Use [hardening patterns](hardening-patterns.md) for the implementation and review checklist. Select the
sections for the boundaries you actually touch, including local destructive paths and server-side fetches.
Authentication identifies the caller; authorization decides whether that caller may access this resource.
Protect both, and keep CSRF defenses distinct from CORS and authentication.

Before sign-off, exercise invalid and unauthorized inputs, check real response headers and error bodies,
inspect logs for data leaks and verify the native dependency audit. For data stores, verify export and
deletion through every copy. Record exceptions with a reason and review date, not an unexplained waiver.
