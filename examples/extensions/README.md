# Extension Examples

Example extensions for Maestro. None ship yet; this page gains a row for each one
when it lands.

## Usage

An extension crate is a `cdylib` whose root names its factory. The
[extension guide](../../docs/extensions/guest.md) describes the interface it
registers, and `just extension-author-component` builds the author component
example in `crates/maestro-extensions-wasm/examples/author_component.rs`.

Maestro discovers extensions in two directories, in this order:

1. `<cwd>/.maestro/extensions`
2. `<agent_dir>/extensions`

[Extension loading](../../docs/extensions/loading.md) describes the remaining
steps.

## Examples

No example ships yet; one arrives with a row here and an entry in
[the examples tracking issue](https://github.com/Orchestration-Maestro/maestro/issues/2074).
