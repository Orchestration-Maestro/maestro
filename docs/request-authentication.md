# Request authentication

Model invocation forwards a supplied `StreamOptions.api_key` unchanged. It does
not select credentials, check empty secrets, authorize secret-free endpoints,
overlay headers or call an auth resolver. Resolve the credential owner explicitly
before supplying an adapter's api key. See [provider credentials](credentials.md).

`AuthResolver`, `RequestAuth`, `SecretString`, `AuthStatus`, `TokenExchange` and
`TokenExchangeResult` remain the credential owner's records and interfaces.
`SecretString::expose()` is deliberate access; its Debug output is a fixed marker,
not memory zeroization. Source labels and credential names must be non-secret.
Configured status is metadata, not validation or a prediction of request success.

The credential owner supplies precedence, persistence, helper execution and
cancellation behavior. Token exchange is an explicit credential primitive, not
an invocation side effect. Adapters are responsible for keeping supplied secrets
out of generated content and diagnostics. Supplied option/header maps are
sensitive surfaces and must not be logged.
