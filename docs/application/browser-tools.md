# Browser executable tools

## Document extraction

`maestro_app::create_extract_document_tool` creates an independently configurable
`extract_document` declaration using a supplied `Fetch`, or the
[`default_fetch`](https://docs.rs/maestro-models/latest/maestro_models/fn.default_fetch.html) when passed `None`.
The returned `tool` is a shared mutable executable declaration that can be
registered directly. Clone its execution callback under the read guard, then
release the guard before invoking it.

`maestro_app::EXTRACT_DOCUMENT_TOOL` is the shared default instance: a native
`LazyLock`, or a browser-local `thread_local` entry. Factory instances have
separate proxy settings. Set the returned `cors_proxy_url` slot to an optional
prefix; it starts unset and is read when the direct download fails with a CORS
error. A nonempty prefix is concatenated with the URI-component-encoded authored
URL for one retry. Other download failures do not retry. Failed proxy downloads
and missing proxy configuration provide manual-attachment guidance.

Execution accepts `{ "url": "https://example.com/note.txt" }`. It checks existing
cancellation before trimming and validating the absolute URL, preserving the
trimmed request spelling. Downloads check non-success status first, declared
content length next, and actual bytes after collection. Both size checks reject
only values above 50 MiB; the diagnostic labels this limit `50MB`. Download
attempts use the supplied cancellation signal without a deadline; attachment
extraction is not given a subsequent cancellation check.

Filename selection excludes query and fragment before choosing the final slash
segment, without percent-decoding. Empty names become `document`; the exact
`https://arxiv.org/` prefix appends `.pdf`. The tool passes buffered bytes and this
name to [attachment loading](browser-attachments.md), not the response MIME.
Current loading supports text, images and PowerPoint; remaining document parsers are assigned to
[#747](https://github.com/Orchestration-Maestro/maestro/issues/747),
[#762](https://github.com/Orchestration-Maestro/maestro/issues/762),
[#769](https://github.com/Orchestration-Maestro/maestro/issues/769).
Images and attachments without extracted text fail this tool; whitespace-only
text is returned unchanged. Browser tool cards remain assigned to
[#791](https://github.com/Orchestration-Maestro/maestro/issues/791).

Results contain one text block and exactly four camelCase details:
`extractedText`, `format`, `fileName` and `size`. Attachment MIME determines the
format tag (`pdf`, `docx`, `xlsx`, `pptx`, or `unknown`); the name and byte length
come from the loaded attachment. There is no progress or termination request.


## JavaScript execution

[`create_javascript_repl_tool`](../../crates/maestro-app/src/browser/tools/javascript_repl.rs)
retains a supplied `SandboxHost`. Register its shared `tool` handle; the factory
wrapper need not remain alive. The host owns hidden creation, execution and
removal. Removal follows settlement and precedes return-value formatting, including
on host failures. The browser frontend supplies
[`BrowserSandboxHost` and its lazy default registration](../browser/sandbox.md#hidden-javascript-execution)
on the existing retained frame owner. The interface adds no hard interruption.

`runtime_providers_factory` starts as an empty successful factory and may be
unset. Its current callback runs separately for descriptions and executions,
outside the setting lock. Descriptions retain nonblank provider text in order.
The independent `sandbox_url_provider` starts absent and is forwarded as an owned,
fallible callback. Both settings remain live after registration. Callers release
setting/tool guards before invoking callbacks.

The tool requires string `title` and `code` arguments. Its factory runs before
empty-code and pre-abort checks. Whitespace-only code is accepted. A supplied
`SandboxValue` keeps caller-owned values outside JSON and controls their display;
the JSON implementation delegates to the canonical JSON writer. Output preserves
console order, adds the return value and file notices, then trims shared
whitespace. Failed sandbox results become tool errors and omit return/files.

Details always contain `files`, including an empty array. Downloads preserve
UTF-8 or binary bytes as padded base64 with byte lengths; only encoded details
replace empty filenames/MIME types with `file`/`application/octet-stream`.
Tool cards remain tracked in
[#791](https://github.com/Orchestration-Maestro/maestro/issues/791).

## Sandbox runtime providers

The application supplies console capture, read-only attachment helpers and
returned-file collection. The browser SDK re-exports `ConsoleRuntimeProvider`,
`ConsoleLog`, `AttachmentsRuntimeProvider`, `FileDownloadRuntimeProvider` and
`DownloadableFile`. Use `BrowserSandboxValue` as their value parameter for the
browser router; the default JSON parameter supports native controlled hosts.
The projection trait stays with the application contracts.

This example is also checked in the console provider's public rustdoc:

```rust
use maestro_app::browser::components::sandbox::{
    console_runtime_provider::ConsoleRuntimeProvider,
    providers::SandboxRuntimeProvider,
};

let console = ConsoleRuntimeProvider::<serde_json::Value>::new();
let earlier_logs = console.get_logs();
console.reset();
assert!(!std::sync::Arc::ptr_eq(&earlier_logs, &console.get_logs()));
assert!(console.get_data()?.is_empty());
# Ok::<(), maestro_models::DiagnosticErrorInfo>(())
```

Log and file getters return live shared collections, allowing callers to retain
and edit them. Reset replaces the collection: previous aliases keep their entries.
Collection and replies run when `handle_message` is called; its returned future
carries their result. Collection precedes acknowledgement, so a response failure
does not undo it; response callbacks run without internal guards. Host fields remain unformatted
values. JSON preserves missing versus null arguments; browser values retain
native handles, including binary data and undefined fields.

Console settlement messages now update its completion getters: success clears
an earlier error, failure records message/stack, and reset clears settlement.
Lifecycle messages receive no extra provider acknowledgement. Attachment input
is retained and read afresh for each data snapshot; projection selects only id,
filename, MIME, size, content and optional extracted text. Its model-facing
help is the shared attachments description. Online returned files are collected,
not downloaded by the host; the injected helper retains its standalone offline
browser-download branch.

Each provider returns a complete injectable function. Native tests check host
behavior and full source equivalence; browser builds check adapter signatures.
Real iframe events, native values and download effects remain qualified by
[browser smoke checks](https://github.com/Orchestration-Maestro/maestro/issues/825).
See [sandbox messaging](../browser/sandbox.md) for transport ownership.

## Measuring prompt tokens

The native developer example measures the read-write artifact, read-only artifact
and attachment runtime descriptions, followed by the JavaScript REPL and artifact
tool descriptions with empty runtime-provider lists:

```sh
CARGO_BUILD_JOBS=3 ~/.local/bin/capped cargo run -p maestro-app --locked --example count_prompt_tokens
```

Set `ANTHROPIC_API_KEY` in the environment through your usual secret-management
mechanism; do not put credentials in shell history. An absent or empty key fails
before making requests. **This opt-in command contacts a provider-authenticated
live service and may incur provider charges.** Tests use controlled responses
and a local HTTP server, never this live endpoint.

The example sends each complete description sequentially to
`https://api.anthropic.com/v1/messages/count_tokens` using model
`claude-3-5-sonnet-20241022`. It reports the returned numbers, not a local token
estimate, using the process locale for grouping and decimal presentation.
Individual transport, HTTP, body or decoding failures are attributed to their
prompt on standard error; later requests continue and only successful counts
contribute to the total. A report with five failed requests still completes
successfully with total zero. Provider credentials and model availability remain
requirements of the live service.
