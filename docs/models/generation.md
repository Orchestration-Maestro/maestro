# Catalog feed acquisition

The native-only `maestro_models::catalog_generation::generate_models` module
provides `fetch_open_router_models`, `fetch_ai_gateway_models` and
`load_models_dev_data`. After its initial progress line is written successfully,
each operation issues one GET through its supplied `Fetch`, with no supplied
authentication, timeout, cancellation signal or retry. A failed initial write
returns an error without fetching. HTTP status does not replace JSON processing.

These operations return typed model descriptors; they do not register models,
refresh the embedded catalog or write catalog files. Returned descriptors retain
feed encounter order and may contain duplicate identities. models.dev traverses
providers in authored order, with canonical integer entry keys before other
keys; Xiaomi variants expand in variant-major order. Final catalog-wide
overrides and deduplication are not applied here.

## Normalization and failures

Tool-capability filters select records before optional metadata is checked.
`OpenRouter` accepts string or array membership; Vercel tags must be arrays;
models.dev requires literal `true` for tool support and reasoning. Text input is
retained, with image input added by each feed's modality rule. The Cloudflare gateway strips
supported OpenAI/Anthropic upstream prefixes and retains the Workers AI prefix.
The two Kimi coding aliases normalize to the canonical ID and name, and defer
only to an eligible, well-formed canonical record. Other selected IDs retain
their spelling. `OpenRouter` requires a supplied string name; Vercel and models.dev
use a truthy supplied name or the selected ID, except the fixed Kimi alias name.

Gateway prices accept decimal prefixes and are scaled to USD per million
tokens; nonfinite parsing or scaling becomes zero. models.dev rates are already
per million. Missing or falsy limits use 4096, except the GitHub provider's
128000 context and 8192 output defaults. Finite nonzero negative and fractional
limits remain. Truthy limit and models.dev price leaves must be finite numbers;
gateway price leaves must be numbers or strings when not defaulted as falsy.
Non-null models.dev input modalities and `OpenRouter` architecture modalities
must be strings or arrays. Invalid values in these validated fields reject
selected models. Other fields can be defaulted or ignored: `cost: 42` supplies no
price leaves and defaults to zero, wrong-typed models.dev `reasoning` becomes
false, and an invalid SDK identifier uses the fallback route.

A source fetch, body or whole-feed decode failure returns an empty vector if
writing its diagnostic succeeds. A malformed selected record is skipped after
a diagnostic; remaining records proceed if writing that diagnostic succeeds.
Output and diagnostic writer failures return their `io::Error`.

Progress announces the source before fetching and prints the returned descriptor
count after body completion and normalization. Per-record diagnostics use the
source's failure prefix followed by `skipping model {id}: invalid metadata`.
String IDs are JSON-escaped; other supplied IDs retain their raw JSON spelling,
without recursive conversion. Missing array IDs use the JSON-escaped
`<entry:N>` with zero-based `N`. models.dev diagnostics use the entry key.

Body decoding removes one leading UTF-8 BOM and replaces invalid UTF-8 sequences.
Named-field lookup and typed record decoding skip names that are not valid UTF-8,
so unread lone-surrogate names do not hide representable fields. Full-value
conversion instead retains every surviving member or fails; see the
[whole-value conversion boundary](chat-completions.md#whole-value-conversion).
Object entry keys retain distinct UTF-16 units through duplicate resolution and
ordering, so lone-surrogate keys cannot overwrite different keys. In surviving
IDs and other decoded strings, each unpaired UTF-16 escape is spelled as one
U+FFFD replacement character because Rust strings cannot hold that unit; valid
pairs and literal escaped text remain intact. Unknown metadata is borrowed
without recursively converting, serializing or dropping it as a value tree.

## Controlled client

No live service is needed to call the interface:

```rust
use std::sync::Arc;
use maestro_models::{Fetch, HttpResponse};
use maestro_models::catalog_generation::generate_models::fetch_open_router_models;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let fetch: Fetch = Arc::new(|request| {
        assert_eq!(request.url, "https://openrouter.ai/api/v1/models");
        Box::pin(async {
            Ok(HttpResponse {
                status: 200,
                headers: std::collections::BTreeMap::new(),
                body: Box::pin(futures_util::stream::iter([
                    Ok(br#"{"data":[]}"#.to_vec()),
                ])),
            })
        })
    });
    let runtime = tokio::runtime::Builder::new_current_thread().build()?;
    let mut output = Vec::new();
    let mut errors = Vec::new();
    let models = runtime.block_on(fetch_open_router_models(
        &fetch, &mut output, &mut errors,
    ))?;
    assert!(models.is_empty());
    assert!(errors.is_empty());
    Ok(())
}
```
