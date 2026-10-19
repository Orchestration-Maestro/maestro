# Workspace daemon

The [daemon specification](specs/workspace-daemon.md) defines the application.
The transport leaf described here provides synchronous Unix sockets; application
wiring and environment selection are tracked in [path composition](https://github.com/Orchestration-Maestro/maestro/issues/1441).
The common command records live in [daemon protocol](daemon-protocol.md).

## Sockets

`maestro-ipc` owns native socket effects and consumes supplied paths. It reads no
environment variables and chooses no configuration root. Both transports use the
shared [JSONL codec](json.md#jsonl-framing); socket code defines no command set.

`socket_paths::client_socket_path` selects, in order:

1. The caller's explicit named-daemon client path, even when empty.
2. The client path derived from the supplied API socket override
   (`MAESTRO_SOCKET_PATH` at the application boundary).
3. The supplied client override (`MAESTRO_CLIENT_SOCKET_PATH`), even when empty.
4. `daemon-client.sock` inside the supplied daemon folder.

Derivation uses the API path's native file stem and parent: `api.sock` becomes
`api-client.sock`, `a.tar.gz` becomes `a.tar-client.sock`, and a path with no usable
UTF-8 stem uses `maestro-client.sock`. Relative paths stay relative; selection does
not canonicalize them. Socket filesystem operations resolve relative paths when
called.

Before binding, preparation creates parents, rejects a live listener with
`maestro server is already running (socket busy at {path})`, and removes stale
socket entries. A stale connection failure does not authorize deletion of a
regular file, directory or symlink: preparation preserves that entry and returns
`AlreadyExists`. A link to a live listener takes the live-listener branch.
Unclassified errors propagate unchanged.

Binding disables automatic pathname removal on listener drop. The caller records
`SocketFileIdentity` and can remove the pathname if its current device and inode
still match. A missing path succeeds. This metadata comparison followed by unlink
is not atomic; callers must coordinate pathname replacement with cleanup.

After binding, callers restrict the socket to owner read/write mode (`0600`)
through `socket_paths::restrict_socket_permissions`. Private listeners also need
this post-bind call on Unix. Write shutdown leaves the read direction usable.
Only an actual end-of-input read ends incoming traffic; pending command bytes are
not consumed by a disconnect probe.

Windows transport is tracked in [Windows sockets](https://github.com/Orchestration-Maestro/maestro/issues/1473),
and other Unix qualification in [Unix qualification](https://github.com/Orchestration-Maestro/maestro/issues/1472).
See [socket libraries](libraries/workspace-daemon.md) for dependency features and licences.

## Window-title templates

`maestro_daemon::settings::window_title` parses the default
`{hostname}: {workspace}` format and the `hostname`, `workspace`, `tab`, `pane`
and `terminal_title` tokens. Doubled braces are literal braces; whitespace inside
a token is ignored. Empty formats disable titles. Unknown tokens and unbalanced
braces return diagnostics rather than a partial template.

Title sanitation removes control characters, keeps the first 200 Unicode scalar
values, then trims surrounding whitespace. An empty result is absent. The library
provides parsing and sanitation; applying titles to the outer terminal remains a
runtime responsibility.

## Topology

`maestro-workspace` models workspaces, tabs, split layouts and pane viewport
state. A `WorkspaceTopology` owns the collection and one identity allocator.
Its clock is supplied by the caller; separate owners have independent counters.

Workspace handles and public tab and pane numbers stay stable when tabs move.
Closed numbers are not reused. Restore admission reserves saved numeric
identities and public counters before subsequent allocation, and assigns fresh
terminal identities.

Layouts keep a binary split tree, ratios and focus history. Closing the selected
pane returns to the remembered live pane, or a neighbouring leaf in tree order.
Resizing uses the nearest eligible divider, with an opposite-edge fallback.
Geometry uses cell rectangles without frontend decorations.

Creation and splitting take a consuming spawn callback. The callback receives
owned workspace, tab, pane and terminal identities; its result returns directly
to the caller without a thread-safety or cloning requirement. A failed split
restores the layout, focus history and viewport state. Failed tab creation keeps
its reserved tab number but does not consume a public pane number.

Pane transfers return their original state when insertion fails. The source
keeps its public pane mapping until the caller explicitly unregisters it. Taking
the sole pane can leave an empty source workspace for the caller to remove or
refill.

Repository status and native identity paths arrive as data. Labels use supplied
HOME and checkout observations without reopening paths. Metadata expiry is
explicit: callers supply an instant and request a sweep; reading values does
not expire them.

The topology API starts no terminal, background task or daemon listener. See the
[daemon design](specs/workspace-daemon.md) for the remaining runtime boundaries.

## Snapshot

[`maestro_workspace::persist`](../crates/maestro-workspace/src/persist/mod.rs)
loads and saves version-1 topology at supplied paths. Owned records retain
workspace/tab/pane names, public numbers, selection, working directories, split
axes and resume provenance. Screen history is stored separately and can be
removed by saving without history.

Saving writes pretty JSON without a trailing newline through the shared
[atomic replacement](locking.md#atomic-replacement) operation, following symbolic
links. Topology commits before history is saved or removed; failure on the second
file does not undo the first. Snapshot saves ignore postcommit directory-sync
advisories.

Missing files load silently as absent. Read, parse and unsupported-version failures
produce warnings and return no snapshot. Only version 1 is accepted; there is no
legacy reader or conversion. Topology classifies its raw header before decoding
workspaces, while history decodes its complete record before checking its version.
Unknown record members are ignored. Capture, restore and default-path selection
are separate operations, not part of these supplied-path functions.

## Pane processes

The Linux process layer provides owned terminals through `maestro_process::pty`.
A pane's caller supplies its executable, literal arguments, working directory,
environment policy and setup entry. The child becomes its terminal's session leader;
the master supports foreground-group queries and window-size changes.

The I/O actor owns the master. Input waits for capacity, while terminal replies and
geometry updates remain independent of that queue. During handoff, accepted input
drains before reads pause. Only a quiesced actor can duplicate its master. Rollback
resumes it; successful committed release closes the old owner without closing an
owned duplicate. An optional asynchronous exit hook finishes before the old master
is released. Graceful shutdown must keep draining critical notifications until
pending hooks finish, before destroying the runtime. The
[daemon loop delivery](https://github.com/Orchestration-Maestro/maestro/issues/1445)
owns that integration; destroying the runtime cancels outstanding hooks and releases
their terminal resources.

Shell builders retain login-shell custom commands, plain pane commands and
shell-evaluated scrollback editor selection. Foreground observations contain process
facts rather than application classifications. Application wiring is described by the
[workspace daemon contract](specs/workspace-daemon.md).

## Pane agents

[`maestro_detection::agents`](../crates/maestro-detection/src/agents.rs) owns the
22-entry agent catalogue, canonical labels, aliases and interactive executables.
Canonical lookup is strict; alias lookup trims whitespace, ignores case and
removes one recognized executable suffix.

On Linux, `identify_agent_in_job` consumes supplied foreground facts from
`maestro-process`. A recognized process-group leader wins. Otherwise wrapped
identities rank ahead of native names, and the first equal-priority member wins.
Recognition includes runtime script operands, supported package entry points and
symbolic-link targets. Inline code and module operands do not become script names.

`AgentState` reuses the protocol's four-state `PaneAgentState`. `AgentDetection`
carries screen confidence, with independent indicators grouped in
`AgentVisibility`. The catalogue distinguishes lifecycle-authoritative integration
pairs from identity-only pairs. Screen rules provide live evidence; completion
state remains a daemon decision. Authority predicates accept canonical `maestro:`
provenance. Call
`normalize_agent_source` at a retained report boundary to translate a legacy
provenance prefix without changing other source text.

## Agent state

`maestro-daemon::terminal::state::TerminalState` retains one pane's detected
agent, hook authority, session identity, managed launch and presentation. Detection
and integration reports feed one effective-state recomputation. Live lifecycle
integrations own state while matching foreground evidence remains valid; other
hooks can yield to fresh visible blockers. Process exit clears matching authority
before recomputation. Replacement sessions need the integration's recognized
start reason and fresh process evidence; stale and buffered reports remain tied
to their source and generation.

Retained hook, session and metadata inputs normalize provenance once. Clear and
release lookups use the same normalizer, so legacy and canonical source spellings
share ownership and sequence freshness. External agent labels stay unchanged.

`terminal::metadata` applies presentation overrides without changing execution
state. Title, display agent and state labels have independent precedence times.
Source and agent guards hide unrelated reports. Expiry is explicit: callers read
the next deadline and apply an expiry observation; the state owner starts no
timer. Manual labels are trimmed, while agent names retain their spelling and
session ownership. Managed launches use caller-supplied settling and timeout
values, releasing their names on mismatch or process exit.

`maestro-daemon::terminal::title::stripped_terminal_title` trims a terminal title
and removes one recognized leading activity glyph only when whitespace or the
end of the title follows it. Other symbols and Unicode text stay intact. An empty
result has no presented title. Raw-title changes are tracked separately from
normalized presentation changes; only the latter advance the wrapping revision.
Windows elevation-title handling is tracked in
[Windows qualification](https://github.com/Orchestration-Maestro/maestro/issues/1473).

`maestro-daemon::terminal_theme` retains default foreground/background colours
and palette entries. A palette alone does not make the default-colour theme
nonempty. Background luminance determines light or dark appearance when the host
has not supplied one. The module builds colour-scheme reports and default-colour
set/reset sequences. Host colour queries and reply parsing have one owner,
`maestro-tui::terminal_theme`; consumers reuse its RGB and default-colour records.

## Resume plans

`maestro-daemon::agent_resume` validates official integration session references
from reports or snapshots. Identifiers contain at most 512 UTF-8 bytes; absolute
paths contain at most 4096. Both reject empty values and control characters.
Path-capable integrations prefer a valid path, falling back to a valid identifier.
Other integrations accept identifiers only.

Snapshot admission retains canonical source provenance and rejects a source that
does not match its agent. Resume plans contain the external executable and literal
argument strings, never a shell command. Their deduplication key separates source,
agent, reference kind and value with NUL bytes. Planning starts no process and
performs no filesystem lookup.

## Detection manifests

[`ManifestCache`](../crates/maestro-detection/src/manifest/cache.rs) loads the
20 embedded detection documents. Callers supply optional override and catalog
folders; detection does not discover global configuration paths. A valid local
`<agent>.toml` override takes precedence over a compatible download in
`remote/<agent>.toml`. Downloads older than the bundled version are ignored.
Invalid external documents fall back with an explain warning.

Detection uses retained compiled rules until `reload_manifests` is called.
Separate cache instances keep separate source roots. Rules select screen, title
or progress regions and combine substring and regex gates by priority.
`detect_agent_with_osc` returns state, live visibility and the transcript
suppression flag. `explain_with_input` also returns source versions, evaluated
rules and the first 240 characters of each evidence region, with `...` when
truncated. Explain JSON preserves
absent fields as null. A known agent without a matching rule falls back to idle
without live-state confidence; an absent agent returns unknown.

## Detection catalog

[`manifest_update`](../crates/maestro-detection/src/manifest_update.rs) checks a
caller-supplied catalog URL through a supplied fetch operation. Requests carry a
256 KiB response limit, two retries, a five-second connection timeout and a
15-second total timeout for the transport adapter to enforce. The engine rejects
oversized or non-UTF-8 responses before parsing them. It creates no network
client or background worker.

The catalog resolves safe relative document paths against its own URL directory.
Each agent updates independently: newer versions publish through the shared
atomic replacement owner, identical equal-version bytes are left alone, and
downgrades or changed bytes at the same version are rejected. `status.toml`
records outcomes under the supplied catalog folder. `auto_update` reloads the
supplied cache after a commit, before invoking the result callback. Daemon path
composition, scheduling, offline admission and the catalog-check setting remain
with the daemon caller.

## Input model

`maestro-daemon::input` owns decoded keys, committed text and host input events.
It reuses the protocol's key codes, phases and source records. A key keeps native
Windows evidence or raw VT bytes separately from its canonical phase. Release
builders clear generated text and reset grouped repetition to one without
rewriting that evidence.

`TerminalKey::as_key_event` supplies a semantic-only view: code, modifiers and
phase, with a synthesized source and no generated text. Moving a key into a
`ClientInputEvent` preserves its transport metadata. The wire has no shifted
code-point field; that value stays local. Ingress masks unknown modifier bits,
uses the wire repeat count over the native record's count and normalizes releases
last. A non-key event returns unchanged to its routing caller.

`input::mouse` accepts supplied host geometry without reading the terminal.
Host pixel coordinates are one-based; cells and pane rectangles are zero-based.
Pane pixel results are one-based and scaled with integer arithmetic. Report
parsing requires a complete numeric SGR report. Relocation preserves the button
text and terminator without imposing parsing's stricter numeric validation.
