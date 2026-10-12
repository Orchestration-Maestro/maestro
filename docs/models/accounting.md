# Token totals and costs

A reported usage is the token counts a provider sends with a response. Each
protocol adapter turns it into one `Usage`; the catalog descriptor's prices turn
it into costs through [`calculate_cost`](catalog.md#controlled-catalog-example).

## Totals by protocol

- Chat completions reconstruct `total_tokens` as input plus output plus cache
  categories. The server's own total is not read. Cached tokens come from the
  prompt detail count when present (an explicit zero included), otherwise from
  the legacy hit count; cache reads and writes are subtracted from the prompt
  tokens, never below zero. Reasoning tokens are already part of the output.
  Top-level usage wins over a choice's usage, even when empty; a later report
  replaces earlier categories.
- Response protocols keep the supplied `total_tokens`. A zero or missing total
  stays zero. Cached tokens are subtracted from the input.
- Conversation streams keep a nonzero supplied total and otherwise use input
  plus output.
- Message streams sum their four categories. The start event sets all four; a
  later update replaces only the categories it carries, an explicit zero
  included.

## Cancellation

A cancelled invocation keeps the usage reported before cancellation and
reports zero for everything else. No count is estimated from the prompt or the
partial text. A provider that reports usage only in its final update therefore
leaves a cancelled invocation at zero.

## Qualification

The `usage_totals` and `usage_after_abort` tests drive the public `stream`
operation with controlled server bytes and compare the complete result,
including all five costs, for every delivered provider and account route. They
prove how reported counts are interpreted; they do not measure a remote
service's reporting schedule, cache hit rate or billing.
