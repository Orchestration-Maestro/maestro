## Approved agent-loop libraries

`maestro-agent` inherits `futures-util =0.3.34` (defaults off, `std`, `sink`)
for driving admitted progress futures without spawning tasks. Native tests use
Tokio `=1.53.3` (defaults off, `rt`, `sync`, `macros`, `net`, `time`; the last two serve the
loopback conformance and scripted pacing tests). The product adds no runtime
or internal dependency edge. These libraries are MIT OR Apache-2.0 and MIT,
respectively. Awaited operations are documented in [conversation turns](../loop.md).
