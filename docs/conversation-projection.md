# Conversation projection

`transform_messages` borrows conversation messages and a target model, returning
an independently owned request history. It does not edit stored history, tool
arguments or declarations. It returns no errors or logs; callback panics propagate
normally.

Replay identity includes provider, API and model ID. Compatible signed or redacted
thinking is retained; unsigned blank thinking is omitted. Across identities,
redacted and blank thinking is omitted, usable thinking becomes unsigned text,
and ordinary text loses its signature. Nonempty tool reasoning signatures are
removed across identities; explicitly empty tool signatures remain present.
Emitted text is never trimmed. Other response and result metadata is retained.

A target without image input receives `(image omitted: model does not support images)`
for user images and `(tool image omitted: model does not support images)` for tool
images. A newly inserted placeholder is suppressed when the preceding output
block already has that exact text. Ordinary text is never deduplicated.

The optional normalizer receives each cross-model call's original ID, the actual
target descriptor and the unchanged source assistant, in history/content order.
`None` disables ID rewriting only. All callbacks run before result synthesis,
even for assistant responses that are later omitted. Empty normalized IDs are
valid and the returned ID is used exactly as supplied.

Real tool results match the first unanswered original call occurrence in the
current assistant batch. Associations end at every user/assistant boundary;
equal normalized IDs do not merge occurrences. Unmatched and extra real results
are retained in their supplied order. Before each user/assistant boundary and
at history end, missing calls receive error results with their projected ID,
declared name, unsigned `No result provided` text, absent details and their own
current epoch-millisecond timestamp. Errored and aborted assistants are omitted,
after repairing the preceding batch, and contribute no missing results themselves.

## Controlled example

No provider registration, credentials or network calls are needed:

```rust
use maestro_models::{Message, Model, UserContent, transform_messages};
use serde_json::json;

let model: Model = serde_json::from_value(json!({
    "id": "controlled", "name": "Controlled model", "api": "fixture",
    "provider": "fixture", "baseUrl": "https://fixture.invalid",
    "reasoning": false, "input": ["text"], "contextWindow": 100,
    "maxTokens": 10,
    "cost": {"input": 0, "output": 0, "cacheRead": 0, "cacheWrite": 0}
}))?;
let message: Message = serde_json::from_value(json!({
    "role": "user", "content": [{"type": "image", "data": "AA==",
        "mimeType": "image/png"}], "timestamp": 1
}))?;
let stored = vec![message];
let request = transform_messages(&stored, &model, None);
let Message::User(user) = &request[0] else { unreachable!() };
let UserContent::Blocks(blocks) = &user.content else { unreachable!() };
assert_eq!(serde_json::to_value(blocks)?, json!([{
    "type": "text", "text": "(image omitted: model does not support images)"
}]));
assert_ne!(request, stored);
# Ok::<(), serde_json::Error>(())
```
