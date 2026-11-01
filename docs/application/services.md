# Hosted services

`maestro_app::configuration` exposes caller-driven release checks, install
reporting and share links. Each accepts the same supplied hosted-service origin.
The compiled `config::SERVICE_ORIGIN` is empty: passing it produces no request
and no viewer link. A nonempty origin is concatenated literally with each path;
there is no URL normalization or separate viewer-host override.

## Release checks

Native `version_check::get_latest_maestro_release` performs one GET to
`/api/latest-version` with the native user agent and `accept: application/json`.
It selects a nonempty trimmed string `version` and optional trimmed string
`packageName`, ignoring other members and wrong-type fields. Duplicate members
use their last value. Non-success status returns no release without reading the
body; transport, body and JSON failures propagate. A null JSON root is an error;
other non-object roots return no release.

`get_latest_maestro_version` projects the version and preserves errors.
`check_for_new_maestro_version` catches failures and returns only a newer
candidate. Numeric version components compare by their first difference;
build metadata is ignored and whole prereleases use the prepared locale
comparator. Unparsed inputs fall back to unequal trimmed text.

The default deadline is 10,000 milliseconds for headers plus body together.
`VersionCheckOptions` can supply a different deadline (including zero) or the
shared HTTP transport override. Nonempty `MAESTRO_SKIP_VERSION_CHECK` or
`MAESTRO_OFFLINE` skips requests; telemetry preference does not affect checks.
Its `skip_version_check` and `offline` selections replace those two reads, and an
absent selection keeps reading the flag.

## Install reporting

`telemetry::is_install_telemetry_enabled` resolves the existing settings
preference, whose default is enabled. A supplied environment value overrides
it: only `1`, case-insensitive `true` or `yes` enables reporting. Empty or other
values disable it, without trimming.

Native `report_install_telemetry` captures settings and `MAESTRO_TELEMETRY` at
invocation. Nonempty `MAESTRO_OFFLINE` prevents a request;
`MAESTRO_SKIP_VERSION_CHECK` does not. Its `offline` argument replaces that read,
and `None` keeps reading the flag. The returned owned future retains no
borrow of the settings, version or origin and spawns nothing. Its caller owns
execution and trigger timing.

Awaiting enabled work performs one GET to `/api/report-install?version=...`.
The only authored data is the URI-component-encoded supplied version and native
user agent: no installation identifier or request body is added. Its deadline
is 5,000 milliseconds until response headers. It discards status and failures,
never reading the response body.

## Share links

`config::get_share_viewer_url` returns `<origin>/session/#<gist-id>` literally,
including an empty ID or punctuation in the fragment. It performs no request.
Model-request attribution is separate from this hosted-service origin.
