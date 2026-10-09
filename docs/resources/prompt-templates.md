# Prompt templates

`maestro_resources::prompt_templates` loads supplied Markdown templates and expands
leading slash commands. It does not execute a shell or select application settings.

```rust
use maestro_resources::{PromptTemplate, SyntheticSourceOptions,
    create_synthetic_source_info, expand_prompt_template};

let templates = vec![PromptTemplate {
    name: "review".into(),
    description: "Review selected paths".into(),
    argument_hint: Some("<paths>".into()),
    content: "Review $1; remaining: ${@:2}; all: $@".into(),
    file_path: "/prompts/review.md".into(),
    source_info: create_synthetic_source_info("/prompts/review.md".into(),
        SyntheticSourceOptions {
            source: "local".into(), scope: None, origin: None,
            base_dir: Some("/prompts".into()),
        }),
}];
assert_eq!(expand_prompt_template("/review src tests", &templates),
    "Review src; remaining: tests; all: src tests");
```

Expansion selects the first exact name after a leading `/`, splitting the name at
an ASCII space. Argument parsing removes single/double quote delimiters; only
unquoted spaces and tabs separate arguments. Empty quoted fragments disappear and
backslashes remain literal. Original `$1`, `$2`, `$ARGUMENTS`, `$@`, `${@:N}` and
`${@:N:L}` markers select arguments; inserted text is never rescanned. Missing
positional arguments insert empty text. Slice start zero selects from the first
argument. Selected arguments join with one space, preserving their own whitespace.
Unknown commands remain unchanged.

`load_prompt_templates` accepts caller-selected roots, home spelling and ordered
explicit paths through `LoadPromptTemplatesOptions` and `ResourceOperations`.
With defaults enabled, it scans user then project directories before explicit
paths. Disabling defaults leaves explicit paths active. Directory scans are
shallow, preserve adapter order and accept exact `.md` suffixes, including hidden
files and file symlinks. Repeated paths and names remain repeated. Paths use
[the lexical path owner](../../crates/maestro-path/src/lib.rs); provenance uses authored spelling, not link targets.

Metadata uses the shared [frontmatter parser](../../crates/maestro-resources/src/frontmatter/mod.rs). Only `description`
and `argument-hint` are selected: absent/null fields are absent, strings retain
text, and other selected types silently omit the file. Empty descriptions fall
back to the first nonblank body line, retaining its whitespace and at most 60
Unicode scalars, followed by `...` only when longer. Nonempty authored descriptions
are not truncated. Empty hints are omitted.

File read/metadata parsing/selected-field failures omit only that file. A link
metadata failure skips that link; directory-read or source-classification failure
ends only that scan, retaining earlier records. Explicit-path metadata/source
failures skip that path. Project-root and explicit-path resolution errors outside
those catches propagate when the process directory is needed and unavailable.
