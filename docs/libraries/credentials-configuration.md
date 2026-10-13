## Approved credentials configuration libraries

`maestro-credentials` uses `indexmap =2.14.2` with defaults for ordered headers,
plus the existing `maestro-path` utility for authored documentation paths. Native
only Tokio `=1.53.2`, defaults off, enables `time` and `process` for native helper commands and asynchronous lock retries. Indexmap is MIT OR Apache-2.0; Tokio is MIT.
Browser callers supply configuration operations without a native process edge.
Stored credentials add `serde =1.0.229` (`derive`) and the workspace `serde_json`
for records, plus the `maestro-models` edge for environment keys and OAuth records.
