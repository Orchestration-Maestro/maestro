# How to document in Maestro

Maestro's documentation has two readers: the person using or extending
Maestro, and the agent itself, which reads the same README, guide pages and
examples when a user asks about Maestro. A page that is accurate, complete and
easy to follow serves both. This guide explains where each kind of
documentation lives, how to structure it and how to keep it true. Voice and
naming are set by [the identity guide](identity.md). The rules that the build
enforces are in [AGENTS.md](../AGENTS.md#documentation).

## Contents

1. [Where documentation lives](#where-documentation-lives)
2. [Write a guide page](#write-a-guide-page)
3. [Keep code blocks tested](#keep-code-blocks-tested)
4. [Add an example](#add-an-example)
5. [Write a specification](#write-a-specification)
6. [Record terms and decisions](#record-terms-and-decisions)
7. [Write rustdoc and comments](#write-rustdoc-and-comments)
8. [Choose the words](#choose-the-words)
9. [Check a change](#check-a-change)

## Where documentation lives

| Kind | Location | Written for |
| --- | --- | --- |
| Product entry | `README.md` | A first-time reader: what Maestro is, how to build it, where to read next. |
| Guide page | `docs/<topic>.md`, with deeper pages in `docs/<area>/` | People using or extending Maestro, and the agent answering their questions. |
| Example | `examples/extensions/` and `examples/sdk/`, each with a `README.md` index | Authors copying a working pattern. |
| Specification | `docs/specs/<feature>.md` | Implementers and reviewers of a feature. |
| Decision record | `docs/adr/<decision>.md` | Maintainers who later wonder why something was built this way. |
| Glossary | `CONTEXT.md` | Everyone: one definition per domain term. |
| Rustdoc | `//!` and `///` comments in each crate | Callers of a crate's interface. |
| Changelog | `CHANGELOG.md` | Release readers. It is generated from commit messages at release time, and pull requests never edit it. |

The default system prompt points the agent at the README, the `docs` folder and
the `examples` folder, and names the guide page for each common topic (for
example, `docs/extensions.md` for extensions and `docs/resources/skills.md` for
skills). A page the agent cannot find or follow is a question it cannot answer.

## Write a guide page

A guide page covers one topic. [Extensions](extensions.md) is a good model.

1. **Title.** Name the topic as a short noun phrase in sentence case: `# Skills`,
   `# Extensions`, `# Settings`.
2. **Purpose.** Open with one paragraph: what the feature is, when the reader
   needs it, and any limit they must know before they start. The extensions page,
   for instance, states early that a component is not an untrusted-code sandbox.
3. **Contents.** Give a numbered list of links to the sections when the page has
   more than three.
4. **Sections in working order.** Use these names when the concept matches, so
   pages read alike: *Locations* (where the files live), *Format* (the file or
   interface), *Usage* (how to use it), *Loading rules* (what loads, in what
   order), *Validation* (what makes input invalid and what the reader sees), and
   *Example* (a complete worked case). Add topic sections where these do not fit.
5. **Complete examples.** A reader can copy an example and it works: no `...`
   gaps, no invented flags. Prefer linking a shipped example file to pasting a
   long one.
6. **One copy of each rule.** When another page owns a rule, link to that
   section instead of restating it. Two copies drift.
7. **Reachable.** Link a new page from the page that introduces its topic, such
   as a `docs/extensions/` page from `docs/extensions.md`, so a reader can reach
   it by following links.

Use relative links between pages and give every code fence a language (`rust`,
`toml`, `json`, `sh` or `text`).

## Keep code blocks tested

When a guide page documents a crate, the crate root can include the page as its
crate documentation, for example in `crates/maestro-resources/src/lib.rs`:

```rust
#![doc = include_str!("../../../docs/resources/skills.md")]
```

Every `rust` block on that page then compiles and runs as a doctest when the
tests run, so the example cannot fall behind the code. Hide setup lines with a
leading `#` (see the `fn main` wrapper in
[skills](resources/skills.md#supplied-paths)). Blocks fenced as `toml`, `json`,
`sh` or `text` are not compiled. An example that cannot run as a doctest belongs
in `examples/` as a built target instead.

## Add an example

An example shows one behaviour in one file.

1. Add `examples/extensions/<name>.rs` or `examples/sdk/<name>.rs`, named in
   `snake_case` after what it does. Start it with `//!` documentation saying what
   it shows, as [`hello.rs`](../examples/extensions/hello.rs) does.
2. Register it as an `[[example]]` target of the crate whose interface it uses,
   with a relative `path` back to the single copy in `examples/` (see
   `crates/maestro-extensions-wasm/Cargo.toml`).
3. Add one row to the folder's `README.md` table saying what the example
   registers or does, and its run command if it has one. Keep both lists in
   file-name order, so a new example goes in its own place.

The shipped-examples test, `crates/maestro-app/tests/shipped_examples.rs`, fails
when a file has no row, when a row or link names a path that does not exist, or
when the table or run commands are out of file-name order.

## Write a specification

A feature's specification lives in `docs/specs/<feature>.md` and lands on `main`
before its implementation. Its issues link to it rather than copying it (see
[the issue tracker guide](agents/issue-tracker.md)). A feature specification uses
these sections, in this order. Leave out any that do not apply, and add sections
such as *Delivery order* when the feature needs them:

1. **Purpose and authority**: what the document governs and what it leaves alone.
2. **Problem statement** and **Solution**, from the user's side.
3. **User stories**, numbered: "As a *role*, I want *behaviour*, so that
   *benefit*."
4. **Behaviour contract** or **Design**: numbered rules grouped by area, each
   one testable.
5. **Implementation decisions** and **Testing decisions**.
6. **Differences with no reference source**, **Corrected reference defects** and
   **Runtime mechanics not copied**, when the feature has a reference behaviour.
7. **Out of scope**.
8. **Glossary**, with the terms it adds to `CONTEXT.md`.

[Packages](specs/packages.md) is a complete example. A behaviour that has no
reference source is listed under its differences, so a reviewer can see every
addition in one place.

## Record terms and decisions

**Glossary.** `CONTEXT.md` defines each domain term once:

```text
- **Package manifest:** The `maestro` key of a package's `package.json` listing its
  resources.
```

The definition says what the thing is, in one or two sentences, without
implementation detail. Add the entry in the same change that introduces the term,
then use the term everywhere. [Domain docs](agents/domain.md) explains how the
glossary and decision records are read.

**Decision records.** Write `docs/adr/<decision>.md` only when a decision is hard
to reverse, would surprise a reader without context, and was a real choice
between alternatives. A record has two sections, *Decision* and *Trade-off*, as
in [agent execution affinity](adr/agent-execution-affinity.md). Easily reversed
or obvious choices need no record.

## Write rustdoc and comments

Every crate root needs `//!` documentation and every item, private ones
included, needs `///` documentation; the build denies missing documentation and
rustdoc warnings ([AGENTS.md](../AGENTS.md#documentation)). Rustdoc starts with
what the item does, and adds `# Errors`, `# Panics` and `# Examples` only when
they apply ([identity guide](identity.md#code-identity)).

A comment explains a reason or an invariant the code cannot show. Comments never
carry planning material such as ticket numbers, issue references or delivery
notes; the conventions check reports any it finds, with file and line.

## Choose the words

Write crisp technical prose: state the result, name the constraint, give the next
action. Use the terms in `CONTEXT.md`, Maestro's own names, sentence-case
headings and straight quotes. Leave out hype and filler such as "seamless",
"magic", "just" and "simply". The [identity guide](identity.md#voice) has the
full word lists and examples.

## Check a change

1. Run `just check`. It fails on missing documentation and rustdoc warnings.
2. Run `just test`. It runs the doctests in included guide pages and the
   shipped-examples test.
3. Follow every relative link you added or changed.
4. Read the page as its reader would, start to finish. Read it once more for AI
   writing patterns: inflated claims, filler, repetitive structure, vague
   sources. Report each edited page in the pull request body as
   `HUMANIZER <path>: <n> fixes`.
5. Commit with a conventional header such as `docs(settings): ...`, and leave
   `CHANGELOG.md` alone.
