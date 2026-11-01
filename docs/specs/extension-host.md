# Component host capabilities

## Problem

Extensions need the user's filesystem, environment, standard streams and native asynchronous operations. Import registration alone does not prove that a loaded component can use them. File saves also need secure creation and atomic reader visibility, and interactive output must use the terminal's existing ordered queue.

This specification completes the [component extension contract](maestro-port.md#r07--one-component-extension-system) and [guest network and processes](packages.md#guest-network-and-processes). It preserves working capabilities while completing native file operations, shipped output routing and real-host qualification.

## Solution and user stories

1. As an extension author, I can use paths outside the working directory and inherit the operating environment, so installed extensions can reach the same resources as the user.
2. As an extension author, I can save private bytes and inspect or change native modes through host files, without implementing a publisher in my component.
3. As an interactive user, I can load an extension that writes stdout without splitting a terminal frame or losing native I/O.
4. As an extension author, I can retain native HTTP, child, watch, provider and OAuth resources after setup returns, with ordered callbacks and independent cancellation.
5. As a maintainer, I can distinguish real platform evidence from source inspection before claiming a capability is qualified.

## Evidence status

Each capability below has a proven contract, cited to Maestro's existing specification, guide or source owner. "Proven" describes the contract or inspected implementation, not a passing runtime result. Runtime qualification is unknown until the acceptance observations are recorded. Source proposals and closed issues are not substitutes for those observations.

## Capabilities kept

### Operating environment

| Item | Contract and acceptance observation | Confidence and public evidence |
| --- | --- | --- |
| S01: native paths | Both standard generations read and write exact bytes by absolute and parent-relative paths outside cwd, follow allowed file/directory links and later target updates, and retain ordinary native errors. Compare native controls with both guest paths, including an absolute intermediate directory link below a search-only parent. Establish that native known-leaf reading succeeds while listing its parent fails. A privileged bypass does not qualify that premise. Keep native Windows drive/share, junction and cwd behavior. | Proven contract; combined-link and platform runtime results unknown. [Operating environment](../extensions/host_io.md), runtime `wasi::root`. |
| S02: files and modes | Host files save empty/binary bytes atomically, create a requested 0600 file without a wider exposure window, preserve an existing 0640 rewrite mode, create a 0700 directory, read/change native file/directory modes and accept native-valid 255-byte leaves. Follow the existing final-link policy, including missing link targets. Failed precommit leaves prior bytes/mode intact; postcommit durability warnings remain distinguishable. Native permission semantics must be qualified on each platform, not replaced with invented POSIX bits. | Proven requested contract; delivery and platform results unknown. [File publication](../locking.md), application `extension_native_io`, component extension contract. |
| S03: environment | Inherit ambient entries, including empty values, without a filter. An isolated child host receives a nonsecret sentinel; actual guest standard-library lookup returns it unchanged. Keep documented native-string conversion and native cwd, home and platform observations. | Proven implementation; fresh execution unknown. [Operating environment](../extensions/host_io.md). |
| S04: artifact-relative assets | Each instance observes its selected artifact path through `MAESTRO_EXTENSION_PATH`, even with a conflicting inherited value or unrelated cwd. Copies loaded from two locations and reload resolve their own adjacent assets. This does not promise heap persistence across reload. | Proven implementation; fresh execution unknown. Runtime `wasi::build`, `lifecycle::State::new`, [operating environment](../extensions/host_io.md). |
| S05: separate standard streams | Actual guest reads supplied stdin and writes exact binary sentinels to separately captured inherited stdout/stderr. Check EOF and explicit flush. Print, JSON, RPC and synchronous regular-file destinations retain stdout; do not suppress or redirect guest output to protect a protocol. | Proven implementation; fresh execution unknown. [Supplied stdout](../extensions/host_io.md#supplied-stdout), runtime `wasi`. |

### Terminal and native effects

| Item | Contract and acceptance observation | Confidence and public evidence |
| --- | --- | --- |
| S06: interactive stdout | While queued terminal output runs, each component write enters the same ordered ingress whole, between complete host frames. A retained writer follows startup, stop, suspend/resume and restart; stopped or synchronous destinations use inherited stdout. Test the shipped executable's initialization, reload, failed startup and final drain while native exec still works. Both output generations share the supplied sink. | Proven requested contract and existing queue; shipped composition unknown. [Guest network and processes](packages.md#guest-network-and-processes), terminal `output`, executable `host::module_loader`. |
| S07: network | Share inherited networking and name lookup across standard 0.2 and asynchronous 0.3. Real guests resolve, connect, listen and accept against owned loopback endpoints. Demonstrate truly pending I/O and independent Store progress, not only successful linking. | Proven implementation; fresh execution unknown. [Operating environment](../extensions/host_io.md), runtime `wasi` and `lifecycle`. |
| S08: HTTP | Use the existing host HTTP client, preserving request options, absent versus present-empty bodies, ordered normalized headers, status, non-success responses, bytes and existing proxy/TLS/environment behavior. One retained body cursor serves read/text/JSON and aliases. Local acknowledged chunks prove pending delivery, preheader/midbody errors, reentrant getters and consumer close. No new wire-header ordering guarantee is implied. | Proven contract and implementation; fresh execution unknown. Component extension contract, application `extension_native_io::http`. |
| S09: child resources | Literal argv, optional cwd/environment, independent stdin/stdout/stderr and code/signal/error observations reach the existing process owner. Omitted environment inherits; supplied empty environment replaces it. A detached controlled child survives callback return and actual extension reload and is reaped by the same collector. Pending input, end ordering and separate partial output remain observable. Host-shutdown survival is not promised. | Proven implementation and existing qualification source; fresh execution unknown. [Detached children](../extensions/host_io.md#detached-children), application `extension_native_io`. |
| S10: explicit exec | Keep explicit exec separate from raw spawn and shell-tool presentation. Preserve captured/supplied cwd, optional positive millisecond timeout, cancellation, separate partial stdout/stderr and code/killed/error distinctions. A resistant child and descendant must actually terminate and be reaped; sending a signal is not evidence of exit. Add no new timeout or supervisor. | Proven contract; fresh execution unknown. [Tools](maestro-port.md#r05--tools-and-process-operations), tools `process::exec`, runtime `exec`. |
| S11: watches | Existing native watch registrations survive setup, report semantic create/modify/rename/remove changes, and close/release at their original owner. Prove setup failure and eventual last-drop release. Do not promise portable notification counts or a new event order. | Proven implementation; fresh execution unknown. Application `extension_native_io::Operations::watch`, native watch owner. |

### Providers, callbacks and domain effects

| Item | Contract and acceptance observation | Confidence and public evidence |
| --- | --- | --- |
| S12: resource lifetimes | Retain permitted owned hooks/signals/body/output leases after initiating callbacks return. Prove a truly pending producer, ordered delivery, native transport demand, producer-tail completion and exact-once release. Dropping an observer, closing a consumer and cancelling a caller are distinct. Consumer close stops its native input/producer, never the caller's turn signal. Add no event capacity. | Proven contract and implementation; fresh execution unknown. Component extension contract, runtime `local_resources`, application HTTP body owner. |
| S13: provider delegation | Providers register callable streams and can select a concrete backend without changing model ID/provider/API or recursively dispatching through their own registration. Preserve context, source/usage/cost/timestamp fields and the gateway's deliberate base-URL, API-key and header overrides. Test both selected protocols, pending events, setup and later failures. Reuse the existing model registry and protocol-selection delivery. | Proven contract; explicit-selection delivery/runtime unknown. Component extension contract, runtime provider bridge, model `records::stream`. |
| S14: request hooks | Await each applicable payload/response callback in order. Every present payload replacement, including null/false/zero, replaces; absence does not. On provider paths that invoke response inspection, it finishes before authored body/event processing. Keep provider-specific invocation rules, reentrant getters and caught extension-event errors distinct from failing low-level provider-option hooks. Failures after invocation become the proper error/aborted stream result. | Proven contract and implementation; fresh execution unknown. [Model contracts](maestro-port.md#r01--models-providers-and-authorization), runtime `model_calls` and `provider_controls`. |
| S15: OAuth | Retain callable interaction/login/refresh/key/model callbacks and original credential ownership. Reuse the existing model-owned PKCE primitive through one shared guest projection: 32 entropy bytes, unpadded URL-safe verifier and SHA-256 challenge of verifier text. Test known vectors, entropy/error forwarding and retained callbacks. Gateway flow/cache stays package-owned; its pasted callback needs no local listener. | Proven contract; shared projection delivery/runtime unknown. Component extension contract, model `oauth::pkce`, guest `oauth`. |
| S16: domain forwarding | Session/model/tool/UI calls reach their original application/domain owners. A loaded plan-mode component toggles tools, blocks a disallowed bash call before execution, tracks completion, restores current-plan progress from the real session and updates existing UI. Its configurable default shortcut is Ctrl+Alt+P. Plan state stays guest-owned; persistence stays session-owned. Plan mode has no direct dependency on the native save or stdout work. | Proven dependency/behavior contract; application workload qualification unknown. Component extension contract, runtime API/context/history/UI projections, [plan review](packages.md#p12-plan-review). |
| S17: fault isolation | Retain registrations, UI and subscriptions after an actual component fault; attribute each later refused call without re-entering the poisoned instance, including destructors. Healthy dispatch continues and native leases release once. Prove startup fault, pending work and later command/UI/subscription use. | Proven contract and implementation; fresh execution unknown. Component extension contract, runtime `factory`, `lifecycle` and `replies`. |

## Implementation decisions

### One files publisher

The complete save path is guest files facade -> canonical WIT -> existing typed imports/resource admission -> domain native-I/O port -> application files -> `maestro_lock::atomic_replace`. Standard native calls implement mode inspection/change and directory creation. Authored paths remain strings until application I/O. Blocking publication runs outside the local pump.

First extend the existing atomic operation with `Option<u32>` creation mode as its fifth argument. Existing workspace/detection callers pass `None`, keeping their default creation, final-link and temporary policies. Application saves pass the requested mode, follow final links and use the existing unique temporary policy. Reuse the lock owner's bounded adjacent-name function. Publication, exclusion, temporary cleanup and committed-warning handling stay lock-owned.

Recoverable in-place `replace` remains a separate operation for its existing callers; it preserves native identity but does not promise whole-file visibility to unlocked readers. Atomic publication replaces the directory entry and keeps its documented hard-link/ownership/ACL differences. Do not globally change intentional in-place callers.

Evidence: proven existing split in [locking](../locking.md), atomic publisher and workspace/detection callers; requested-mode delivery is unknown. The native application-to-lock edge already exists. No new crate, library, feature or dependency edge is required.

### One terminal admission path

Components run on the existing dedicated Store thread. Extend the existing native-I/O factory to accept optional supplied stdout beside native effects, within five arguments. The executable constructs the native terminal before loading extensions and supplies its lifecycle-following standard writer. The runtime sees `Write + Send`, never a terminal type. Keep the terminal queue local and reuse its sendable admission handle.

The terminal alone chooses current admission or inherited stdout and serializes transitions with foreign writes. Keep existing active-generation `OutputWriter` closure errors: the new lifecycle-following view does not revive a retired sender. Flush proves admission, not physical delivery; stop drains accepted writes. No second queue, global stdout interceptor or writer task is introduced.

Evidence: proven Store-thread/queue ownership in runtime `factory` and terminal `output`; shipped lifecycle qualification is unknown.

### One canonical asynchronous contract

Combine file additions with existing factory/package/settings operations in the same canonical WIT, generator, imports and pump. Change generator inputs/logic, not generated output. Preserve current HTTP/process/watch/provider/OAuth resources, callbacks, leases and cached-fault behavior. Backend selection and the shared PKCE projection land once in their existing owning work; neither is reimplemented here.

Evidence: proven owners in the component extension contract and current domain/runtime modules; integrated runtime observations are unknown. Keep the engine adapter outside the browser dependency closure and retain current approved manifests/toolchain pins.

## Testing decisions and success criteria

Use existing unit seams for immediate publication effects and the single `tests/integration.rs` program in each affected crate for public behavior. Real authored components use the existing author preparation and supplied adapters. Native controls establish filesystem/platform premises; controlled providers do not prove actual application model/session selection.

| Checkpoint | Required evidence |
| --- | --- |
| Atomic publisher | Before-write creation-mode observation, unchanged existing modes/default callers, complete old/new bytes for unlocked readers, native-valid long names, final links, failed precommit preservation and committed-warning distinction. A final stat alone cannot prove secure first creation. |
| Native files | The real component performs the complete S02 operation set through the shipped adapter, alongside existing native effects and factory operations. No recorder or source inspection substitutes for execution. |
| Shipped output | Whole guest escape/binary writes between frames through actual executable startup/reload/suspend/resume/drain, both output generations, retained handles and concurrent native exec; separate inherited-mode controls. Test children and acknowledgements are Rust. |
| Capability reconciliation | Map S01-S17 to named distinguishing assertions and saved receipts. A single component runs 0.2 standard-library basics beside a genuinely pending 0.3 operation. Retain existing HTTP/process/watch/provider/OAuth/fault regressions and final workload-owned application tests. |
| Platform matrix | Record Linux/macOS/Windows command, host premise, case, assertion, result and log/CI link. Mark unavailable denial/share/volume/mode premises unqualified, not passed. Reuse existing native jobs; cross-compilation is not runtime evidence. Keep the Windows replacement/share discrepancy open until actual controls settle it. |

Contract confidence is proven by the cited owners and S01-S17; results remain unknown until recorded. Each changed test has saved failing and passing runs, or a failed-and-restored mutation. Already-passing unchanged reuse cases need no fabricated failure. Tests wait on explicit progress/completion, not sleeps, polling, timed receives or new deadlines. Changed tests stay under 30 seconds and use test-created storage/endpoints/programs, never real credentials or paid providers.

Run `just check`, changed crate unit/integration tests and reverse dependents, `just test`, strict rustdoc and existing `just ci` through the approved build launcher. Use `CARGO_BUILD_JOBS=3`, the pinned toolchain and ordinary nextest setup. Long runs keep logs and exit files. The existing 80% line-coverage gate applies; test-only qualification cites production owners' evidence rather than inventing a denominator. Both required reviews check source-backed scope, graph placement, tests and documentation.

## Corrections and mechanics not copied

| Correction | Existing owner and acceptance |
| --- | --- |
| A sent termination signal is not an exited child. | Process owner; resistant child/descendant actually terminate and reap, with separate partial output. |
| OAuth callback state must be present and match the flow. | Gateway package; reject missing/mismatched state before token exchange. |
| Direct-access reuse must be keyed by credential. | Gateway package; changing credentials cannot reuse another credential's token. |
| A private file must not expose bytes before chmod. | Atomic publisher; observe restrictive temporary creation before any bytes, preserving rewrite mode. |

These corrections do not introduce second owners. Their contract evidence is the requested native outcomes and existing process/files/OAuth owners; fresh results remain unknown.

Do not copy foreign module-loader/event-loop behavior, engine/string/stack limits, broken chunk character decoding, engine/library internals or OS notification multiplicity. Native async and existing libraries supply those mechanics. Exact bytes, authored callback order, explicit timeouts, completion and ownership remain required. No useful capability is omitted.

## Documentation and glossary

Update this contract, the existing package/component contracts, [host-I/O guide](../extensions/host_io.md) and [locking guide](../locking.md) in the owning change. Feature pages open with a quick start, then reference and examples. Document every Rust item, private included. Run the humanizer on edited prose in Maestro's voice before commit.

Add one glossary term when the atomic prerequisite lands: **Atomic publication:** lock-owned temporary-file publication for unlocked readers to see complete old or new bytes; distinct from recoverable in-place publication. Existing native-I/O, runtime-adapter, PKCE and output-backlog terms remain unchanged. Review `AGENTS.md`; no rule or graph permission changes, so record why no edit is needed. Do not edit README or CHANGELOG.

## Assumptions, boundaries and open questions

The source and proposed ownership are proven; runtime receipts, coverage and complete platform parity are unknown. The combined search-only/absolute-intermediate-link case is a qualification spike, not an assumed filesystem redesign. If it fails, capture native and guest results before fixing the existing root owner. A mechanism unable to meet the contract needs a decision; an unsupported-link exception is not acceptance.

Always preserve current native access and working effects. Ask before a new library/feature, policy, graph edge or capability reduction. Never add grants, path/address allowlists, environment filters, stdout suppression, fuel/memory caps, default deadlines, compatibility versions or another loader, store, publisher, supervisor, HTTP client or pump.

Out of scope: implementing the game engine/assets, gateway package, sandbox package or plan-mode workload; adding an OAuth listener to a pasted-callback flow; repairing graph tooling; adding a platform workflow. Their existing owners retain delivery and final application qualification. No capability is deferred without its existing tracked owner.
