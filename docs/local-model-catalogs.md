# Model descriptors and catalogs

A `Model` is a caller-supplied descriptor containing an ID, display name, API,
provider, base URL, input kinds, reasoning data, limits, cost, headers and
compatibility data. Invocation does not require catalog membership or replace
caller fields with registered metadata.

`ApiProvider` registers raw and simple callbacks for an API. Replacement keeps
its insertion position; removal affects later lookups without invalidating
retained callbacks or active producer work. Registration is protocol dispatch,
not model selection or credential metadata lookup.

Catalog loading, model selection and overrides are separate capabilities, not
implemented by this invocation foundation. See [supplied model invocation](records.md).
