# Reasoning conversation requests

`MistralOptions` holds common request settings, tool choice, prompt mode and
reasoning effort. The options are available from `maestro_models` and
`maestro_models::providers::reasoning::mistral`; streaming entry points are not
available yet.

```rust
use maestro_models::{MistralOptions, MistralPromptMode, MistralReasoningEffort,
    MistralToolChoice};

let options = MistralOptions {
    tool_choice: Some(MistralToolChoice::Function { name: "lookup".into() }),
    prompt_mode: Some(MistralPromptMode::Reasoning),
    reasoning_effort: Some(MistralReasoningEffort::High),
    ..Default::default()
};
assert_eq!(options.prompt_mode, Some(MistralPromptMode::Reasoning));
```

The private request encoder selects recognized fields from object-shaped records,
adds their declared defaults and emits transport field names. Optional null is
retained only where the field admits it. Unknown record members are omitted after
decoding; dictionaries use the [shared JSON representation](https://github.com/Orchestration-Maestro/maestro/blob/main/docs/records.md).
Invalid selected fields return `Input validation failed: `
followed by the native decoder's explanation.

The encoder has no transport, retry or cancellation policy. See the
[shared model records](https://github.com/Orchestration-Maestro/maestro/blob/main/docs/records.md) for the owning JSON representation.

Private preparation projects history, selects model-specific simple reasoning
controls and encodes payload-hook replacements. Raw controls remain explicit.
Authored model and option headers merge with exact letter case before HTTP case
variants combine. This provider does not call the response hook.

Private sending makes one attempt. Without a caller cancellation signal, one
30-second deadline covers request setup and body reads; a supplied signal disables
that deadline. Admission requires status 200 with `text/event-stream`, allowing
case differences and parameters. A 422 JSON response is syntax-checked without
selecting error-record fields. Nonempty trimmed HTTP error bodies retain at most
4,000 Unicode scalars before a truncation suffix; native causes and empty-body
fallback messages are not subject to that limit.
