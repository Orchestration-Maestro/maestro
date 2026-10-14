# Browser document tool

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
