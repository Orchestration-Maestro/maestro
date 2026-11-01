# Browser chat example

The chat example is a complete browser-local chat built only from the published
web components. Its source is the `chat` example of `maestro-web`:
[`chat.rs`](../../crates/maestro-web/examples/chat.rs) starts it,
[`app.rs`](../../crates/maestro-web/examples/chat/app.rs) wires it,
[`title.rs`](../../crates/maestro-web/examples/chat/title.rs) edits titles and
[`custom_messages.rs`](../../crates/maestro-web/examples/chat/custom_messages.rs)
adds its own message type.

```sh
mise exec -- just browser-chat-example --port 9000
```

The recipe builds the example for `wasm32-unknown-unknown`, binds it with
`wasm-bindgen` into `target/maestro-browser/chat` next to its page, and serves that
directory on loopback with `miniserve`. Extra arguments go to `miniserve`.

## What it shows

- One IndexedDB database, `maestro-chat-example`, holds the settings,
  provider keys, custom providers, sessions and session summaries. It becomes the
  page's application storage.
- A `ChatPanel` shows the session. A provider without a stored key opens
  the key prompt, and a refusal keeps the draft. The session's tools are the
  artifacts tool and a JavaScript tool whose runtime providers come from the chat
  screen.
- Once a session holds a user message and a response, it gets a title,
  an identifier in the address (`?session=`) and a stored copy. The copy is
  refreshed after every completed message and keeps its first creation time. The
  title is the first sentence when its punctuation follows 1 to 50 characters.
  Otherwise, text up to 50 characters stays whole; longer text uses the first
  47 characters and an ellipsis.
- Sessions opens the stored list; choosing one loads it, and deleting
  the shown one starts a new session. New Session clears the address and starts
  afresh. The title is a button that turns into a field with its text selected:
  Enter or leaving the field keeps a new non-blank title, and Escape keeps the
  old one. The bell queues a demonstration notification for the next turn. The
  header's own appearance control stands in for the chat's, which the chat screen
  turns off. Settings opens the provider and proxy tabs.
Opening an address that names a stored session restores it. An
unknown session starts a new one and clears the address.

`SystemNotificationMessage` is a message type of the example's own. Its renderer
shows it as an alert with its local time, and its conversion hands it to the model
as a user message wrapped in `<system>` tags.

The example does not ask for persistent storage.

## Behaviour notes

The page never reloads. New Session and a missing session replace the address and
start a new session in place. Replacing the shown session aborts its active
request. Message completion preserves an open title draft; a pending title save
updates only its originating session. A restored session keeps the example's
instructions, and a stored summary sums the assistant usage.

## Tests

The browser exports in
[`chat_example_browser.rs`](../../crates/maestro-web/tests/chat_example_browser.rs)
compile the example's own modules into the browser test program. They drive the
example through its controls, with a recorded fetch and real browser storage.
