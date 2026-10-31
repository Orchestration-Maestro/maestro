# Workspace daemon

The [daemon specification](specs/workspace-daemon.md) defines the application.
The transport leaf described here provides synchronous Unix sockets; application
wiring remains separate from its supplied-path and owned selection interfaces.
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
socket entries. Preparation inspects the entry in two cases only: a stale
connection failure (connection refused, not found or timed out) and a native
not-a-socket failure. Every other connection failure is returned unchanged and
nothing is inspected. Inspection reads the entry's type without following a link,
and when that entry is not a socket it preserves the entry and returns
`AlreadyExists`. A link to a live listener takes the live-listener branch.

The not-a-socket test follows the native error, because the general error kind does
not separate these cases on macOS. There an ordinary file reports a native
not-a-socket failure, which admits the entry inspection, and a datagram socket
reports a native wrong-protocol failure with that same general kind; preparation
keeps the datagram socket's exact native error and socket identity. On Linux an
ordinary file reports connection refused, so it is admitted through the stale case,
while a datagram socket still reports wrong-protocol. On both platforms the ordinary
file is preserved and preparation returns `AlreadyExists`. Only a stale connection
failure removes a socket.

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

## Retained daemon state

`maestro_daemon::app::state::DaemonState` retains one `WorkspaceTopology`,
terminal metadata, active and selected workspace positions, and historical pane
aliases. Its supplied clock feeds the topology allocator. Pane-size estimation
uses the first supplied pane rectangle, falling back to the configured headless
rows and columns before a view attaches.

Public target parsing distinguishes stable numbers (`w1:t4`, `w1:p2`) from
historical positional tabs (`w1:4`) and raw pane spellings (`p_3`). A stale pane
alias returns no match rather than selecting a different live pane. Current-only
pane lookup excludes historical spellings.

Workspace moves preserve active and selected workspace identities when display
positions change. Block moves use the caller's member order, reject duplicates
and missing members, and leave unchanged orders clean. Accepted moves mark the
session dirty. Notification context uses the resolved workspace label and adds a
tab label only for a live pane in a workspace with multiple tabs.

Focus navigation records workspace and pane identities, not display positions.
Switching workspaces or tabs marks the selected tab's panes seen and the session
dirty. Last-pane navigation toggles across workspaces and tabs; removed targets
are discarded. Copy navigation retains its pane while focus is elsewhere, clears
cross-pane selections, and returns to copy context when that pane is selected.

Closing a selected parent checkout removes its repository group; closing a linked
checkout removes only that workspace. Pane death removes only its own workspace
when its last pane exits, retaining the logical active and selected workspaces.
Removal clears extension provenance and affected copy/selection records. Pane
aliases remain recorded; resolving an alias to a removed pane returns no match. Detached terminal metadata is removed and its identity is queued for the
loop to shut down the existing runtime; still-attached terminals are retained.

Screen, process and hook reports update the existing terminal-state owner.
Reserved native reports retain session identity without overriding screen state.
Official release events leave process ownership intact; custom releases use the
terminal owner's release operation. Session-only changes mark persistence dirty
without inventing a visible update. Completion acknowledgement distinguishes
initial idle from finished work and respects active tab and outer-terminal focus.
State updates retain before/after presentation, release status and acquisition
suppression for the loop's consumers.

Background notifications can be immediate or delayed. Pending transitions cancel
when work resumes or their pane dies. Due delivery rechecks live state, identity,
focus and the root terminal's directory. Workspace toasts, client notifications
and sound decisions are independent; terminal delivery does not replace the
workspace toast. Host notification and sound effects remain with the loop.

Update-ready events retain the available version and the producer's install
command. Workspace delivery shows installation guidance; other delivery modes
leave any existing toast unchanged. Receiving an event does not acquire or install
an update.

`maestro_daemon::api::event_hub::EventHub` shares one sequence across clones and
retains the latest 512 ordered event payloads. Replay returns only events strictly
newer than the supplied sequence. Events use the shared type-tagged protocol
records without another envelope.

`maestro_daemon::key_combo` parses, formats and matches workspace key combinations
using shared key codes. It preserves named punctuation, Unicode characters,
modifier aliases and shifted-key matching; shifted Tab normalizes to BackTab.

## Snapshot

[`maestro_workspace::persist`](../crates/maestro-workspace/src/persist/mod.rs)
loads and saves version-1 topology at supplied paths. Owned records retain
workspace/tab/pane names, public numbers, selection, working directories, split
axes and resume provenance. Screen history is stored separately and can be
removed by saving without history.

Saving writes pretty JSON without a trailing newline through the shared
[recoverable in-place publication](locking.md#recoverable-byte-publication) operation, following symbolic
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
is released. Before destroying the async runtime, drain critical notifications
until pending hooks finish. Pane shutdown itself does not wait for event capacity. The
[daemon loop delivery](https://github.com/Orchestration-Maestro/maestro/issues/1445)
owns that integration; destroying the async runtime cancels outstanding hooks and releases
their terminal resources.

Shell builders retain login-shell custom commands, plain pane commands and
shell-evaluated scrollback editor selection. Foreground observations contain process
facts rather than application classifications. Their records are available through
`maestro_process::foreground` on every platform; native lookup remains Linux-only.
Application wiring is described by the
[workspace daemon contract](specs/workspace-daemon.md).

## Pane terminals

`maestro_daemon::pane::terminal::PaneTerminal` owns one concrete terminal engine
with a single lock for screen state and pane policy. `process_pty_bytes` returns
ordered child replies, title changes, bells, clipboard writes and working-directory
observations. Its foreground-job source is lazy: colour sets and primary-screen
history clears request it at most once per write, only when the shell PID is known.
Callers send replies directly to the PTY, independently of queued
user input. Capability and host-colour replies retain their position among engine
replies, including queries split across writes. Child colour overrides survive
host-theme refreshes until reset or a matching foreground-owner restore.

Resize retains bottom-follow and returns in-band size reports. Scrolling and mode
reads use live engine state. Synchronized output suppresses immediate redraws;
returned delays let the caller schedule image and cursor settling without a
pane-owned timer. Key and mouse encoding reuse the engine's encoders. Mouse reports
and converted non-character keys use live pane modes; character keys and semantic
fallbacks retain the caller's keyboard protocol. Keyboard handoff replays the
retained flag stack without the independent modified-key mode. Grouped key repeats
expand at the destination; releases encode once.

Visible, recent and detection reads use the engine's retained rows. Plain physical
reads join each row's cells, reading a blank cell or a whole image-placeholder
grapheme as one space; unwrapped and styled reads use the engine's text formatters.
Recent snapshots report whether the physical-row budget omitted older rows;
unwrapped variants join soft wraps without changing that budget. Retained
literal search handles Unicode case folding and complete graphemes. Word and WORD
motions use the same retained buffer. `with_render_view` refreshes the single
render snapshot and lends it under the pane lock, with the engine's default
palette, the host colours, the pane's initial default colours, the image setting
and the cursor presented at the supplied instant. Frame rendering stays with the
[multiplexer](multiplexer.md#rendering).

`maestro_daemon::selection::Selection` anchors and drags in absolute retained rows,
orders inclusive endpoints and clamps screen coordinates to the pane. A click
alone does not activate a selection. `extract_selection` reads those endpoints
through the engine; highlighting and clipboard policy stay with the caller.

Native Windows input fallback and recent-text qualification are tracked in
[Windows qualification](https://github.com/Orchestration-Maestro/maestro/issues/1473).

## Pane runtime

`maestro_daemon::pane::agent_detection` supplies screen-scan and publication
decisions. An identified idle pane can skip a scan when its content revision is
unchanged and no identity, exit or pending-idle transition requires a read. Plain
idle after working is held for three further observations or 700 ms; visible idle,
a changed identity or process exit bypasses that hold. Unchanged visible blocker
evidence is refreshed after 800 ms. The caller owns observation times and applies
the returned decision; these helpers do not start the detection task.

On Linux, `maestro_daemon::pane::detection_task::spawn_basic_detection_task`
starts fallback detection for an imported pane. The caller supplies the terminal,
child identity, content revision, compiled manifest cache and event queue. The
detector reports a newly identified process before a three-second startup grace
and clears the previous process's OSC evidence. Lifecycle activation resets
fallback tracking, while an explicit release suppresses reacquisition for one
second. The caller must cancel and join the returned task on teardown.

`maestro_daemon::events::AppEvent` carries process observations, screen state
changes, hook state reports and agent releases. Its async publisher waits for queue space rather than dropping these
transitions and pane death; a closed receiver produces a warning. Bell, clipboard
and cwd metadata from the reader use nonblocking delivery.

On Linux, `maestro_daemon::pane::PaneRuntime` owns the live terminal, child watcher,
I/O actor and optional fallback detector. `PaneRuntimeConfig` supplies geometry,
theme, manifest cache, event and render destinations, and a process-owned
`PtySetupEntry`. The entry executable must call `maestro_process::pty::setup_entry`
before its ordinary argument parser. The composition-root binary is separate from
these library interfaces.

Runtime constructors accept a configured shell, a shell command or literal argv.
History is seeded before live reads. Spawned detectors use a 50 ms initial delay,
then 500 ms for unidentified panes or 300 ms for identified panes; release or
transient child colours use 50 ms rechecks. Imported panes reuse the same detector
with the imported-pane cadence. Terminal reads aggregate bells, retain admitted
cwd reports, send terminal replies through the actor and coalesce render requests.
Focus reporting and bracketed paste use the destination's live modes. Runtime
cursor reads gate visibility, clip to the supplied viewport and translate its
coordinates. Pixel size requires nonzero dimensions without multiplication
overflow. Alternate-screen wheel input becomes vertical cursor keys only when
alternate-scroll routing is selected, using the live keyboard modes.

Explicit shutdown cancels and joins detection and delayed work, requests I/O
shutdown and escalates hangup, terminate and kill across the original terminal
session, with 250 ms per signal. Session lookup uses the original session leader's
identity even after that child is reaped. Shutdown returns without waiting for
event capacity; the child watcher and imported-reader exit hook retain lossless
pane-death delivery until the consumer reads. Dropping a runtime requests the
same stop and session escalation but cannot await tasks.

Handoff pauses readers after accepted input drains and duplicates the owned
master. Export retains terminal modes, keyboard negotiation and the display title;
primary-screen replay keeps up to 8 KiB of complete recent lines. Import starts
paused without taking process ownership. Commit releases the old reader;
`assume_handoff_ownership` makes the imported owner responsible for session shutdown.
`terminal::runtime_registry::TerminalRuntimeRegistry` stores runtimes directly by
durable terminal identity and coordinates reader pause, ownership and redraw.

`maestro_daemon::render_signal::RenderSignal` coalesces render work while retaining
its origins: generic requests, pane terminal damage and terminal-title changes.
Hidden panes share one pending wakeup. A newly damaged visible pane or a new title
source requests another wakeup even while hidden damage is pending. The caller
supplies the visible pane set and performs the wakeup; the signal starts no task.

`has_immediate_work` distinguishes visible damage, titles and generic work from
hidden-only damage. `take` drains all accumulated origins together and clears the
pending flag while retaining the supplied visibility policy. Reading title origins
does not drain them or mark those panes as damaged.

`maestro_daemon::pane::shell` builds native shell operands from supplied settings
and environment observations. A configured shell wins over `SHELL`; blank values
use `/bin/sh` on Unix or `powershell.exe` on Windows. Automatic mode uses a login
shell only on macOS. Login mode resolves an executable path before launch and
sets `SHELL`; on Unix, argument zero is `-` followed by the shell basename.
Windows direct PowerShell launches wrap the profile prompt to report filesystem
cwd without changing the status observed by the original prompt. When prompt
reporting is enabled, a wrapped prompt may start above the visible screen: directory
recovery follows its retained soft-wrap rows, but rejects an incomplete prompt whose
prefix has been discarded.

`maestro_daemon::pane::launch_env::PaneLaunchEnv` applies terminal defaults
(`TERM=xterm-256color`, `COLORTERM=truecolor`) and names the terminal the child runs
in: `TERM_PROGRAM=maestro` and `TERM_PROGRAM_VERSION` set to the product version,
whatever the parent holds. It sets `fish_handle_reflow=0` unless the parent already
holds that exact name; a differently cased name does not count. It removes an
inherited agent thread id and the markers of the outer terminal or multiplexer
(`ITERM_SESSION_ID`, `LC_TERMINAL`, `LC_TERMINAL_VERSION`, `WEZTERM_PANE`,
`KITTY_WINDOW_ID`, `WT_SESSION`, `TMUX`, `TMUX_PANE`, `STY`, `ZELLIJ`,
`ZELLIJ_SESSION_NAME`, `ZELLIJ_PANE_ID`), then applies user environment pairs.
User pairs can override these defaults. The daemon marker, supplied socket/binary
paths and managed workspace/tab/pane identity then override user pairs. Callers may
retain inherited identity or remove only the pane id. Existing replacement
environments remain replacements, and both environment modes give the same result.

The native PTY adapter is currently available on Linux; shell policy branches for
Windows and macOS are exercised with supplied target and environment observations.

`maestro_daemon::pane::probe` classifies foreground jobs using the shared process
observations and agent catalogue. A leader environment hint wins over its name;
an identifiable leader wins over a non-leader hint. Unidentified jobs retain the
observed group without inventing an agent. Raw `MAESTRO_AGENT` values are read by
`maestro-process` and classified here.

Probe scheduling reacts to group changes, pending completion and restore checks.
Unidentified same-group acquisition lasts up to eight seconds, checking every
500 ms for the first 1.5 seconds and every two seconds afterward. Identified agents
receive five-second safety checks; unidentified panes without a group receive
30-second checks. Lifecycle authority skips stable routine scans but preserves
exit and pending-release checks. Six consecutive misses release a retained
identity; returning to the pane shell publishes completion before clearing it.

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
recoverable publication owner, identical equal-version bytes are left alone, and
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

## Named daemons

`maestro_daemon::instances::Selection` observes supplied attach arguments, an
explicit daemon-command target and inherited environment values. It never
rewrites argv or changes process environment. `attach --daemon <name>` and
`attach --daemon=<name>` select explicitly; chat `--session` arguments and
arguments after `--` remain untouched. Help stays with command dispatch.

Names contain 1–64 ASCII letters, digits, dots, underscores or hyphens; `.` and
`..` are rejected. `default` selects the agent root. Listing returns default once,
then valid named directories in lexical order. Deletion refuses default and live
daemons, tolerates a missing directory and leaves other folders alone.

Explicit selection ignores inherited socket overrides. Implicit API overrides
win over client-only overrides; either avoids validating an unused inherited
name. Attach, stop and restart guidance names the selected daemon and warns that
stopping exits pane processes. A custom API socket is retained in restart
guidance. Selection does not dispatch commands or launch a process.

`instances::stop_session` sends one `server.stop` request to the selected API
socket, then waits until neither the API socket nor the client socket accepts a
connection. The whole stop has one 15 s budget, enough for a slow graceful
shutdown. A reply that never arrives, an empty reply or a dropped connection
proves nothing, so the stop keeps checking the sockets. A refusal from the daemon,
an undecodable reply or another transport failure ends the stop with that error.
At the deadline the error names every socket that still accepts connections.

## Daemon client and status

`api::client::DaemonClient` keeps one endpoint: a named-daemon selection or an
explicit socket path, kept exactly as given. Each request opens its own
connection, writes one canonical line and reads one reply line. Ordinary requests
set no timeout. `request_value_with_timeout` applies the supplied duration to
both the send and the receive of that connection, and a zero duration fails with
the platform's invalid-input error. `request` decodes the shared response and
returns a failure with its correlation, command, code and message.

Status is a live ping, never a file. `DaemonClient::status` and
`api::read_runtime_status_at` return the daemon's version, protocol and
capabilities. The reader returns no status for an absent socket, a refused
connection or a daemon that does not answer in time; any other failure keeps its
original diagnostic. `platform::current_process_is_detached_server_daemon`
reports whether the process leads its own session, which is how a detached
daemon runs.

## Environment names

`maestro_daemon::environment::NAMES` lists the names injected into panes and
workspace actions and status commands, plus the inputs observed by daemon selection. It is a
vocabulary table, not an environment reader or launcher. Callers supply observed
values to `SelectionEnvironment`; selection never mutates the process environment.

| Name | Consumers |
|---|---|
| `MAESTRO_ENV` | Pane, action |
| `MAESTRO_SOCKET_PATH` | Pane, action, status command, daemon selection |
| `MAESTRO_BIN_PATH` | Pane, action, status command |
| `MAESTRO_WORKSPACE_ID` | Pane, action |
| `MAESTRO_TAB_ID` | Pane, action |
| `MAESTRO_PANE_ID` | Pane, action |
| `MAESTRO_PANE_RUNTIME_ID` | Pane on Windows |
| `MAESTRO_ACTION_ROOT` | Action pane, action |
| `MAESTRO_ACTION_CONFIG_DIR` | Action pane, action |
| `MAESTRO_ACTION_STATE_DIR` | Action pane, action |
| `MAESTRO_ACTION_MANIFEST_ID` | Action pane, action |
| `MAESTRO_ACTION_ENTRYPOINT_ID` | Action pane, action |
| `MAESTRO_ACTION_CONTEXT_JSON` | Action pane, action |
| `MAESTRO_ACTION_ID` | Action runtime |
| `MAESTRO_ACTION_EVENT` | Action runtime |
| `MAESTRO_ACTION_EVENT_JSON` | Action runtime |
| `MAESTRO_ACTION_CLICKED_URL` | Action runtime |
| `MAESTRO_ACTION_LINK_HANDLER_ID` | Action runtime |
| `MAESTRO_DAEMON` | Daemon selection |
| `MAESTRO_CLIENT_SOCKET_PATH` | Daemon selection |

The daemon finds the workspace home through `maestro_daemon::paths::home_directory`.
On Unix it uses `HOME` as set, including an empty value. When `HOME` is unset,
Home and Follow policies without a followed directory use the process working directory,
or `/` when that cannot be read. Path policies keep `~` and `~/x` literal, and workspace
labels and window titles do not shorten the account directory to `~`.

On Windows the lookup tries `USERPROFILE`, then `HOMEDRIVE` plus `HOMEPATH`, then `HOME`.
Empty values and `~` are unusable. The drive and path must both be usable, and the path
must start with `\` or `/`; otherwise only `HOME` remains to try. Home and Follow policies,
workspace labels and Path expansion share this lookup without an account fallback.
Path expansion accepts `~`, `~/x` and, on Windows, `~\x`. When no home is usable,
Path policies keep the path literal; Home and Follow keep their working-directory fallback.

Optional context fields are injected only when present. The manifest identifier
and the identifier of an action declared by that manifest are distinct values.
Build flags, test inputs and unrelated runtime settings are not daemon-selection
inputs.

## File logging

`logging::init_file_logging` takes `DaemonPaths`, a role and the caller's observed
`MAESTRO_LOG` value. Roles `in-process`, `client` and `daemon` write
`maestro-in-process.log`, `maestro-client.log` and `maestro-daemon.log` in the
selected daemon folder. `help_log_paths_summary` describes those locations.

All file writers share one synchronized retained file per initialization. Output
appends until a write would exceed 5 MiB, then replaces the file without keeping
numbered generations. File creation failure leaves logging uninstalled; a later
write, flush or rotation failure disables that writer without failing the caller.
An already-installed process subscriber is not replaced.

The default filter is `maestro=info`. A supplied valid `MAESTRO_LOG` directive
replaces it; an invalid directive uses the default. Logs include targets and
structured event fields without ANSI colour. Routine API observations log at
DEBUG; nonroutine UI-changing requests and non-OK completions log at INFO.
Request failures and wait timeouts log at WARN. Pane wait and persistence failures
log at ERROR. Lifecycle, workspace, tab, restore, update and integration helpers
retain their event, subsystem, outcome and operation-specific fields.

## Snapshot operations

The live `app`, `tab` and `workspace` modules and `persist::capture` and
`capture_history` use the Linux pane runtime and are available only on Linux.
Snapshot save and clear operations remain available on other native targets.

`persist::save`, `clear` and `clear_history` use `DaemonPaths` and the existing
workspace snapshot storage. They add no format or serializer. Save commits
topology before separate history; absent history removes the history file.
Only a completed combined save emits success. A history failure does not undo
already-committed topology.

Clear removes topology first. If that fails, history remains untouched and only
the failure is logged. Once topology is removed, clear attempts history removal
before reporting topology-clear success, even if history removal failed.
Direct history clear tolerates absence and reports other native removal errors.

Detection catalogue events retain update diagnostics and refresh source/version
summaries from the same caller-supplied `ManifestCache` shared with pane runtimes.
Successful updates show agent labels and versions when workspace toast delivery
is enabled. Matching retained runtimes reset their fallback detectors after the
shared cache changes. Empty updates refresh diagnostics and summaries without
a toast or detector reset.

The daemon can toggle zoom on the active split tab without changing its layout.
A single-pane tab is unchanged; a zoom change marks the session dirty.

Closing a pane or tab targets the active workspace, not the navigation selection.
Detached terminal metadata and plugin records are removed through the same cleanup
owner as workspace closes. Closing the last pane or tab of a parent repository
group requests confirmation when enabled; closing a linked workspace leaves its
checkout directory untouched. Agent completion can be published before removing
the pane, while its identity and notification context are still available.

`app::App` retains daemon state beside the existing terminal runtime registry.
Immediate workspace toasts use the first tab's live root directory for their
context, falling back to terminal metadata and then the workspace identity.
Delayed deliveries reread that directory at delivery time for both workspace and
client notification context. Explicit workspace names still take precedence.

Internal events publish agent release even when an idle agent's execution status
has not changed. Pane death publishes completion and the exit event before its
identity is removed. Temporary overlay exits clean their owned files and restore
previous focus and zoom only while the overlay still owns focus; a focus change
made before exit is preserved.

Construction retains one set of launch adapters, including the shared detection
cache, supplied child setup entry, socket, event sender and render signals.
Runtime configurations bind those inputs to the real pane ID and current size.
When a launch-command pane is marked to return to a shell, exit starts that shell
without removing the pane. A successful replacement clears agent runtime identity
and restores focus. A failed spawn follows the normal pane-close path.

## Repository identity

Successful workspace creation admits native repository metadata and its initial
branch after the pane spawn succeeds. Ordinary repositories, linked worktrees,
standalone bare repositories and embedded bare repositories use the shared
`maestro-workspace::workspace::git` discovery owner, re-exported through
`maestro-daemon::git::discovery`. Reftable branch and revision queries stay with
the daemon's process-owning caller.

Repository grouping uses the shared metadata directory; checkout identity uses
the checkout root. A linked worktree's automatic label names the checkout, not
the repository group. Display operations read admitted labels without repeating
filesystem discovery. Root-terminal metadata can select a newer admitted label
without changing the workspace's retained fallback directory.

## Git and worktrees

### Branch tracking configuration

`maestro-workspace::workspace::git::config::read_branch_config` reads the remote and
merge reference of a branch. Files apply in Git's order: the user files the caller
supplies, then the repository file in the shared metadata directory, then the checkout's
`config.worktree` when the repository file turns on `extensions.worktreeConfig`. The
extension is read from the repository file alone, as Git does, and its last value wins.
`ConfigEnvironment::from_variables` builds the user list from `XDG_CONFIG_HOME` and the
home directory; the reader itself reads no environment variable. The daemon supplies
`HOME` as set on every platform, including an empty value, without consulting the account
directory or the Windows workspace-home variables. With `HOME` unset, it supplies no home
and reads only the user file under `XDG_CONFIG_HOME`, when that variable is set. Otherwise,
the default user files are `$HOME/.config/git/config` and `$HOME/.gitconfig`.
`XDG_CONFIG_HOME`, including an empty value, replaces the first file's directory.

Includes apply where they appear. A file already being read is not entered again inside
itself, so a cycle ends at the repeated file. The `includeIf` conditions are `gitdir`,
`gitdir/i`, `onbranch` and `hasconfig:remote.*.url`. A `hasconfig` include is skipped when
it, or a file it includes, sets a remote URL, and URLs set in the worktree file never
satisfy the condition. For `gitdir`, `gitdir/i` and `onbranch` patterns, `*` and `?`
stop at `/`, `**` crosses it and a trailing `/` means everything below. URL conditions
allow wildcards across `/`. Matching takes time proportional to the pattern length
times the text length.

The fetch refspecs of the tracked remote map the merge reference to its local tracking
reference. A remote of `.` tracks the merge reference itself, a remote without refspecs
uses `+refs/heads/*:refs/remotes/<remote>/*`, and a negative refspec removes no mapping
because it only filters what a fetch downloads.

A read keeps what it saw of each file. `ConfigRead::is_current` turns false after an
edit, a newly created include, a retargeted symbolic link or a file that could not be
read, so a cache built on the read is rebuilt on the next refresh. The bare-layout and
reftable checks of repository discovery use the same line syntax: the last assignment in
a plain section wins and quoted values lose their quotes.

### Repository status

`git::status::git_status_snapshot_for_cwd_with_demand` observes the checkout around a
directory and returns a snapshot of its label, branch, ahead/behind counts and grouping,
with the cache entry to keep. The entry carries a fingerprint: what `HEAD` names (a branch
and its commit, or a detached commit) and, when the counts are wanted, the branch's
upstream and its commit. Loose references are read first, then `packed-refs`; reftable
storage is read through the `git` command. The tracking configuration comes from
`read_branch_config` and is kept while `ConfigRead::is_current` holds and the discovered
checkout is the one it was read for, so a `.git` file retargeted to other metadata is read
again.

`git rev-list --left-right --count` runs through the process owner only when the
fingerprint differs from the cached one; otherwise the cached counts are reused. A
directory outside any repository is remembered for thirty seconds. The cache key of a
checkout is its canonical root, so each linked worktree has its own, and workspace
creation takes its key from the same helper.

### Status refresh

The daemon loop refreshes status on its one timer and starts no other scheduler. A
refresh is wanted while a workspace exists and either a sidebar row shows `branch` or
`git_status`, or a terminal has reported its directory; a report asks once for a fresh
branch and label whichever rows are shown. A refresh is due 1.5 seconds after the last
one ended, or at once when made due. The deadline is one of those the loop already waits
for, and it does not depend on attached clients.

At most one refresh runs at a time. It runs on the blocking pool and publishes
`AppEvent::GitStatusRefreshed`, also when it panics, so the running flag cannot stay set.
A request made while one runs is kept and applied when its result arrives. Workspaces in
one repository share one observation. After a directory report and every five minutes,
remembered cache keys are discarded and repositories are found again; making a refresh
due also drops the results cached for directories outside repositories.

`DaemonState::apply_workspace_git_statuses` applies a result only while its workspace
still follows the directory the result was requested for. The branch and the counts change
only when they were demanded, and worktree membership never changes. The loop requests a
render only when a visible value changed.

## Retained tab-bar status

`App::configure_tab_bar_status` retains up to sixteen ordered zoom, hostname,
local datetime, literal text or command segments. Reload invalidates previous
command results and synchronously kills running commands and their descendants.
Commands use the supplied daemon socket and executable, stable active workspace,
tab and pane IDs, and the focused runtime's current directory when available.
These optional values use `MAESTRO_ACTIVE_WORKSPACE_ID`, `MAESTRO_ACTIVE_TAB_ID`,
`MAESTRO_ACTIVE_PANE_ID` and `MAESTRO_ACTIVE_PANE_CWD`.

Commands drain all output while retaining only the final line's first 4,096 bytes.
Dynamic text is trimmed, stripped of control and Unicode format characters, and
limited to eighty Unicode scalars. Literal text and separators preserve printable
spacing. Command deadlines start before the first task poll; a running command
has no second scheduling deadline. Current-generation completion requests a render
only when its visible output changes. The daemon loop owns calls to the status
scheduler; this owner does not start another background loop.

Local datetime uses the system timezone through the safe calendar library. Linux
hostname observation reads the kernel hostname without starting a process.

Clipboard write observations retain a separate “copied to clipboard” acknowledgement
for two seconds. Disabling clipboard feedback clears its content and deadline.
Feedback never replaces an agent notification; the daemon scheduler expires it
at its deadline. Host clipboard emission remains with the terminal effect owner.

`App::new` consumes the matching internal-event receiver alongside the shared
launch inputs. One drain processes at most sixty-four observations and retains
later events for the next pass. Render impact comes from the same event dispatch
used by direct callers: bells alone and unchanged or stale status output do not
request a render.

Construction initializes agent-panel ordering, sidebar collapse state and
focus-gain redraw policy from the accepted preferences. The agent-panel ordering
shares the settings owner's vocabulary rather than translating a duplicate enum.

Named workspace creation retains its chosen directory until completion or
cancellation. Pending naming suppresses automatic default creation without
allocating a workspace or process. Directory selection follows navigation
selection before terminal focus, then applies the configured follow, home,
current or authored-path policy. Native creation binds real allocated identities
to the shared launch inputs inside the topology's spawn transaction. A failed
spawn publishes neither a workspace nor terminal ownership. Modal input remains
with its input owner.

### Host appearance and theme names

The daemon retains manual, dark and light theme names separately. Automatic
switching is opt-in; without it, host appearance leaves the manual name unchanged.
When enabled, the selected theme's dark/light sibling supplies each omitted name.
Custom colour overrides and the legacy accent are retained for the renderer, which
owns palette construction.

Explicit host appearance reports take precedence over inferred background
luminance. Host colours still update independently, preserving each child's own
colour overrides. The daemon propagates accepted observations to live panes and
retains them for later pane launches.

### Live preferences

`App::reload_config` uses the supplied `SettingsManager` to flush pending writes
before reloading the global document. Invalid documents leave accepted preferences
unchanged. Independently invalid areas retain their current values while valid
areas apply; inverted sidebar bounds reject the interface area before clamping.
Reloads preserve live sidebar collapse state and manually resized widths, subject
to the new bounds. Accepted shell, directory and client preferences stay in
`App::config` rather than a second configuration snapshot.

Explicit reloads report success or warnings through a workspace toast. Silent
reloads preserve existing feedback, and the loop can consume the reload observation
once. Construction requires an explicit `PaneLaunchInputs::no_session` choice;
that policy suppresses automatic background checks without changing their stored
preferences. An accepted history disable clears separate history on the next
loop pass or at shutdown, after joining any captured writer so it cannot recreate
the file. Reading the reload observation does not discard that pending clear.
Rejected history settings leave both the accepted preference and history intact.

Workspace preference saves use the same settings manager. Related leaves, such
as a manual theme name and automatic-switch flag, reach storage in one batch.
Saves preserve unrelated global preferences and fresh nested siblings from other
writers. A failed global write leaves live daemon preferences unchanged and shows
a diagnostic for five seconds; successful saves silently apply the accepted global
document. The settings manager keeps its existing immediate cache and error-drain
semantics.

Tab observations expose the stable public number even after an earlier tab is
closed. Display labels still follow the current position unless explicitly
named; focus and agent attention are projected from the retained live state.
Workspace attention ranks blocked panes above unseen completions, working panes,
seen completions and unknown panes. Pane details follow tab and layout order,
retain tab vector positions, and qualify agent labels when multiple tabs exist.

Terminal targets resolve a terminal handle before a current public pane handle,
then an agent name or detected label. Agent targets accept a current public pane
handle only for an agent, or an explicit agent name; historical pane spellings
remain names. Ambiguous names return all matching viewport identities, current
directories and agent statuses rather than selecting the first match.

Session capture owns the current workspace order, stable numbers, layout, focus,
sidebar state and settled pane launch metadata. A first tab's live root directory
supplies workspace identity. Screen history is captured separately for each
attached runtime, so topology JSON contains no terminal output. Session provenance
uses the terminal owner's live hook identity before its restored identity.

The daemon runs one internal-event loop with one selected deadline timer. Dirty
sessions debounce for five seconds; a busy captured writer retries after 250 ms.
Final saving joins that writer before capturing current state. Interactive size
polling is separate from headless deadlines, and hidden render attempts do not
advance the presentation clock.

Shutdown retains each runtime while draining its critical exit publication,
even when the event queue is full. It saves the session before terminating live
children and does not respawn command shells while stopping.

Terminal titles retain their latest raw text, including progress spinners. Only
changes to normalized title text advance the pane revision and publish a complete
pane update. Sidebar redraws follow the title form used by its configured rows.
Pane and agent observations share one presentation projection; foreground process
directories are read asynchronously through the retained runtime.

Outer window titles render on the daemon from workspace, tab, pane, normalized
terminal-title and hostname tokens. Empty or invalid templates disable ownership
of the outer title. Accepted settings reloads replace the parsed template.

Linux desktop notifications invoke the native notifier with a literal operand
separator before the title. Missing displays or notifier executables return
without showing a notification; other launch and wait failures remain errors.

The native footer and daemon share repository discovery, including standalone
bare repositories and embedded bare worktrees. Checkout paths retain their
spelling for watching; canonical paths remain repository identity keys.
Caller-specific metadata rules use one typed policy: footer worktree markers
require a space after `gitdir:`, while workspace identity also accepts the
unspaced form. Footer directory metadata ignores `commondir`; a worktree link
uses it. Workspace discovery climbs past incomplete nested repositories; footer
discovery stops at a marker without HEAD. Native directory and link bytes are
retained. The footer calls the workspace owner through the direct native
application dependency. The browser footer has no native repository discovery.
