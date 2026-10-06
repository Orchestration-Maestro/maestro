# Model invocation

`maestro-models` accepts caller-supplied descriptors and registers `ApiProvider`
callbacks by `Model.api`. The provider and model identifiers are supplied data,
not catalog admission keys. See [supplied model invocation](records.md) for the
controlled example and shared stream lifetime contract.

Raw and simple callbacks receive their options unchanged. Invocation does not
resolve credentials, project context, overlay headers, normalize content or
calculate usage. Adapters supply complete records, usage and terminal events.
An unknown API or failed setup is an invocation error; an error event settles
result observation to its supplied assistant message.

The leaf crate has no internal workspace dependencies. Its producer-owned FIFO
can be iterated independently of its result. Event records retain shared message
and tool-call handles, so earlier handles observe later mutations. Abandoning an
observation does not stop the producer.

[Conversation projection](conversation-projection.md) and argument validation
remain explicit pure helpers, not implicit invocation policy. Credential ownership
is described in [provider credentials](credentials.md).
