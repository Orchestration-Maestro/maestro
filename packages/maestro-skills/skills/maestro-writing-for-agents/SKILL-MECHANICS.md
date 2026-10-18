# Package a discoverable skill

Use one `SKILL.md` in a directory matching its frontmatter `name`. Descriptions start
`Use when`, name triggering situations rather than summarize a workflow, and stay
under 500 characters where possible. The loader's description limit is 1,024 characters.
Use ordinary relative Markdown links for reference files and concrete delivered skill
pointers such as `/skill:maestro-documentation`.

## Invocation choice

A model-invoked skill has a discoverable description and remains available to users.
Omit `disable-model-invocation` when the model or another skill must reach it. The
description spends always-loaded context in exchange for independent discovery.

A type-only skill sets `disable-model-invocation: true`. Keep a human-facing trigger
description, but do not assume the model sees it or can invoke it. A human must request
it. Shared text needed by type-only callers belongs in plain reference files, which
any caller can read, rather than hidden inside another type-only skill.

Create an invocation split only for a genuinely distinct trigger or independent caller.
Use [writing guidance](SKILL.md) for sequence splits. A router reduces the human's
lookup burden by naming skills and their triggers. It suggests type-only skills;
it cannot auto-run them on the user's behalf. Keep host tool/plugin files out of
content packaging unless the actual host requires them.
