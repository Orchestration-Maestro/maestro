# Workspace architecture checks

The [foundation specification](specs/maestro-port.md#crates-and-delivery-order)
owns the crate graph. `maestro-test-conventions` checks that graph through
`check_workspace(root)`; it does not execute product capabilities.
Its crate dependency table lives in `src/graph/policy/table.rs` within the
conventions crate. `src/graph/policy.rs` keeps the rule logic and re-exports
`POLICY` for its existing callers. Table rows stay in crate-name order, as do
the rows of the specification's crate table and the crate names of `wasm_crates`
and each route of `test_inputs` in `.github/ci.toml`. The path patterns of
`test_prepare_inputs` sort as text instead, so
`crates/maestro-extensions-wasm/**` comes before
`crates/maestro-extensions/**`. A crate step then writes its line where it
sorts instead of at a position every other step shares. `check_workspace`
reports a row or entry that is out of that order, and a CI entry written on its
key's line.

## Application export layout

`maestro-app` keeps its public imports at the crate root. Private, static
`src/exports/` fragments hold the declarations by area: `browser_storage`,
`browser_tools`, `clipboard`, `configuration`, `credentials`, `extensions`, `json`,
`markdown`, `models`, `packages`, `presentation_data`, `process`, `resources`,
`session`, `settings`, `text` and `tools`. Each area owns its matching `.rs` file;
`exports/mod.rs` aggregates them without changing the original owners or target
gates. Unrelated areas therefore need not edit the same declaration file.

### Named exports

Run `just check` to check the application export boundary. Export records and
operations by name; do not expose the daemon, terminal or Markdown crate itself,
its modules or its wildcard imports.

The checker follows aliases across source files and local glob imports. A glob
imports only bindings visible from the importing module and cannot widen their
visibility through a public relay. Local glob bindings shadow root external-crate
aliases, including when a later export refers to an earlier alias through `self`.

For example, `pub use maestro_terminal_emulator::painting::Dirty;` selects one item.
`pub use maestro_terminal_emulator::painting;` exposes a whole module and is rejected.

The same run checks the credentials crate's public surface. Its root names every
export: the checker reports a public re-export or module whose name the crate's
declared surface does not carry, and refuses a wildcard, so a writer cannot
return as a public name.
Application-owned modules may group named records and operations.

## Inventory and dependencies

Dependency descriptions here state domain edges, subject to the
[native feature-unifier exception](specs/maestro-port.md#native-feature-unifier),
including leaves and exact sets. The unifier itself has no workspace dependencies.

The extension domain may depend directly on resources. The crates that list
`maestro-interchange` in the specification table may depend on it; its job is to define the
records a guest shares with its host, including command provenance records
`SlashCommandInfo` and the open `SlashCommandSource` label, and the renderer option
record `MessageRenderOptions`, `CustomMessageInput<T>`, `MessageDelivery`,
`UserMessageDelivery`, `SendMessageOptions` and `SendUserMessageOptions`. Resources and
application presentation data re-export the two command provenance records; session
re-exports the six others; guest authoring re-exports every record except
`CustomMessageInput`, which it defines as its own alias. The declaration scanner rejects
a declaration of any of these records except `CustomMessageInput` outside the record
owner, and duplicates within it.
The record owner depends only on the dependency-free JSON leaf.
No reverse dependency is permitted.

The [specification](specs/maestro-port.md#crates-and-delivery-order) declares these
owners, all now present: `maestro-text` and `maestro-json` are leaves, `maestro-interchange`
depends only on the JSON leaf and holds the session, tool, package, toolkit, child-process, extension-context and front
matter record modules,
and `maestro-brand` depends on text and JSON.
The `maestro-office` crate reads office documents (spreadsheet number formatting, workbooks and Word packages) and depends on text, JSON and lock; the web frontend consumes it.
The `maestro-http` crate performs HTTP requests: the replaceable client, retries, streamed bodies, server-sent events, proxies, proxy scripts, SOCKS and complete-message sockets. It depends on cancellation, interchange, text and JSON; models consumes it and re-exports the client records at its existing paths.
The `maestro-providers` crate adapts the built-in model providers: it holds their login flows, the callback server and the callback pages, and installs the built-in login providers into the registry that `maestro-models` owns. It depends on models, http, brand, text and JSON; credentials, the application and the composed executable consume it, and a credential store installs the built-ins when it is built.
The `packages` module of `maestro-interchange` owns the portable package interchange
records; native packages and guest authoring specialize its resolved-package scope. Pattern depends
only on text, so it is no longer a leaf. The [Markdown parser](markdown.md)
also depends only on text; the toolkit consumes its owned source records and
the web frontend imports them directly. The dedicated conventions checker may
consume the same parser's code-block source ranges for documentation checks.
Tools may also depend directly on text for shared newline
normalization. The guest authoring crate may depend on interchange,
JSON and text, never on native session, tool or catalog runtimes. The table is not
repeated here.

Pure retained [session-tree projection](sessions/tree-projection.md) belongs to
`maestro-session`; frontends import it from `maestro_session::export_html`.
Its allowed dependency set is unchanged; it has no frontend or toolkit edge.

The `maestro-export` owner may depend directly on `maestro-json` and
`maestro-text` for shared JSON and whitespace semantics. It may not depend on
`maestro-web`; the application supplies the viewer bundle, as described in the
[session export contract](specs/maestro-port.md#r04--session-trees-persistence-and-exports).
The `export_reuses_text_and_json_without_frontend_edge` architecture test checks
these permissions for normal, build, optional and foreign-target declarations.

`workspace-crates.json` lists workspace members and declared-but-absent owners
with string-valued classes.
The binary composition root, conventions checker,
checked test-process owner, disposable test-directory owner, test tracing owner,
portable fixture owner, repository tooling and the feature unifier
(`maestro-dependencies`) are `dedicated`; all product libraries and adapters are `core`.
Browser qualification (`maestro-test-browser`) is dedicated, unpublished and
depends on tooling for preparation. Hakari excludes its test-only browser stack
from traversal. Repository tooling (`maestro-tooling`) is development-only, never
shipped and has
no internal dependencies apart from the optional `maestro-path` edge and the native-only
feature unifier. Classes are not numeric
delivery layers. Known absent entries do not require placeholder crates; unknown
actual members are rejected.

The specification table identifies leaves with no internal dependencies; the
`maestro-path` crate is also a leaf. The
[architecture conventions test](../crates/maestro-test-conventions/tests/architecture.rs)
checks the table against the enforced graph policy. The frontend rows of that table
are the frontend edge table: chat, CLI, RPC, web, the multiplexer and the executable
depend only on the lower crates their row lists, declared when used, and a
frontend calls `maestro-app` only for what the application owns. Frontend dependency
requirements follow the [specification's dependency boundary](specs/maestro-port.md#common-semantic-and-boundary-contract).
Other crates may use a subset of their specification row, except the terminal
adapter, which requires the toolkit and `maestro-text`; an absent required toolkit
or text owner fails.

Completeness is the union of declared normal/build dependencies across features
and targets plus host-resolved dependencies. Transitive and dev dependencies do
not satisfy a direct requirement. The permitted internal development edges are
`maestro-tui-crossterm` to `maestro-brand` for the terminal examples, the
native-only `maestro-extensions` to `maestro-credentials` edge for extension
tests that build a real model catalog, the native-only `maestro-models-registry`,
`maestro-cli` and `maestro-extensions-wasmtime` to `maestro-providers` edges for tests
that read the built-in login providers, the native-only `maestro-detection` to
`maestro-text` edge for terminal newline normalization in checked-child tests, the native
`maestro-rpc` to `maestro-tui-crossterm`, `maestro-tui`, `maestro-models-registry` and
`maestro-credentials` edges for protocol tests that render exports and drive a real session
through the scripted provider, the native-only `maestro-chat` to `maestro-models` edge for interactive-mode
tests that drive a real session through the scripted provider, the native-only
`maestro-cli` to `maestro-listeners` development edge for the controlled print
adapter, the native-only `maestro-extensions-wasm` to `maestro-listeners`
development edge for the controlled guest bus, the native-only `maestro` to
`maestro-lock` development edge that serializes the executable's file-trigger
test on the shared host fixture, and the native development edges from the
thirty declared callers to the
`maestro-test-process` owner (see the
[specification](specs/maestro-port.md#test-process-owner)). The brand exception
permits neither a normal/build brand dependency nor a theme dependency, and does
not satisfy the adapter's required toolkit/text dependencies. The extension edge
permits no normal/build credentials dependency. The test-process
owner is reachable only through those native development edges, has no outgoing
domain edge and is never admitted by a `maestro-test-*` name pattern.
The directory owner follows the same native-only declaration checks, with the
exact caller list of its
[directory contract](specs/maestro-port.md#test-directory-owner) and one outgoing
edge, to `maestro-path` for the real path of its roots. The tracing owner
follows the same checks with its own five callers; see the
[tracing contract](specs/maestro-port.md#test-tracing-owner). The portable fixture
owner has six exact
development callers, listed in its
[fixture contract](specs/maestro-port.md#test-fixture-owner). Its declarations may
be portable or target-specific; normal/build edges and outgoing domain or `maestro-path`
edges are forbidden. Both production
(normal/build) and test (all kinds) graphs must be acyclic before individual edge
policy is checked.

The checker first runs offline `cargo metadata --format-version 1 --no-deps`.
It then detects the compiler host with `rustc -vV` and requests offline,
host-filtered full metadata with default features, not all features. Both metadata
children omit `RUSTC_WRAPPER`; the calling process and build commands retain it.
A thread keeps each document it reads and reuses it while the workspace layout, the
manifests, `Cargo.lock` and `.cargo` files stay the same. Edits to source files alone
start no second Cargo run.
Package IDs and canonical member paths establish identity, not dependency aliases.
Optional, build, dev and inactive foreign-target declarations remain subject to policy.
Malformed metadata and command failures return diagnostics, not acceptance.
`check_declared_graph` runs the declared pass from a metadata document the caller
supplies, so the conventions suite rejects a whole family of hypothetical edges from
one metadata read. Host-resolved edges, the required direct sets and the source
checks stay with `check_workspace`.

The cancellation leaf has no internal dependencies; only models and the toolkit
may depend directly on it.

The [shared filesystem effect contract](specs/maestro-port.md#crates-and-delivery-order)
declares watch below layer 0 without workspace dependencies. Lock depends only
on path for final-link resolution, using the existing `sha2 =0.11.0` to shorten
lock and record names for long file names. Only theme/application may consume watch.
These crates may consume lock: workspace, detection, settings, credentials, session,
models, agent, tools, extensions, application, the daemon, the runtime adapter, the
terminal adapter, the interactive frontend and the command-line frontend, plus the
executable crate as a native development dependency. The edge is portable so browser
callers reach the same operations. The lock owner provides
waiting, bounded and single-attempt acquisition, recoverable in-place byte publication
and poison-tolerant acquisition of shared state. See [locking](locking.md) for the
durable copy, replay order and shared-state contract.
Feature owners retain serialization, symbolic-link policy and scheduling.

The process owner `maestro-process` is declared core below layer 0 and depends only
on `maestro-text` for POSIX shell operand serialization. Only tools, packages, credentials,
detection, the application, the daemon, the toolkit and the terminal adapter may consume it, on native targets. It owns child-process lifecycles through its `children` module
(captured spawn, raw pipe collection, exit and EOF observation, termination and
reaping, with a caller-selected post-exit grace and TERM-then-KILL escalation); callers keep cancellation, termination, settlement and output policy.
Linux pane terminals use the separate `terminal` module, with setup, foreground facts
and an owned I/O actor. The `byte_queue` module owns ordered buffers and partial-write
offsets shared by that actor and native terminal output; callers keep readiness and
shutdown policy. See [pane processes](workspace-daemon.md#pane-processes).

Synchronous capture follows the timeout handling in the
[contract](specs/maestro-port.md#crates-and-delivery-order). The checker verifies
dependency ownership, not target predicates.

The listener leaf `maestro-listeners` is declared core below layer 0 with no
workspace dependencies. It owns one job: retaining callbacks in live registration
order, admitting one per identity and returning removed ownership to the caller.
Models, the toolkit, the agent, the application and the extension runtime may
depend on it and keep their own borrowing, locking, delivery and settlement
policy. Retired registrations are reclaimed once no traversal runs and at least
half the slots are retired, so removal costs expected constant work amortized over
the removals and clearing every registration costs work proportional to their
number. While a traversal runs, retired slots stay until it ends.

## Authored paths and `maestro-path`

An authored path, the text a user, setting or file wrote, stays a `String` and
goes through `maestro-path`; it becomes a `std::path::PathBuf` only at a
file-system call. The crate offers `join`, `normalize`, `resolve`, `relative`,
`dirname`, `basename` and `is_absolute` for the `posix` and `win32` flavors, plus
a POSIX `parse`, with the results of the JavaScript runtime's `path` module,
except where its result is a defect; the crate's tests mark each one. The crate
root re-exports the flavor of the compile target. Nothing reads the process
working directory or the environment: `resolve` and `relative` take a `Cwd`
snapshot from the caller, and `try_resolve` resolves what the operands anchor and
fails with `CwdUnavailable` where the runtime would have read the working
directory. `resolve_with` delegates to `resolve_with_flavor` using the target
flavor; callers can select either flavor explicitly with `resolve_with_flavor`.
Both resolve several operand groups from callbacks the caller supplies: it calls the drive callback once, and the
working-directory callback once, only for the groups the operands and drives
leave open, and returns that callback's error unchanged. `home_dir` returns a
supplied home, empty included, and calls its account callback only when none
was supplied. The callbacks decide what is read and when; the crate reads
nothing itself. Windows relative paths require matching resolved
root identities, including anchoring. A matching anchored drive, share or
namespace supplies a comparison base; otherwise, nonempty tails need a complete
shared first component. Empty tails still permit component traversal. Without a
shared base the normalized destination is returned: `D:a` to `D:b` under a UNC
working directory with no drive entries yields `D:b`, not `..\b`.

`maestro-dependencies` is the other named exception: every member may declare it,
and only as a non-optional normal dependency under
`cfg(not(target_arch = "wasm32"))`. It has no internal dependencies, is not listed
in the specification table, cannot satisfy a required direct dependency and is the
only crate whose direct `serde_json` entries bypass member inheritance.

`maestro-path` has no domain dependencies and is the optional-edge exception to
leaf and exact-set rules, alongside the native feature unifier. Any crate in the
specification table may declare it as a normal or build dependency (optional and target-specific forms included)
without changing its layer, subject to the exclusions in the specification's
[crate graph](specs/maestro-port.md#crates-and-delivery-order).
Lexical path helpers take supplied directory observations; `native_home_dir` alone
reads the native home environment and account entry. They add no filesystem
service, directory cache, scheduler or workspace dependency. The edge is ignored when checking a crate's complete
direct set or leaf
status. The
[architecture conventions test](../crates/maestro-test-conventions/tests/architecture.rs)
checks permitted table and `maestro-path` edges. Dev-dependencies on `maestro-path`,
dependencies from it, and cycles are rejected like any other internal edge.

The crate's `fs` module holds the path operations that read the file system or
the process directories. `maestro_path::fs::resolve` resolves a path against the
process working directory and the Windows drive directories, reading them only
when the operands leave a part open; it builds for every target, and
`maestro-resources` re-exports it at `maestro_resources::paths`. The other two
operations exist on native targets only. `maestro_path::fs::read_dir` lists one
directory's entries sorted by file name on every platform; callers keep kind
filtering, name decoding and any further ordering.
`maestro_path::fs::canonicalize` resolves [real paths](#real-paths).

## Real paths

`maestro_path::fs` is the one module that resolves a native path to its real path
through the file system: links followed, `.` and `..` resolved. Windows reports a
real path in the verbatim form,
`\\?\D:\work` or `\\?\UNC\server\share\work`. `maestro_path::fs::canonicalize`
returns the ordinary form instead, `D:\work` or `\\server\share\work`, through
the lexical `maestro_path::win32::ordinary`. A real path then compares equal to
the spelling users, settings and test directories hold, and Git, Cargo and the
other programs Maestro starts receive a path they accept. Other platforms get
the native real path unchanged. A Windows real path that is not Unicode text
keeps the verbatim form, because the conversion works on text.

Callers keep their own policy. The file mutation queue keys a file that does not
exist yet by its lexical path, and the write lock resolves the nearest existing
ancestor of a missing target. Workspace discovery falls back to the path it was
given. Each caller maps the error to its own kind.

The drive prefix and the verbatim spelling have one copy each.
`maestro_path::win32::has_drive_prefix` answers whether a path opens with a
drive letter and its colon, and `maestro_path::win32::ordinary` converts the
verbatim form; the skill, extension and pane owners call them instead of testing
the bytes themselves, and the Windows root parser reads a drive root through the
same test. Each keeps its own policy: the skill walk converts a link target, the
extension adapter maps a component spelling, and the pane drops the leading
slash of a file-URI drive path.

Like every caller, `maestro-lock` and `maestro-packages` call the owner only
on native targets, where it exists. Their browser build reports `io::ErrorKind::Unsupported` for
a real path, the failure a missing file system gives, so the containment checks
in those two crates refuse there rather than admitting a lexical path.

## Bounded ownership checks

Direct `wasmtime`/`wasmtime-*` library declarations belong only to the runtime
adapter, except that the guest-authoring crate may declare `wasmtime` and
`wasmtime-wasi` as development dependencies for its test-only host probe;
`wasmtime-wasi-http` is excluded everywhere. The terminal toolkit cannot
declare `ratatui`, `syntect` or `two-face`, including aliased, optional, build,
dev and foreign-target forms. A root may transitively use its runtime adapter.
Review also checks equivalent new framework/highlighting libraries; the finite
library-name list is not a general library classifier.

Rust member files, including files outside `src/`, are traversed while skipping
`target/` and `.git/`. Each file is read once; a leading byte-order mark is removed
and physical CRLF is converted to LF before native Rust parsing. Parsing, comment
lexing, production line counts and WIT literals share that canonical text.
Declarations are recognised from native tokens, so macro templates and every item
shape are covered without expansion; attribute payloads are excluded.
Raw identifiers (`r#name`) normalize to their ordinary names. Tool definition/render-context/result-option types
belong to tools, except the guest author definition in
`maestro-extensions-wasm/src/types/tools.rs`; application selector types and selector
modules belong to chat.
The four message delivery records belong to `maestro-interchange`, and so do
`ForkPosition` and `NavigateTreeOptions`. Duplicate declarations in these
owners are rejected. Qualified uses and re-exports are not declarations. Duplicate tool declarations
within either owner are rejected. Planning references in comments retain their separate policy and
line-accurate diagnostics from native compiler-lexer comment spans.
Legal included fragments need not parse as full modules; the compiler owns
Rust validity, while the quality checker still counts their production lines.

WIT checks recognize actual `wit_bindgen::generate!` and
`wasmtime::component::bindgen!` declarations. A `path:` may be a static string or
array of static strings. Cooked strings use Rust escapes (including hexadecimal,
Unicode and line continuations). Physical CRLF within cooked and raw literals
becomes LF through shared source normalization, while escaped carriage returns
remain carriage returns; raw strings otherwise keep their literal contents.
A shorthand string macro argument names a world and
reads the declaring member's default `wit/` directory, not a path named by the
string. Paths resolve relative to the declaring member's Cargo manifest. Every input must
exist inside the canonical guest-authoring member directory, not a copied host
tree. Directory inputs are roots; file inputs use their containing directory.
The host's generated binding names the guest's `wit/` directory as its `path`, and the
roots of the host and guest generators must match. An inline guest package borrows its
directory for dependencies only, so it adds no shared root.
When host and guest both declare inputs, their canonical root sets must agree;
world names and individual file selections may differ. With one side declared,
ownership still applies but root-set equality is inactive. Inline WIT outside
the guest is rejected. Dynamic paths (`env!`, `concat!`, constants), missing paths
and unresolved declaration shapes require review and do not pass verification.
No owners or inputs means the check is inactive, not that a runtime passed.

These scans are structural, not a Rust semantic analyzer. They do not resolve
arbitrary macros, generated identifiers or computed paths. Manual source review
must establish caller-supplied generic tool context, no tools-to-extension or
application coupling, no application policy in frontends, a wiring-only binary,
and complete WIT code-generation evidence. Current absent owners are tested with
scratch workspaces, not certified as delivered capabilities.

The RPC mode lives in `maestro-rpc`: the session records in `rpc_types`, the one
connection runner in `connection_runner`, the client of a child process
(`RpcClient`) and the standard-stream adapter. The conventions check rejects an
`Rpc*` declaration in any crate other than `maestro-rpc` and `maestro-protocol`,
naming its file and line. One declaration is exempt: `RpcModeRunner` in the
application's `presentation_data/mode_dispatch.rs`, the callback its mode dispatch
runs. The protocol tests drive sessions through the application's scripted session
harness, `maestro_app::test_support`. They enable it with the application's
`test-support` feature on a native development dependency.

The chat tests that drive the interactive frontend build their session,
credentials, catalog and resource loader in
`crates/maestro-chat/tests/footer_support/harness.rs`. The conventions check rejects
a second construction of the session configuration anywhere in the chat crate's
test code, naming its file and line, so every chat test shares one setup, which
returns a session with its services. The rule covers every chat test source in any
directory, except the setup itself. Paths compare by component, so a native
separator changes nothing, and the shared token walk includes macro arguments,
closures and impl bodies.

Tests of the interactive mode drive it the way a user does.
`crates/maestro-chat/tests/interactive_mode_support/` builds the mode through
`InteractiveMode::new` over the recording terminal and the manual frame host, with a
session whose provider the test scripts. A test types keys into that terminal, prompts
the session or stores its history, and reads the terminal's effects and the mode's
read-only view. `InteractiveMode::view` returns a `ModeView`: `region` renders one of
the eight laid-out regions (`Region`) at the terminal's width, and `editor_text`
returns the current editor's text. The view exists only under the crate's non-default
`test-support` feature, which the crate's own development dependency enables. A test
that needs state the view does not show stays beside the code in
`src/interactive_mode/tests/`. The child-process isolation and the recorded-case
readers live once, in the support directory, and the tests inside the library include
the same files.

## Quality checks

Native platform viewers belong to the application's `utils::platform_opener`.
The conventions check reports executable literals outside that owner, including
nested macros, closures, methods and source fragments. It also tracks local
`open` bindings into command constructors and rejects the shell `cmd /c start`
combination. Parsed test-only items are excluded.

A single name passed to `clap::Command::new`, directly or through an explicit
import, defines a command-line item rather than launching a viewer. The check
accepts those names without exempting process calls in the same file. Imports
follow their containing token group, and competing imports keep the process
check active. Globs, re-export chains, constructed names and type inference are
not resolved; reviewers check them. For example,
`use clap::Command; Command::new("open")` is accepted, but
`std::process::Command::new("open")` beside it is rejected.

Clippy rejects the ambiguous `serde_json::Map::remove` in every target `just check`
compiles. `shift_remove` keeps the remaining key order; `swap_remove` is only for
maps whose remaining order is never observed. Ordinary maps are unaffected.

Conventions reject a shell script directly under `.config/`: nextest's setup entries call
the development tool in `maestro-tooling`, which has the type checker, the linter and tests.

Documented commands run `cargo` or `just`, so a page may not name a program from a
directory under the reader's home that a checkout does not provide. Conventions read
the Markdown version control tracks, documentation test fixtures included, and
report each file once at its first line naming a program in `.local/bin`, `~/bin`,
`$HOME/bin`, `${HOME}/bin` or `/home/<name>/bin`. A page that names the directory
itself still passes,
as do agent directory paths such as `~/.maestro/agent`, and a local page that never
ships stays out of the check.

Public text is Maestro's own, so another project's names and words stay out of it. The
check reads each Rust identifier, string literal, doc comment and other comment in the
members' sources, each Markdown page that version control tracks under `docs/`, and each
tracked file and directory name under `crates/` and `docs/`. It splits the text at
non-alphanumerics and case changes into words and checks two lists in
`crates/maestro-test-conventions/src/leaked_words.rs`:

- `DENIED` holds generic library and product words, readable. A word of one or more
  segments joined by `_` matches when its segments sit next to each other, joined by `_`,
  `-` or a case change alone and ignoring case. A longer word never matches: a denied word
  `quartz` is found in `quartz_load`, `QuartzLoad` and `quartz-notes.md`, not in `quartzite`.
- `HASHED` holds the 64-bit FNV-1a hash of the lowercase form of other projects' own names,
  so the repository never spells them. A word matches when its hash does. A short name that
  is also an ordinary word stays off the list, and this scan does not check it.

A hit names the file, the line, what holds the word (identifier, string literal, doc
comment, comment, documentation page, or file or directory name) and the word as the text
spells it. The diagnostic names `DENIED` and `HASHED` as the lists to edit.

`EXEMPT` beside them holds the places a user, a provider, a stored file or a detected
program carries a word, each with its path, word (a hashed word by its hash in `0x` and
sixteen hex digits) and reason. The deny list's own file holds each readable word, so the
check adds one row per `DENIED` word for it. A row that matches nothing, including one whose
path left the tree, is reported, so an exemption cannot outlive its need.

A module is named for what it holds. Conventions reject a module below a member's `src/`
named `utils`, `util`, `helpers`, `common`, `misc`, `shared` or `types`: such a name says
where code was put. The check reads every module declaration, inline ones and test modules
included, and every module file and directory. A module is identified by its member and its
path below `src/` (`records::types`), so a declaration, its file and its directory
are one module. It is reported once, with the file and line of its declaration, or with
its path when no parsed source declares it. Sources outside `src/`, such as `tests/` and
`examples/`, are not checked.

`EXEMPT` in `crates/maestro-test-conventions/src/bucket_modules.rs` holds the modules that
keep such a name, one row each: the member, the module path and the reason. There are two
reasons. A module that mirrors a file or directory of the ported interface keeps the layout
it mirrors. A module of Maestro's own that has no better name yet stays listed until it is
renamed. The rows are sorted by member, then by module path, and a row out of order or
repeated is reported. In the workspace a row that matches no module is reported too, so a
renamed, moved or deleted module takes its row with it in the same change. A new module
takes a name for what it holds, and the table gains no row for it.

Tests wait on completion signals; shared CI bounds the whole job. Conventions
report recognized test-owned timer calls with file and line: Tokio
`timeout`/`timeout_at`, standard `recv_timeout`, `recv_deadline`, `wait_timeout`,
`wait_timeout_while` and `std::thread::park_timeout`. Test ownership comes from
test file layouts, test-support members and explicit `cfg(test)` scopes,
including impl/trait methods and associated-constant initializers.
Direct paths and known imported, renamed, grouped or namespace-alias timer forms
are checked; known timer-module globs are recognized. Direct timer paths take
precedence over aliases; competing known aliases are retained. Aliases apply file-wide;
local shadowing is not modelled. Production deadlines remain allowed. Comments
and literal contents are not call syntax. The scan does not expand macros,
infer receiver types, resolve arbitrary re-exports/function values, evaluate
compound configurations or follow unparsed fragments. These forms and sleep-based
watchdogs remain source-review obligations.
Optional `futures_util::poll!` invocations are also rejected in those test scopes,
including qualified/raw paths, known imports and literal nested macro tokens.
Pin the future and call `std::future::Future::poll` once with
`std::task::Context::from_waker(std::task::Waker::noop())` instead; this needs no
optional dependency feature. Macro aliases use the same file-wide scope and
limits as timer aliases. Comments, literal contents and ordinary function calls
are not macro invocations; no compiler expansion is performed.

Conventions reject token-identical function bodies with at least 30 interior
tokens, including free functions, methods, trait defaults and nested functions.
Authored test code counts the same as production code: bodies in `tests/` files,
`tests.rs`, `#![cfg(test)]` files and inline `#[cfg(test)]` items join one
identity index with every production body, across all workspace packages. A
copied test helper is therefore reported until its callers share one owner, such
as the package's test module root or a test-support library they already
depend on. Whitespace and comments are ignored; token spelling, literal contents
and delimiter kinds are preserved. Each diagnostic lists the token count and all
native file paths with one-based signature lines, including same-line sites.
Only the generated model catalog and the exact generated binding files of the guest and
host extension crates are excluded. Macro definitions are not expanded, but authored invocation tokens
inside functions are compared. Unparsed fragments produce no candidates;
compound configurations are not evaluated. There is no baseline, allowlist or
suppression attribute.

Conventions reject a platform temporary directory that any member's test code
names itself. Test-owned code is a test file layout and every item inside an
inherited `cfg(test)` context, closures and macro arguments included; the
`temp_dir` spelling is rejected there as a call, as a value or inside macro
tokens, and its import is rejected anywhere in the crate, whatever the alias. A
method named `temp_dir` is the receiver's own API, never the platform spelling,
so a call on a receiver is accepted while every member of its arguments stays
checked, and a declaration's own `temp_dir` name is accepted the same way with
its body and arguments staying checked. The check follows no bindings or data
flow. Tests that observe the
platform directory, name an uncreated socket path under it or pass it as the
owner's parent read it through `maestro_test_directory::temporary_root`, and
directory creation and removal go through `Scratch`. The check covers every
member, so a new crate is checked on arrival; `maestro-test-directory` alone is
outside it, because its `temporary_root` accessor is the one place that reads the
platform directory. The diagnostic names `TEST_DIRECTORY_CALLERS`, where a crate
asks for the owner.

Conventions reject real-path resolution in production code outside
`maestro_path::fs`, with the file and line. There the name `canonicalize` is
accepted only as the qualified path `maestro_path::fs::canonicalize`. The check
rejects `std::fs::canonicalize` and `tokio::fs::canonicalize`; the `canonicalize`
method on any receiver; a path such as `Path::canonicalize` used as a value; an
import of the name from anywhere, under any alias; and the name in macro tokens
without the owner's crate and module in front. The scan reads names, not types, so a crate's
own method needs another name. It skips test code as the other checks define it:
test layouts, items inside an inherited `cfg(test)` context and the
`maestro-test-*` crates.

Clippy's protected `redundant_clone` lint rejects unnecessary copies workspace-wide. The protected `module_name_repetitions` lint rejects an item whose name starts or ends with its containing module's name; rename the module and keep the exported item name.
Hand-written documentation uses only `///` and `//!`; conventions reject block doc comments with their file and line, except in the generated bindings at `maestro-extensions-wasm/src/bindings.rs` and `maestro-extensions-wasmtime/src/bindings.rs`. Line documentation in those files is still checked for repetition; allowed block comments end the comparison run.
Conventions reject consecutive identical nonempty trimmed physical documentation lines within one contiguous run of same-style line documentation comments, retaining list and quote markers. An attribute, an ordinary comment or code ends the run. All Markdown code blocks (including nested list and block-quote fences) reset comparison and are ignored. Blank documentation lines are retained and do not reset comparison; owner/style changes do. Headings, rules and HTML lines are compared in their physical order like other non-code lines.
Under `crates/*/src`, files carrying `#![cfg(test)]` must be named `tests.rs` or be below a `tests` directory. A module named `tests` must carry `#[cfg(test)]`, every `#[cfg(test)]` module must be named `tests`, and test module declarations cannot carry `#[path]`. These layout conventions support production line counting; module declarations are validated throughout the syntax tree.
Duplicate assertions are a review judgement.

Conventions reject a second or foreign declaration of `MessageRendererCall`:
`maestro-extensions` declares it exactly once in `src/types/renderers.rs`. Chat
production syntax may not name the raw extension machinery: `MessageRenderer`,
`CallbackEmitter`, `EventBus`, `get_message_renderer` or `invoke_callback`.
Import declarations, imported aliases, every segment of a qualified path and
method calls also count. An import path is read whole: a prefix module, a nested
group, a renamed item and a glob that names a restricted item are each rejected.
The frontend uses the projected call and `get_message_renderer_call` instead.
Test layouts, `cfg(test)` scopes, ordinary comments and string literals are
exempt. The scan reads parsed syntax, so it does not expand macros, resolve
arbitrary re-exports or follow unparsed fragments.

The root manifest must forbid every protected quality lint; member manifests
are parsed as TOML and must set the boolean `lints.workspace` to `true`.
The only exception is the guest-authoring manifest's exact table below;
missing lint wiring is rejected for every member, including that guest.
The compiler rejects protected-lint allowances, including attributes emitted by
macros and conditional attributes. Lint-group allowances fail because the
workspace also forbids `forbidden_lint_groups`, which ignores warnings denial.
Test files are exempt from the 500-line production limit. Documentation tokens
(`///`, `//!`, `/** */`, `/*! */`) are excluded in complete Rust modules and
included expression fragments alike. Files that parse as Rust modules also
exclude items carrying `cfg(test)`, using source line and column spans regardless
of attribute order. A non-blank line counts when any non-whitespace character
remains outside those exclusions: code sharing a documentation or test line still
counts. Ordinary comments and string contents count. Blank lines are excluded.
A trailing test item's comment-only suffix is excluded only when no production
item follows. Rust validity belongs to the compiler.

The guest-authoring crate and the native component host each declare their own
lint table: the workspace table with `pedantic`, `too_many_arguments` and
`excessive_nesting` lowered from `forbid` to `deny`, because generated bindings
cannot compile under `forbid`. Each crate generates its bindings from the same
interface files, the guest for the extension to run against and the host for the
traits it implements. Only a crate's generated `bindings.rs` may carry a lint
attribute, the single allowance `clippy::same_length_and_capacity`. Every handwritten
top-level module, integration test and example restores the groups with
`forbid`, nested files inherit it from their parent module, and the crate root
stays wiring-only.

Every member retains `unsafe_code = "forbid"`.
The [terminal emulator](terminal-emulator.md) is pure Rust.

The workspace daemon is core design: the root always wires the daemon,
multiplexer. Required direct edges are root → application and multiplexer,
and multiplexer → application. The required root
edges must be non-optional normal dependencies active on native builds. Like the
CLI and chat, the multiplexer may use application, toolkit, terminal adapter and
theme; it may also use the protocol crate for frame records. The `maestro-path`
and native feature-unifier exceptions apply too.
Declared-but-absent core owners require no placeholder crates.

Browser geometry uses installed `js-sys` host calls rather than a handwritten
binding macro: `Reflect::get` catches method lookup failures, `Function::call0`
retains the element receiver and catches invocation failures. Unchecked handle
conversion admits rectangles from a second same-origin realm. Both failure
phases reach the existing host error and restoration path; lint levels and the
generated-guest exception above are unchanged.

The 28 browser-library roots are declared in `.github/ci.toml`'s `wasm_crates`,
including `maestro-path`. Check each declared library locally with:

```sh
cargo clippy --no-deps -p <crate> --target wasm32-unknown-unknown --locked -- -D warnings
```

The current shared wasm job builds these roots; it does not yet run this lint
command. The [shared wasm-lint rollout](https://github.com/Orchestration-Maestro/maestro-rust-workflows/issues/73)
tracks that separate extension. Native strict checks remain unchanged.

A browser compile witness is an example target whose body only compiles for the
browser target, so native Clippy never reads it. Every witness the workspace keeps
is listed with its owning package and source in `maestro-tooling`'s browser smoke
command, which compiles each one for `wasm32-unknown-unknown` and stops at the
first failure. The commit hook runs it when a staged crate owns a witness, and
`just check` runs it for the workspace. `.github/ci.toml` declares the same
witnesses under `wasm_examples`, so the shared wasm job compiles each one on every
pull request, and a test fails when the declaration and the witness list differ.

## Commands and failure evidence

Run `just check`, `just test` and `just ci` with pinned tools. `just check` includes
formatting, Clippy with warnings denied, strict public rustdoc and conventions;
`just ci` is the shared merge check. Workstation Cargo commands, including nested
metadata and hook commands, use the capped launcher with `CARGO_BUILD_JOBS=3`.
As #109 specifies, `just test` remains unwrapped; the separate ported non-LLM test
script removes the specified provider credentials and sets the agent's auth file
aside, restoring it on exit.

For example, a scratch workspace with listed settings and models members cannot
add this settings manifest declaration:

```toml
[dependencies]
maestro-models = { path = "../models" }
```

`check_workspace` returns:

```text
forbidden production dependency: maestro-settings -> maestro-models
```

Removing the dependency passes. An otherwise empty CLI fixture instead reports
`maestro-cli: missing required direct dependency maestro-app`; restoring the
application dependency passes. Other listed frontend edges are permitted, not
required, and are declared only when used. Source failures include the offending
file and line.

The row bounds every crate a frontend can name; the reach check closes the one route
around it, a lower-crate item that the application re-exports. It resolves each path a
frontend writes below `maestro_app` through the application's own bindings and rejects
one that lands in another crate:

```text
crates/maestro-chat/src/lib.rs:1: Tool belongs to maestro-tools; import it from its owner, not through maestro-app
```

The application's own exports obey the same boundary: they name each item of a
lower crate, never a module, wildcard or crate alias.

`check_workspace` reads each Rust source once. A file is parsed, lexed and walked a
single time; the checks read what the walk recorded, and no parsed syntax outlives
loading. Files load on one thread per available core, and the source checks then
run side by side over the loaded members. When several checks fail, the first in a
fixed order is reported, so a run that fails reports the same failure every time.

Checked test selection and direct native fixtures belong to `maestro-test-process`.
Its one harness-free example receives literal native operands; its installer copies
aliases through that same artifact without writable executable handles in the test
runner. `maestro-tooling` builds, fingerprints and stages the host artifact in the
existing test bundle and nextest setup exports its explicit path, including after
archive relocation. This adds no domain dependency or guest/browser native edge.
Each isolated child applies its typed native setup at its entry, so the test runner
keeps its own modes, limits, session and environment. See
[child-process tests](development.md#child-process-tests).

The conventions checker rejects Rust string tokens whose decoded value is
`--exact` outside `maestro-test-process`, reporting the source path and first
violating line. It scans all loaded Rust sources, including macro tokens,
disabled configurations and source fragments. Comments, larger fixture strings,
byte strings and C strings are not selector tokens. This bounded rule does not expand macros, resolve constructed
strings, interpret data files or infer process launches; review still checks
those cases.

The same source owner checks parsed `ignore` attributes. Ignored tests name
either their controlled-child owner or the pinned Chromium job. The Chromium
reason belongs only to `maestro-test-browser/tests/browser.rs`, where every test
must carry it and the integration root must register that module. Comments and
fixture strings are not attributes; syntax fragments without a parsed file are
not checked for ignore ownership.

Every crate builds its integration tests as one program from
`tests/integration.rs`. Add new `tests/*.rs` files as unconditional out-of-line
modules in that root; support modules stay in their existing directories. The
conventions check covers every workspace member, new crates included: a member
with top-level integration-test sources needs `autotests = false` and exactly
one test target, the runnable `integration` target at `tests/integration.rs`.
A library target that holds in-source tests keeps `[lib] test` on; one with none sets
`test = false`, and the conventions check rejects either mismatch.

Tool metadata has one generic record owner, `maestro-interchange::tool_info::ToolInfo<P>`.
Native callers use its default parameter-schema owner through models; guest authoring
keeps a `ToolInfo` alias over the same owner. Live callback storage
belongs to `maestro-listeners::Listeners<P>`, with one admission per identity by
default or one per registration as an event-emitter listener list keeps them.
Agent subscriptions, session listeners, extension error delivery, terminal input
listeners and native output-failure handlers reuse it. For shared `Rc` callbacks it
also owns subscribing, which returns the removal
operation, and the live walk in registration order. What each listener receives
and when it runs remain with the caller. The collection returns removed ownership
for release outside guards, and it reclaims retired registrations while no
traversal runs, so one removal and a complete clear both stay cheap.

Extension runner sessions and context catalog getters retain one direct shared
`ModelRegistry` holder; its internal synchronization stays catalog-owned.
The complete action runtime composes the existing activity/flag state.
Binding installs action and base-context operations before provider flushing;
command-context binding remains independent.


Shared extension event/result schemas and assembled prompt records belong to
`maestro-interchange`, specialized with existing native/guest operands. The declaration
scanner rejects duplicate schemas while allowing specialization aliases and
re-exports. One event inventory feeds author unions and guest adapter data.

The extension `ToolDefinition<P,D,S>` alias specializes the existing tools record
with its concrete context and local invocation. The ownership scanner permits
that alias in native registration; the application root keeps standalone defaults.
