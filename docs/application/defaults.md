# Default resource routes

Select the user root with the [application configuration](configuration.md)
queries, then pass that root to the resource owners. Settings, credentials,
custom models, user prompts and default session directories consume paths beneath
that supplied root; they do not independently select an environment override.
The managed binary directory belongs to the user root, not the package directory.

An explicitly supplied empty root is present: suffix paths such as `settings.json`
are relative to the working directory. It is not the native selector's empty
**environment override**, which falls back to the home-based default. Default
prompt discovery skips the user scan for an empty supplied root. Explicit roots
are not passed through environment-root expansion again.

See the owning documentation for [settings](../settings.md),
[credentials](../credentials.md), [model files](../catalog.md),
[prompt templates](../resources/prompt-templates.md) and
[sessions](../sessions/session_format.md).

The application selects theme directories and loads resources through
[application resources](resources.md).
