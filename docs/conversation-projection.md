# Conversation projection

`transform_messages` borrows conversation messages and a target model, and returns
an independently owned request history. It does not edit stored history, tool
arguments or declarations. It returns no errors and writes no logs; a panic in a
callback propagates normally.

Replay identity is the provider, the API and the model ID together. When the
identity matches, signed or redacted thinking is kept and unsigned blank thinking
is dropped. When it differs, redacted and blank thinking are dropped, usable
thinking becomes unsigned text, and ordinary text loses its signature. A nonempty
tool reasoning signature is removed across identities; an explicitly empty one
stays present. Emitted text is never trimmed. All other response and result
metadata is kept.

A target without image input receives `(image omitted: model does not support images)`
in place of user images and `(tool image omitted: model does not support images)`
in place of tool images. Maestro skips a newly inserted placeholder when the
preceding output block already holds that exact text. Ordinary text is never
deduplicated.

The optional normalizer receives, for each cross-model call, the original ID, the
target descriptor and the unchanged source assistant, in history and content
order. `None` disables ID rewriting only. All callbacks run before result
synthesis, even for assistant responses that are later omitted. An empty
normalized ID is valid, and Maestro uses the returned ID exactly as supplied.

A real tool result matches the first unanswered original call occurrence in the
current assistant batch. Associations end at every user/assistant boundary, and
equal normalized IDs do not merge occurrences. Unmatched and extra real results
keep their supplied order. Before each user/assistant boundary and at the end of
the history, each missing call receives an error result with its projected ID, its
declared name, the unsigned text `No result provided`, absent details and its own
current epoch-millisecond timestamp. Errored and aborted assistants are dropped
after the preceding batch is repaired, and they add no missing results themselves.

## Controlled example

This example needs no provider registration, credentials or network calls:

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
