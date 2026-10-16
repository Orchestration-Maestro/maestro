# Ordinary command metadata

`maestro_app::presentation_data::slash_commands::BUILTIN_SLASH_COMMANDS` contains
21 ordered built-in command names and descriptions. `BuiltinSlashCommand` owns its
strings. The quit description uses the configured application name. The table
supplies presentation data, not handlers, argument schemas or session enumeration.

`SlashCommandInfo` and `SlashCommandSource` are shared request records, re-exported
through resources, application presentation data, the application root and guest
authoring. Existing guest paths remain usable. Command source serializes as an open
string; `EXTENSION`, `PROMPT` and `SKILL` supply the default labels, while future
labels need no registry. Each record owns its name, optional description and
`SourceInfo`. An absent description is omitted and an empty description is retained.
Provenance uses the resource owner's existing path, source, scope, origin and optional
base directory fields. Guest command inventories use the existing JSON-string host
operation, without another wire format.
