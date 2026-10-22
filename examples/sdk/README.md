# SDK Examples

Programmatic sessions through Maestro's embedding interface. An example builds a
session, supplies the model, prompt, tools and extensions it wants, and drives
the conversation.

## Examples

The executable examples arrive with the embedding work: #227 ports the thirteen
reference examples, each registered as a Cargo example target of the crate that
owns it. This page gains a row for each one as it lands.

## Running

An example is built and run through its owning crate:

```sh
cargo run -p <crate> --example <name>
```

The remaining examples of the reference set arrive one ticket at a time; the
tracking issue is #2074.
