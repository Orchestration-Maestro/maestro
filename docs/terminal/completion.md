# Completion

`CombinedAutocompleteProvider` suggests command names, command arguments, direct
file paths and recursive attachment paths. The editor supplies lines, a UTF-8 byte
cursor and a borrowed signal.
Callers await the suggestion future; the provider does not schedule editor updates.

```rust
use maestro_cancellation::Cancellation;
use maestro_tui::autocomplete::{
    Command, CompletionError, CompletionOptions, CursorPosition,
    NativeAutocompleteOperations,
};
use maestro_tui::{AutocompleteProvider, CombinedAutocompleteProvider, SlashCommand};

async fn complete() -> Result<(), CompletionError> {
    let command = SlashCommand {
        name: "help".into(),
        description: Some("Show help".into()),
        argument_hint: Some("<topic>".into()),
        get_argument_completions: None,
    };
    let provider = CombinedAutocompleteProvider::new(
        vec![Command::SlashCommand(command)],
        "/work".into(),
        Some("fd".into()),
        NativeAutocompleteOperations::default(),
    );
    let lines = vec!["/he".into()];
    let suggestions = provider.get_suggestions(
        &lines,
        CursorPosition { line: 0, col: 3 },
        CompletionOptions { signal: &Cancellation::new(), force: None },
    ).await?;
    if let Some(suggestions) = suggestions {
        let item = &suggestions.items[0];
        let completed = provider.apply_completion(
            &lines, CursorPosition { line: 0, col: 3 }, item, &suggestions.prefix,
        );
        assert_eq!(completed.lines, ["/help "]);
    }
    Ok(())
}
```

## Commands

Supply `Command::SlashCommand` for argument callbacks or
`Command::AutocompleteItem` for a name and description. Command labels use the name
or value, not a supplied display label. Nonempty hints precede descriptions with
` — ` between them. Command names use shared fuzzy ranking; the first ASCII space
selects arguments for the first exact command name.

An argument callback returns `Result<Option<Vec<AutocompleteItem>>, CompletionError>`.
Callback failures propagate unchanged; absent or empty callback results yield no
suggestions. Command completion does not suppress callbacks based on the signal.

## Paths and host operations

Attachment tokens take precedence over commands and forced direct completion.
Unforced leading slash queries select commands; forced queries bypass commands for
direct path completion.

`AutocompleteOperations` supplies home lookup, directory enumeration, link metadata,
label comparison, cancellation reads and search execution. Implement it for an
in-memory, remote or browser filesystem;
matching and candidate construction remain in the same provider. Native operations
are available only outside the browser target. Their locale reader can be replaced
with `NativeAutocompleteOperations::with_environment` independently of home lookup.

Direct completion filters names by lowercase prefix. Resolved directories precede
files, with stable host comparison within each class. Listing, home and comparison
failures yield no suggestions; failed link metadata leaves that entry file-like.
Candidates retain relative, `./`, home and absolute prefix styles; reconstructed
components follow the [path utility](../../crates/maestro-path/src/lib.rs).
Only Windows separators are normalized to slashes; literal Unix backslashes remain
filename characters. Paths containing ASCII spaces or an already quoted prefix
are quoted.

Applying a candidate replaces its prefix and returns a byte cursor. The suffix is
retained except for one leading closing quote when the prefix is quoted and the
candidate ends in a quote. Command insertion adds a slash and trailing space unless
the value, after removing its opening display quote if present, starts with a slash.
Attachment-file insertion adds a trailing space; directory
insertion keeps the cursor inside a closing quote when one is present.

## Recursive attachment paths

The caller supplies the optional search executable to the constructor. An absent or
empty executable disables attachment suggestions. Native operations use
`Cancellation` as their signal and run that executable directly, with null stdin
and captured stdout/stderr; stderr is consumed without display.

A directory before the last native separator scopes the search when metadata
identifies it as a directory. Failed metadata falls back to unscoped search; a
failed home lookup ends the attachment request. Display prefixes retain authored
spelling after native separators are converted for display. The search executable receives literal basename queries or escaped
full-path queries, includes hidden files and directories, follows links and applies
its ignore rules. It is asked for 100 results; NUL-delimited records preserve
filename whitespace and Unix backslashes, excluding exact `.git` components.

Nonempty queries rank by whole-string lowercase basename equality, basename
prefix, basename substring, then path substring; matching directories receive a
bonus. Empty queries keep process order. Stable score ties retain duplicates and
only the first 20 ranked candidates are returned. Attachment host failures and
observed cancellation return no suggestions. Native cancellation checks child
status, then kills and waits when the child remains running; status or kill
failures return I/O errors to the provider.

## Shared fuzzy ranking

`fuzzy_match` matches ordered Unicode scalars case-insensitively, with a letter/digit
swap fallback after a failed primary match. Lower matching scores rank first.
`fuzzy_filter` requires every whitespace-separated token to match, retains borrowed
original items in stable score order and skips projection for a whitespace-only query.
See [text helpers](text.md) for the shared whitespace policy.
