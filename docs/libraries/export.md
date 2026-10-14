# Session-document serialization libraries

The [serializer](../export.md) reuses the workspace's existing versions.
No Markdown parser, highlighter, viewer framework or runtime is added here.

| Library | Version | Direct features | License |
| --- | --- | --- | --- |
| serde (typed result details and fixtures) | 1.0.229 | defaults, derive | MIT OR Apache-2.0 |
| serde_json | 1.0.151 | defaults, preserve_order, float_roundtrip | MIT OR Apache-2.0 |
| base64 | 0.23.1 | defaults off, std | MIT OR Apache-2.0 |

Internal dependencies are the session, model-record, theme, JSON, text and path
owners, plus tools-owned definitions and toolkit components. Native file opening stays in the session owner, color/brand resolution
in the theme owner, canonical payload text in the JSON owner and whitespace
classification in the text owner. The supplied viewer owns its parser and
highlighting dependencies.
