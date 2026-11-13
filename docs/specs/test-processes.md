# Rust test children

This contract describes checked reruns, direct native fixtures and the typed native setup an isolated child applies before its body. Acceptance checks below are requirements, not implementation receipts.

## Problem statement

Controlled test programs still require Python or generated shell programs. Tests that reach a native executable through `PATH` cannot use an ordinary libtest rerun: literal native operands are not harness arguments, and harness output changes byte protocols. Tests need one Rust owner for both routes without changing the product operations they exercise.

Confidence: **proven in source**. Evidence: #3768; `crates/maestro-test-process/src/lib.rs`, `src/proof.rs` and `src/test_process.rs`; the [test-process contract](maestro-port.md#test-process-owner). Runtime and platform qualification of the replacement remain **unknown** until its acceptance checks pass.

## Solution

Keep checked test reruns and raw native fixtures in `maestro-test-process`. Use checked reruns when the test owns the invocation and framing. Use one prepared harness-free Rust executable when product code must receive native operands or raw streams. Replace interpreted fixture setup with typed native effects at the beginning of the isolated Rust child. Repository preparation remains in `maestro-tooling`.

Confidence: **proven requirement**, authorized by #3768. The native fixture, `install_executable(artifact, destination)`, artifact preparation and typed native setup are implemented and tested. No product startup, search, shell, RPC or cancellation behavior changes.

## User stories

### Launch and observations

1. As a test author, I want an exact named-test rerun, so that a missing test cannot pass without running. **Proven existing contract:** [test process owner](maestro-port.md#test-process-owner).
2. As a native-backend test author, I want an executable found by its real program name, so that literal arguments reach the backend without an extra product hook. **Proven requirement:** #3768.
3. As a protocol test author, I want binary streams without harness output, so that framing and malformed-input assertions remain meaningful. **Proven source behavior:** application RPC, component exec and executable I/O tests.
4. As a lifecycle test author, I want the actual child PID and real descendants, so that exit, EOF, signals and group cleanup retain their separate meanings. **Proven source behavior:** process, tools, executable I/O and completion tests.

### Isolation and delivery

1. As a test author, I want native setup confined to the isolated child, so that file permissions, limits and session identity do not alter the test runner. **Proven requirement:** #3768 and existing isolated setup callers.
2. As a test runner, I want one explicit prepared artifact in local, filtered and archive runs, so that tests neither compile fixtures nor search build directories. **Proven existing preparation seam:** `maestro-tooling` test setup.
3. As a maintainer, I want every controlled interpreter program replaced without losing its assertions, so that Rust-only support does not reduce qualification. **Proven requirement:** #3768.
4. As an example user, I want actual shell and SDK-language behavior still qualified, so that replacing test support does not remove the technology being tested. **Proven scope clarification:** #3768; [SDK and RPC guide](../rpc.md).

## Implementation decisions

### S01: one test-process owner

**Confidence: proven requirement. Evidence: C01-C02.**

The existing owner retains checked selection, eligibility, startup proof, environment edits, stream operations and reaping. It also owns one harness-free native fixture example target and the installation operation callers need. The target's entry point only dispatches private, cohesive fixture families. The sole integration program remains `tests/integration.rs`.

Eligibility reads one executable's harness listing once, on the first launch that needs it, and answers every later launch from that reading: a running executable cannot change what its harness lists, so a test that starts many children pays one listing, not one per child. A stale or misspelled identity is still rejected, and a failed reading is not kept, so a broken harness reports the same failure on every launch.

One selection scheme carries a test's child role. `TestProcess::case` writes the role into the child, and `selection()` reads the role and the identifier back; fixture data a stage needs travels in its own variable. The owner spells that variable once, and its guard rejects any other quoted `MAESTRO_*_CHILD` or `MAESTRO_*_CASE` name under `crates/`, in a file of any type, listing every site it found.

No new crate, product resolver, alternate launcher, relay supervisor, runtime compiler or script evaluator is introduced. The fixture target has no dependency back into product owners. Host-side tooling stages an artifact; it does not gain a normal library dependency on test-process.

### S02: direct native execution

**Confidence: proven requirement. Evidence: C01, C05, C08-C10.**

Callers supply the explicit prepared artifact and destination paths to one fallible installer. Installed aliases execute that artifact directly. Select finite scenarios by an explicit selector or the installed program name with a sibling fixture-specific record. Parse records into typed values before effects; reject unknown or malformed input. Keep original native operands untouched, including empty strings, spaces, quotes, metacharacters and Unicode. Preserve native non-text arguments where the existing platform supports them.

The native process is the child under observation. It writes no harness prologue and preserves raw stdin/stdout/stderr, cwd, environment, status, signal and PID observations. A descendant retaining pipes remains a real linked Rust descendant, not a thread. Reuse the existing owner's byte, socket, readiness, completion and descriptor helpers.

Installation runs through the same artifact's private copy/permission operation. The test runner does not hold a writable executable descriptor across sibling launches. Copy mutable-age fixtures rather than hard-linking them. The caller's disposable directory owns installed files. No external `install` command is needed.

### S03: prepared artifact supply

**Confidence: proven requirement at the existing setup seam. Evidence: C01, C03.**

Preparation builds the host fixture target once for its inputs, identifies the emitted artifact, records source freshness and content, stages it with the existing bundle and exports its absolute path through existing nextest setup. Local preparation, filtered package runs and archive extraction/remapping receive the same explicit artifact contract. A missing, stale or incompatible artifact fails clearly; tests never discover a target directory or compile a fallback.

Keep native test-use admissions exact. A guest-authoring crate may import the installer only through its declared native development edge; guest and browser artifacts acquire no native dependency. Reuse already approved exact dependencies and features, inherited from the workspace. Update offline fixture dependency sources when the actual closure changes. No new library is authorized by this specification.

### S04: native isolated setup

**Confidence: proven requirement, delivered. Evidence: C01, C04.**

The shell-source setup field, interpreter launch branch and `shell_setup` API are gone, and every caller moved in one green change. The consuming checked builder carries only the typed effects existing callers need: Unix umask, process-count limit, nonterminating file-size-signal handling and new-session creation. Environment edits remain the existing ordered environment operations, including byte-valued Unix `HOME`.

Every isolated child applies its request as its first child-entry operation and retains any registration guard until it finishes. Failure prevents the operation under test. No parent-global mutation, unsafe pre-execution hook or preload/relay performs setup.

Use already approved safe native facilities. Nonterminating file-size-signal handling is not an ignored signal disposition and does not promise ignored-disposition inheritance across exec. Prove current-child file-size errors and restoration at actual callers. Preserve their observed modes, limit premises, session identity and subsequent successful writes. Existing cache preparation becomes explicit ordered native operations, not a generic setup language.

### S05: complete fixture conversion

**Confidence: proven requirement. Evidence: C01, C05-C18.**

Convert every remaining controlled Python or shell child, shared producer and indirect consumer on current main, including root examples, generated programs, browser setup and platform branches. Preserve existing semantic assertions and all failure variants. A fixed file count is not completion evidence.

| Capability | Observations retained | Evidence (proven in source) |
| --- | --- | --- |
| Application RPC | JSONL/raw-line capture, id/command correlation, absent versus null, refusal, bursts, reply order/replay, malformed and split records, stderr, closed streams, withheld replies, status, signals and restart/stop behavior. | C05 |
| Application native helpers | Clipboard bytes/background transfer/limits, Git argv/cwd/status and descendant-held EOF, configured shell marker, transcript/native-alias observations, package chatter/root/update order and failures. | C06 |
| Chat and completion | Literal hostile URL, detached browser return, editor bytes/modes/operands and failures, trash diagnostics, secrets/counts, concurrent search probes, notices/share frames, cleanup and group-stop behavior. | C07, C17 |
| Tools and search | Literal argv/cwd/env, large independent streams, UTF-8 splits, cancellation/deadline/escalation, linked trees, fd/rg discovery/version/status/raw bytes, hostile patterns and real native search corpora. | C08 |
| Component and executable I/O | Existing guest inputs and context lifetime, binary descriptors/PIDs/groups, observer order, cancellation/retained handles, output routing and no guest native dependency. | C09-C10 |
| RPC and SDK examples | Dialog correlation/cancellation, terminal lifecycle, LF/CRLF and Unicode separators, completion/acceptance in either order, failure and cleanup. | C11 |
| Repository recipes | Existing Cargo forwarding, private Cargo homes, cache ages/fingerprints, authored hooks/env/cwd, artifact output and build ordering; absorb the existing separate recipe dispatcher. | C12 |
| Other native adapters | Package lifecycle/capture/Git transport, credential-process JSON/count/cache/failures, notifications/repository queries, metadata/compiler selection, live foreground payloads and clean CLI stdout. | C13-C17 |
| Browser setup | Binding-version/lock agreement, inherited dependencies and feature union, absolute paths/patches, copied lock/toolchain and no new external lock identities. Existing browser driver work owns this setup port. | C18 |

### S06: the startup boundary stays

**Confidence: proven requirement. Evidence: C01, C05.**

Keep `STARTUP_OBSERVATION` and the 100 ms startup boundary. The early-exit case retains code 7, `boot failed\n`, accumulated stderr, duplicate-start and signal assertions. Qualify it in 200 separate runs while controlled Rust CPU workers keep every assigned test core busy. Start/release and join those workers explicitly. Save each outcome; this qualification is not a permanent long-running test.
The held clock of the early-exit test replaces wall-clock luck, so the 200 runs became a loaded 200-test-run stress qualification; the lane saved it as a log.

### S07: retained real boundaries

**Confidence: proven scope requirement. Evidence: C01, C11, C13, C17.**

Keep real shells when shell parsing, login behavior, command execution or remote grammar is the subject. Keep Windows command formats, PATHEXT, quoting and junction creation where those native behaviors are qualified. Controlled payloads inside those operations still become Rust.

Keep real Git, npm, Cargo, rg, fd and native signal operations where their behavior is tested. Keep the documented Python SDK product example; Python may run only to qualify that example, never as its peer, harness or setup. Preserve non-executed agent-detection data, language corpora, rejection inputs and delivered templates. A maintenance generator with no test caller is not a test fixture.

## Testing decisions

Each row is an acceptance gate, not a claim of an executed result. Requirements are **proven** from #3768 and existing tests; replacement/runtime results are **unknown**.

| ID | Acceptance criterion | Test seam | Evidence |
| --- | --- | --- | --- |
| A01 | Named-test eligibility, one-test startup proof, ordered environment edits and reaping still discriminate failures. | Existing test-process integration cases and controlled invoker/output seams. | C02 |
| A02 | PATH aliases receive exact native operands and binary streams without harness bytes; reported PID equals native child handle. | Existing native backend invocation plus the prepared fixture. | C01, C08-C10 |
| A03 | Concurrent installation succeeds without inherited writable executable handles; invalid selectors and missing/stale artifacts fail before effects. | Installer and preparation public operations, observing files/process results. | C01-C03 |
| A04 | Prepared, filtered and archive-remapped runs, including spaces/Unicode paths, use the explicit artifact with one build per preparation. | Existing tooling setup, archive and nextest environment operations. | C03 |
| A05 | Native setup preserves modes, file-size errors, restored limits, session identity and exact HOME/PATH; parent is unchanged and setup failure suppresses the body sentinel. | Real isolated-child callers; no shell-API text snapshots. | C04 |
| A06 | Every converted fixture retains its owning semantic assertions, real descendant topology, readiness/completion and cancellation/EOF distinctions. | Existing product-facing test modules and their shared-helper readers. | C05-C17 |
| A07 | The startup case passes 200/200 under recorded full-core load with startup source unchanged. | Named application lifecycle test, one-off qualification receipts. | C01, C05 |
| A08 | Current-main and open-change sweeps leave no controlled interpreted child/setup; every retained interpreter or string hit has an exact path and reason. | Source/caller sweep plus execution assertions; not a grep count alone. | C01, C05-C18 |
| A09 | Browser setup port preserves manifest/lock semantics and existing smoke/export results. | Existing browser-driver issue's Rust preparation operations. | C18 |

Write changed tests first and save their failure and success, or save a failed-and-restored mutation. Establish the new Rust child's assertions before deleting its old producer. Test externally visible results, not source prose or helper implementation. Replace fixture pacing and busy waits with acknowledged release; do not add or raise sleeps, retries, timeouts or limits. Keep changed tests below 30 seconds and the existing 120-second hang limit. Deterministic scratch roots, scripted providers and completion signals remain mandatory.

Run the repository check recipe, each changed test module, affected crate tests/doctests and reverse dependents through the approved build environment. Shared CI retains line coverage of at least 80% and native Linux, Windows and macOS checks. Compilation alone does not qualify native signals, executable copying or platform suffix handling. Both review axes must approve the delivered code and docs.

## Out of scope

1. Product behavior or protocol changes, a changed RPC startup window, new runtime/compiler mechanisms or a second test-support owner. **Proven scope:** #3768.
2. A browser-automation library choice or the rest of the browser driver port; the existing driver issue retains both. **Proven ownership:** #3868.
3. Creation-mode publication work, unrelated component lint work or rewriting another open change's setup from its unlanded branch. Consume their landed results without duplicating their ownership. **Proven ownership:** #3871, #3870 and #3589.
4. Root README design, changelog edits, new dependencies or speculative compatibility formats. **Proven repository constraints:** contribution rules and the existing foundation contract.

No useful qualification capability is omitted or deferred without its existing owning issue. Interpreters, generated programs and a separate recipe launcher are implementation mechanics, not product capabilities to reproduce.

## Further notes

### Documentation and glossary

**Confidence: proven documentation requirement. Evidence: `AGENTS.md:104-120`, `CONTEXT.md:267` and C02-C03.**

Update the foundation test-process contract and crate row, the development child-process guide, architecture ownership and the process-library record. Document the new installer and native setup with line rustdoc, errors and executable examples where useful. Explain quick start, choosing checked versus raw children, artifact preparation, typed setup, platform premises and troubleshooting missing preparation.

Extend the existing **Test process** definition in `CONTEXT.md` to include checked reruns, direct native fixtures and isolated native setup. Remove its stale std-only claim. Add no glossary synonym. Review `AGENTS.md`; no frozen rule change is requested. Any necessary local instruction change follows the existing authority rather than rewriting rules. Humanize every edited document in Maestro's voice.

### Open implementation questions

**Confidence: unknown until qualified.**

1. Typed native setup is exported as `maestro_test_process::IsolatedSetup`, with `TestProcess::setup` carrying the request and `apply_requested` applying it in the child. The installer is `maestro_test_process::install_executable(artifact, destination)`.
2. Native copying, archive path remapping, platform executable suffixes, signal handling and current-child file-size errors require actual platform evidence. A failed premise or library mismatch stops the affected delivery for a decision; it never authorizes an interpreter fallback or weaker assertion.
3. Completion requires a sweep of then-current main and every open pull request. No static inventory or source graph is assumed complete across generated code and platform configurations.

### Public source evidence

These Maestro citations prove the contract or what existing tests inspect, not that the replacement passed. All entries are **proven in source** or explicit issue requirements. The lanes retain exact external provenance separately.

| ID | Source or requirement |
| --- | --- |
| C01 | #3768: Rust controlled children, one existing owner, native PATH operands, unchanged startup and loaded qualification. #3868 retains browser-driver ownership. |
| C02 | `crates/maestro-test-process/src/proof.rs:8-35`; `src/test_process.rs:80-110,268-280,419-493`; [existing owner contract](maestro-port.md#test-process-owner). |
| C03 | `crates/maestro-tooling/src/test_setup.rs:1-16,45-100`; `src/test_setup/plan.rs:1-73`; `src/nextest_setup.rs`; `.config/nextest.toml`. |
| C04 | The owner's own setup cases in `crates/maestro-test-process/tests/checked_runs.rs`; `crates/maestro-app/tests/session_account_file.rs`; `tests/support/startup.rs`; `crates/maestro-chat/src/interactive_mode/tests.rs`; `crates/maestro-lock/src/replace/tests.rs`; `crates/maestro-settings/tests/settings_reload.rs`; component `tests/compilation_cache.rs`. |
| C05 | `crates/maestro-app/tests/support/rpc_peer.rs:11-73`; `tests/rpc_client_lifecycle.rs:162-219`; complete lifecycle/events/transport modules and `src/rpc_transport/tests.rs:42-68`. |
| C06 | `crates/maestro-app/tests/clipboard_copy.rs:218-259`; `tests/support/clipboard_image_effects.rs:219-231`; `tests/footer_metadata.rs:195-225`; `tests/footer_watch.rs:970-1005,1196-1235`; `tests/session_bash.rs:188-208`; `tests/package_command_execution.rs:31-125`; `tests/support/native.rs:29-39`. |
| C07 | `crates/maestro-chat/tests/login_support/native.rs:11-75`; `tests/extension_editor.rs:289-292,565-567,1137-1145`; `tests/support/session_picker/mutations.rs:1409`; `src/interactive_mode/tests.rs:1398-1404` and its actions/authentication/autocomplete/models/notices/share/lifecycle/suspend modules. |
| C08 | `crates/maestro-tools/tests/command_execution.rs:83-91,193-198`; `tests/shell_tree_cleanup.rs:28-57`; `tests/find_execution.rs:92,497,559`; `tests/find_support/mod.rs:358-382`; `tests/grep_support/mod.rs:128-142,468-476`; `tests/search_tool_lookup.rs:20-35,289-300`; `crates/maestro-tui/tests/fixtures/native_attachments.rs:164-175` and all four completion consumers. |
| C09 | `crates/maestro-extensions-wasm/tests/support/context_bridge/exec.rs:183-205`; `crates/maestro-extensions-wasmtime/tests/exec_io.rs:80-90,199-205,278-301,446-493`; hook/cache/minimal-mode consumers. |
| C10 | `crates/maestro/tests/native_io.rs:86-591`; `tests/native_io/faults.rs:214`; `tests/native_io/reload.rs:112-113`; `tests/entry_routes.rs:260-306`. |
| C11 | The terminal RPC client's checks live in maestro-packages (`examples/rpc_extension_ui.rs` and the [RPC guide](../rpc.md)); Maestro keeps the native peers they run against. |
| C12 | `crates/maestro-tooling/tests/author_setup.rs:541-559,905-969,1052-1060`; `tests/fixtures/recipe_support.rs`; the `tooling-command` and `tooling-wrapper` families of `crates/maestro-test-process/tests/support/native`; browser/setup/recipe consumers. |
| C13 | `crates/maestro-packages/tests/cache_support/native/mod.rs:11-28`; `tests/candidate_inspection_checks.rs:169-176`; `tests/capture_context_checks.rs:34-52`; `tests/git_staging_checks.rs:1042-1056`; native Git and package-manager unit consumers. |
| C14 | `crates/maestro-models/src/providers/content/google_shared/node_auth/tests/adc.rs:423-434`; `tests/support/profile_files.rs:24-39,60` and every process-profile consumer. |
| C15 | `crates/maestro-daemon/tests/desktop_notification.rs:20-26,72-74`; `tests/repository_discovery.rs:365-380`. |
| C16 | `crates/maestro-test-conventions/tests/support/mod.rs:181-185,229-272`; `tests/architecture.rs:1164`; retained non-executed rejection input in `tests/config_scripts.rs:16-23`. |
| C17 | `crates/maestro-process/src/children/tests.rs:184,315,604`; shell/PTY/capture modules; `crates/maestro-detection/tests/live_foreground.rs:13-48`; `crates/maestro-cli/tests/startup_stdout.rs:35-42`; `crates/maestro-credentials/src/credentials/tests.rs:183-217`; RPC native-signal cases. |
| C18 | `crates/maestro-tooling/src/browser_export_setup.rs`; `crates/maestro-test-browser/src/browser_driver/package.rs`; browser preparation and export qualification in #3868. |
