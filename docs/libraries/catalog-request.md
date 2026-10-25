## Approved catalog request libraries

`maestro-catalog` uses `indexmap =2.14.2` with defaults and `serde` for ordered
request header sources. Native tests use Tokio `=1.53.3`, defaults off, `rt` and
`sync`. These reuse the resolved workspace packages; internal dependencies remain
models and credentials.
