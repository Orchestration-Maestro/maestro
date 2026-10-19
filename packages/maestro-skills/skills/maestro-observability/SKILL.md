---
name: maestro-observability
description: "Use when adding logs, metrics, traces or alerts, shipping production I/O, or missing telemetry prevents diagnosis."
---

# Instrument the questions operators need answered

## Define working

Before instrumentation, write two to four questions an on-call engineer needs answered. For an illustrative
retrying operation: how often does the first attempt succeed, why do attempts fail, and which dependency
is slow? These are questions, not observations about the current system. Every signal must answer one.

Use metrics for aggregate rate and latency, traces for where time went across boundaries, and structured
logs for a particular event. Current incidents go to `/skill:maestro-diagnose`; measured slowness goes to
`/skill:maestro-performance`.

## Structured logs

Emit JSON with stable event names and machine-readable allowlisted fields. Keep levels consistent:
`error` means an invariant broke and someone may act; `warn` means degraded but handled; `info` records a
significant event; `debug` is diagnostic detail normally off in production.

Generate or accept a correlation/request ID at the boundary. Attach it to logs, spans and outbound calls.
When scheduler, replay and manual entry points share a sink, stamp the actual entry point where the run
starts and propagate it with the ID through HTTP headers and queue metadata. A correlated field is not proof
of origin. Do not infer the entry point later from a process table or missing scheduler record.

Never log secrets, credentials or full personal data. Allowlist fields instead of serializing whole bodies
or authentication headers. External-call metadata can include endpoint, status, latency and attempt count
without request contents. Spot-check the emitted structure, not only logger calls in source.

## Metrics and traces

Use RED for every endpoint and external dependency: rate, errors and duration. Use USE for queues, pools
and hosts: utilization, saturation and errors. Track queue depth and processing duration. Store duration as
histograms and inspect p50/p95/p99; averages hide tail latency.

Keep label sets bounded: route templates, status classes and provider names. User/tenant/request IDs,
emails, raw URLs and error messages belong in protected logs or traces, never metric labels.

Use the project's tracing integration; OpenTelemetry is the vendor-neutral route. Initialize before instrumented
clients at startup, enable appropriate automatic HTTP/RPC/database spans and add manual spans only for
meaningful internal work. Propagate inbound/outbound context and queue metadata across asynchronous boundaries.
Use safe filterable attributes without secrets. Sample normally at a low head-based rate; retain all errors
where tail sampling supports it. Verify that one request remains connected across every service hop.

## Alerts and runbooks

Page on symptoms users feel: error rate, latency or queue age. CPU, disk and restarts usually belong on a
dashboard as possible causes. Every alert is actionable, has an SLO- or history-backed threshold and duration,
and links to a runbook. Use page for user-facing action now and ticket for degradation to address this week.
Delete alerts whose only response is to ignore them.

A runbook names what the alert means, the first query/check and the escalation owner. Store it under the
project's runbooks convention, commonly `docs/runbooks/`. Expand only when the first check cannot discriminate
the next action. Correct stale steps when closing the incident that exposed them.

Use the [checklist](checklist.md) to verify the telemetry itself before release. A dashboard containing many
charts is not evidence that it answers the original questions.
