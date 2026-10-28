# Custom message provider

Build with `--example custom_message_provider` using the command above. Loading
is opt-in and needs a component-enabled host with native HTTP. The example
registers the `custom-anthropic` provider with the `custom-anthropic-api`
protocol at `https://api.anthropic.com`, and two reasoning models that accept
text and images: `claude-opus-4-5` and `claude-sonnet-4-5`. Each has a
200,000-token context window and 64,000 output tokens. Select one with `/model`,
for example `custom-anthropic/claude-sonnet-4-5`.

To use an API key, set `CUSTOM_ANTHROPIC_API_KEY` before starting the host. To
use a subscription, run `/login custom-anthropic`, open the URL it shows, sign
in, then paste the returned `code#state`. A state that does not match the issued
proof key is rejected. Saved tokens count as expired five minutes early and
refresh through the same token endpoint.

Each request is one HTTP attempt with the conversation, tool schemas and the
requested thinking budget. Without a limit, a request asks for a third of the
model's output tokens. The response streams text, thinking and tool calls, then
usage and a cost from the model's prices. A missing key fails before any HTTP
request, and cancelling settles the message as aborted. A non-success status ends
the message with `HTTP <status>` and the response body, so the session's automatic
retry handles rate limits and server errors. The example does not retry on its
own.

Real logins and requests reach the provider and can incur charges. The
controlled `custom_message_provider_` runs use loopback HTTP and scripted login
input; they need no credentials or paid service.
