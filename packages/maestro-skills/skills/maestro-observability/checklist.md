# Telemetry acceptance checklist

## Signals and dashboards

- Each on-call question maps to a log, metric or trace, with structured events and correlation IDs.
- Shared log sinks carry entry-point identity set at the producer and propagated with the request ID.
- Actual output contains allowlisted fields, not interpolated object text, credentials or personal data.
- Every new endpoint/dependency has RED; resources have USE and queue depth/processing duration.
- Latency histograms expose p50/p95/p99. Label values are bounded and status codes grouped by class.
- Context crosses inbound/outbound requests and queues; one request has no disconnected span segments.
- Meaningful manual spans carry safe attributes. Sampling and error retention are deliberate.
- Service dashboards show error rate, tail latency, traffic and saturation, plus dependency health.
  Their default range suits an incident, such as one to six hours, rather than hiding it in a month.

## Alerts

Each new alert detects a user symptom, has a justified threshold and duration, reaches an accountable owner
and links to a current runbook with meaning, first check and escalation. Distinguish a page from a ticket.
Daily ignored alerts need repair or removal, not more severities.

## Exercise the consuming route

1. Force a controlled error in staging and locate it by correlation ID without opening the source.
2. Send test traffic; observe the real metric series, bounded labels and sensible values.
3. Follow one request across services and queues in the tracing view; confirm continuity.
4. Test-fire every new alert, temporarily lowering its threshold if appropriate. Restore it, verify delivery
   to the right channel and open the runbook link.

Before launch, confirm logs reach the aggregator, RED panels are visible, at least one applicable symptom
alert was test-fired, traces cross all touched services and on-call knows the runbooks. Record actual evidence
and gaps. Source declarations alone do not prove exported telemetry arrives at its consumer.
