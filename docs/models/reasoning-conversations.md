# Reasoning conversation request encoding

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
decoding; dictionaries use the [shared JSON representation](../records.md).
The decoder's private `$serde_json::private::RawValue` sentinel is not supported as
literal object data in dictionary values or thinking parts, including unknown
thinking-part members. Invalid selected fields return `Input validation failed: `
followed by the native decoder's explanation.

The encoder has no transport, retry or cancellation policy. See the
[shared model records](../records.md) for the owning JSON representation.
