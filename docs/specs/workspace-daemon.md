# Workspace daemon

This specification defines how the workspace daemon, its socket API, its commands and the multiplexer are
built from the workspace reference. It implements the [workspace daemon](maestro-port.md#workspace-daemon)
section of the port specification. The pane terminal engine is specified in
[terminal emulator](terminal-emulator.md) and the wire records in [daemon protocol](../daemon-protocol.md).
Terms in [Glossary](#glossary) are also defined in [the glossary](../../CONTEXT.md).

## Purpose and authority

The workspace daemon keeps workspaces, tabs, panes, pane terminals and pane agents alive for one user while
clients come and go. Maestro ports it like any reference:

- each reference file maps to a Rust module with the reference's exported names in Rust casing, and the
  inside is idiomatic Rust;
- the reference's own tests are ported red first under Maestro names;
- reference defects are fixed and listed once;
- every capability is kept, and nothing is added without a reference line or an owner decision.

The daemon is core: the executable always wires it, there is no feature switch, and it shares the
application's single setup and single protocol.

## Problem statement

The reference spreads one application object over dozens of files. Its slices do not compile in file order:
early state code calls input, rendering and server code. It runs two event loops with duplicated scheduling.
It forwards the pane terminal through three layers. It mixes daemon state with view state. It reaches the
operating system through `unsafe` calls and a terminal library that Maestro does not use. A file-by-file port
in the reference's order would either stall on forward calls or copy those duplications.

## Solution

The port is cut into 37 deliveries that each compile from their predecessors.

- **Order check.** Every call or type use from one delivery into a later one was listed from the reference
  source, and each was resolved in one of three ways:
  - the item moves to its first user;
  - the later delivery adds the call as its own arm ([growth](#growth-later-capabilities-add-their-own-arms));
  - a design rule below removes it.
- **One owner per mechanism.** One loop, one pane runtime type, one state owner, one key vocabulary, one
  file-replacement owner and one dispatch path.
- **Frame library confined.** The multiplexer renders through a port the daemon calls, so the frame library
  stays in the multiplexer.

## User stories

1. As a workspace user, I want my panes, tabs and workspaces to keep running when I close the multiplexer, so that long agent runs survive detaching.
2. As a workspace user, I want several named daemons with separate sockets, snapshots and logs, so that projects stay isolated.
3. As a workspace user, I want the multiplexer to show the sidebar, tab bar, pane chrome, menus, navigator and overlays as the reference does, so that my workflow is unchanged.
4. As a workspace user, I want my keybindings, custom commands, copy mode and mouse to work as in the reference, configured in my one keybinding file.
5. As a workspace user, I want agents in my panes recognised and their status (idle, working, blocked, done) shown, notified and sounded as the reference does.
6. As a workspace user, I want the daemon restarted or upgraded without losing panes: restore after restart, and live handoff to a new executable.
7. As a workspace user, I want git worktrees created, opened and removed from the workspace, and the workspace labelled from its repository.
8. As a workspace user, I want to attach to a daemon on another host over SSH.
9. As a script author, I want every workspace operation as a `maestro` noun with help, completions, JSON output and stable exit codes.
10. As an SDK caller, I want every workspace command, event, subscription and wait on the one protocol, on stdio and on the socket.
11. As a package author, I want workspace actions, action panes, hooks and link handlers delivered as package content and run by the daemon.
12. As a user of other coding agents, I want their hooks installed and kept current so that they report into my panes.
13. As a maintainer, I want no `unsafe` code, no patched library fork and no second setup or protocol.

## Capabilities kept

Every capability of the reference is kept. Two inputs have no Maestro source: legacy snapshot versions and
legacy indexed shortcuts. A Maestro user never holds either file, so no user loses anything.

| Area | Kept capabilities | Deliveries |
|---|---|---|
| Process and sockets | long-lived daemon; named daemons; private local sockets with stale-socket recovery; API and client sockets; monolithic mode, where panes end with the window; server stop, reload and status; live handoff | 1, 8, 12, 13, 21, 29, 30, 34 |
| Command line | one `maestro` command with fifteen workspace nouns dispatched before the chat arguments; help, completions and JSON output; local usage errors before any socket use; not-running and protocol-mismatch reports | 21–23, 34 |
| Socket API | every workspace command and event of the one protocol; subscriptions with replay and filters; waits; agent prompt; continued requests on one connection | 13–15, 17 |
| Topology | workspaces, tabs and split layouts; stable public numbers; moves, swaps, zoom and resize; declarative layouts; status roll-up | 3, 12, 14 |
| Pane terminals | PTY children; ordered terminal replies; colour queries answered from the host theme; retained text, search, word motions and reads; selection; bracketed paste; focus events | 2, 9–11 |
| Pane agents | recognition from processes and screens; detection manifests and the downloaded catalog; hook reports and session identity; agent names, start, prompt, wait, read and send keys; agent views; alternate-screen history reads; resume after restore | 4, 5, 7, 11, 16, 17 |
| Agent integrations | managed hooks and settings for the supported third-party agents; status, outdated detection and recommendations | 20, 23 |
| Workspace actions | action manifests as package content; command runner, logs, startup and event hooks; action panes and popups; link handlers; install preview and build step | 19, 23 |
| Persistence | the topology snapshot with optional pane history; restore with the same public numbers; newer-version refusal | 6, 12, 16 |
| Multiplexer | sidebar, tab bar, pane chrome, dialogs, menus, navigator, settings overlay, keybinding help, narrow layout, toasts, release notes, announcements and onboarding | 26, 27 |
| Input and keybindings | prefix and direct bindings, custom commands, mouse, copy mode, key leases, host input decoding | 25, 28 |
| Notifications | toasts, system and terminal notifications, sounds, window titles, tab-bar status entries | 12, 29, 32 |
| Git and worktrees | repository identity in bare, embedded-bare, linked and reftable layouts; status cache; worktree create, open and remove | 12, 18, 22 |
| Remote attach | attach over SSH with a managed SSH configuration, remote install and remote server lifecycle | 35 |
| Updates | release channels, verified downloads, running-server update plans, package-manager guidance | 33 |
| Platforms | Linux in every delivery; macOS, other Unix systems and Windows in deliveries 36 and 37 | 36, 37 |

## Design

### Placement

| Crate | Modules added | Deliveries |
|---|---|---|
| `maestro-json` | `jsonl` | 1 |
| `maestro-ipc` | `local_socket`, `socket_paths` | 1 |
| `maestro-process` | `pty::{fd, actor, foreground}`, `shell` additions, `child_process::native` signal escalation | 2, 19, 29 |
| `maestro-workspace` | `layout`, `workspace`, `workspace::git`, `tab`, `pane_state`, `terminal_id`, `metadata_tokens`, `persist::{snapshot, io}` | 3, 6, 14 |
| `maestro-detection` | `agents`, `manifest`, `manifest_update`, bundled manifests | 4, 7 |
| `maestro-lock` | `replace` | 6 |
| `maestro-protocol` | `KeyModifiers` constants and the underline bit layout in `frames` (additive) | 9, 26 |
| `maestro-settings` | `SettingsManager::set_global_key` becomes public | 8 |
| `maestro-packages` | the `actions` resource kind, the subdirectory field, `package_filters::toggle_resource`, `PackageManager::set_prepare_callback` | 19 |
| `maestro-daemon` | `terminal`, `terminal_theme`, `agent_resume`, `settings`, `paths`, `environment`, `logging`, `instances`, `input`, `selection`, `pane`, `render_prof`, `events`, `render_signal`, `app`, `aggregate`, `git`, `persist`, `platform`, `key_combo`, `api`, `client`, `restore`, `actions`, `commands`, `popup`, `worktree`, `integrations`, `server`, `pane_graphics`, `announcements` | 5, 8–20, 25, 27–31, 34 |
| `maestro-rpc` | `daemon` (socket adapter and graphics stream channel) | 13, 15, 31 |
| `maestro-cli` | `workspace_commands`, `completion`, `output` | 21–23, 32, 33, 35 |
| `maestro-tui` | `stdin_buffer` host replies and mouse tails; structured key decode; `terminal_theme` host colour queries and reply parsing | 25 |
| `maestro-tui-crossterm` | alternate screen, mouse capture and colour-scheme report modes | 32 |
| `maestro-multiplexer` | `render`, `palette`, `ui`, `view`, `input`, `keybindings`, `surface`, `client`, `sound`, `launch`, `remote` | 26–29, 32, 34, 35 |
| `maestro-app` | `daemon` and `terminal` re-exports, `utils::clipboard` additions, `workspace_actions`, `daemon_presence`, `self_update::release`, the footer's branch reader, the keybinding document readers | 12, 13, 19, 24, 26, 28, 32, 33 |
| `maestro` | server, render-surface, attach, monolithic and PTY set-up wiring | 21, 29, 34 |

An inherent `impl` must live in the crate that defines its type. Reference methods on topology types that
belong to the daemon therefore become free functions in the daemon, with the receiver as the first argument
and the reference's names. The orphan rule forbids `From` between two foreign types, so the multiplexer
converts topology rectangles to frame rectangles with free functions.

### Daemon state, view state and the render surface

- **Daemon state.** `maestro-daemon::app::state::DaemonState` holds every field that daemon code reads or
  writes, directly or through a state method. Examples are sizes, popup geometry, persisted sidebar values,
  toasts, the mode, copy mode, selection, focus records, notification delivery and the worktree-open dialog.
  It is shared by every client of one daemon.
- **View state.** `maestro-multiplexer::view::View` holds the fields only multiplexer code touches:
  navigator, menus, drag, keybinding help, the settings overlay, release notes, announcements and scroll
  positions.
- **Render surface.** `maestro-daemon::server::surface::RenderSurface` is the one port through which the
  daemon renders a client's frame, routes a client's input and lets the view react to daemon changes (for
  example, scrolling the sidebar to the active workspace).
  - The multiplexer implements it, and the executable wires it in.
  - The daemon decides when and to whom frames go; the surface produces the bytes.
  - The per-client render baseline stays in the daemon's client record.
  - The frame library is used only inside `maestro-multiplexer`.
- **Method set.** Each delivery that adds a surface method qualifies it at step 1 against the reference call
  sites listed in its issue.

### Interfaces

- **JSONL framing.** `maestro_json::jsonl::{serialize_json_line, JsonlLineSplitter}`: synchronous, bytes in
  and lines out. Callers own their async glue.
- **Spawning in topology.** `FnOnce(PaneId, SpawnRequest) -> io::Result<T>` is passed to every topology
  operation that creates a pane. The operation returns `(PaneId, T)` and removes the new leaf when the spawn
  fails.
- **Atomic replacement.** `maestro_lock::replace(path, bytes, Replace { through_symlinks })`.
- **Paths and names.** `maestro_daemon::paths::DaemonPaths` and `maestro_daemon::environment::NAMES`.
- **Server.** `maestro_rpc::daemon::run_server` starts the API socket and runs the daemon loop. The
  executable's `server` arm and the in-process tests call it.
- **Daemon client.** `maestro_app::daemon::client::{DaemonClient, ensure_running}`.

### Libraries

These libraries are already approved and are added by the first delivery that uses them:

- local sockets;
- the tracing subscriber and time formatting;
- the TOML and JSONC editors;
- shell completions;
- regular expressions;
- PNG;
- the frame library, for the multiplexer only.

No new library is added. The PTY master does not use the pseudo-terminal library on Unix, because it would
need `unsafe` to hand its descriptor to the async runtime.

## Implementation decisions

### One setup and one protocol

- **Paths and names.** `DaemonPaths` holds every daemon file, socket and log path under the agent directory
  or `daemons/<name>/`. `NAMES` lists every daemon variable with its class: injected into panes, injected
  into actions, or daemon-selection input. Lower crates receive paths and values from their callers; they
  read no variable and name no root.
- **Dispatch.** Workspace commands reach the daemon through the shared RPC dispatch path, on stdio and on the
  socket. The socket adapter lives in `maestro-rpc::daemon`; daemon state, subscriptions and waits live in
  `maestro-daemon::api`. `maestro-rpc::daemon::run_server` owns socket bind, accept and
  connection teardown and drives the existing daemon `App::run` loop. Each accepted socket
  uses the application connection runner with no session; supplied sessions remain supported
  by that same runner. Typed requests flow down to daemon-owned handlers, never through a
  second dispatch loop. Startup announces only resources that have actually bound.
- **Continued requests.** A connection reads lines full duplex. A line that arrives during a subscription or
  a wait is the next command. End of input drops the pending work, which ends the wait or the subscription.

### One daemon loop

- **Shape.** One task over channels with one deadline timer.
- **Each pass.** It drains internal events (at most 64), then API requests, then scheduled tasks, and then
  computes the next deadline.
- **Server arms.** Client accept, server events, frame rendering and client notification are arms of the
  same loop.
- **Monolithic mode.** The same loop with one in-process client.
- **Timers.** Timers, debounce and retries use the async runtime's timers, written once for the loop.

### Topology without runtimes

- **Scope.** `maestro-workspace` holds topology only: layouts, workspaces, tabs, pane viewport state, public
  numbers, metadata tokens, git label rules and native repository discovery.
- **Identity ownership.** One `WorkspaceTopology` owns its workspace collection and the allocator for
  pane, terminal and workspace identities. Creation and split operations use this allocator. Restore
  admission reserves saved pane/workspace identities and public counters before new allocation; terminal
  identities are allocated afresh. Counters survive workspace removal and are independent between owners.
- **Runtimes.** The daemon passes the spawn closure and inserts the runtime into its one registry.
- **Repository identity.** Native discovery belongs to the existing
  `maestro-workspace::workspace::git` owner. The daemon and application footer use
  the same directory walk for ordinary, linked, bare and embedded-bare layouts.
  Native path bytes survive traversal and metadata-link resolution. Shared Unicode
  trimming applies to link text. Workspace discovery climbs past markers without a
  regular HEAD; footer discovery stops there and retains its branch, detached-head
  and watch policy. Git subprocess fallbacks stay in the process-owning callers.
  The footer calls the workspace owner through a direct native-only
  application-to-workspace edge. Discovery tests use real
  temporary directories rather than a second, fake-only filesystem adapter.
- **Geometry.** Topology uses its own rectangle and border set.

### One pane runtime over the concrete emulator

- **Ownership.** `maestro_daemon::pane::PaneRuntime` is the only runtime type for a pane terminal. It owns
  the PTY child, the emulator's `Terminal` and the agent detection task, and it keeps behaviour only.
- **No forwarding layers.** Types that only delegate are not built. The registry stores `PaneRuntime`
  directly.
- **Rendering.** Rendering takes the pane's render state through one locked accessor.

### PTY children without unsafe code

- **Master.** The PTY master is an owned descriptor from `rustix::pty`.
- **Child set-up.** The child must lead its own session and own its terminal. Delivery 2 qualifies at step 1
  how that set-up step runs without unsafe code; the candidate is a set-up entry of the running executable
  that calls `setsid`, `ioctl_tiocsctty` and `exec`.
- **Uses of the master.** Foreground detection, window size and descriptor passing for live handoff use the
  owned master.

### One key and mouse vocabulary

- **Vocabulary.** The protocol's key and mouse enums are the vocabulary in every crate.
- **TerminalKey.** `maestro_daemon::input::TerminalKey` wraps them. It adds the shifted code point and the
  physical key identity, and it has one conversion pair to the protocol's key event.
- **Emulator boundary.** The emulator's key event is built from `TerminalKey` only where an encoder is
  called.
- **Keys the protocol cannot carry.** They are dropped where host input is decoded, as the reference drops
  them.

### One file-replacement owner

`maestro_lock::replace` writes a temporary sibling, syncs the file, renames it and syncs the parent
directory, tolerating file systems without directory sync. It can write through an existing symbolic link.
The snapshot, the detection catalog and the announcements store use it.

### Foreground process facts

- **Location.** Foreground jobs, process trees, argv, working directories and environment values are read in
  the Linux-only `maestro-process::pty::foreground`. Its `ForegroundJob` and
  `ForegroundProcess` records live in the platform-independent
  `maestro-process::foreground` module, with re-exports at their Linux paths.
- **Environment hint.** It is returned as the raw value, and the daemon parses it with the agent catalogue.
  That removes the cycle between process facts and agent recognition.

### Growth: later capabilities add their own arms

- **Rule.** A delivery ports no call into a capability that a later delivery provides.
- **What later deliveries add.** Each later delivery adds its arms in modules that already exist: event
  variants, dispatch arms, loop tasks, state fields, surface methods and view-reaction calls. Every issue
  lists those call sites.
- **Unknown commands until the last family lands.** Until then, the shared dispatch answers the commands of
  families not yet delivered with its existing unknown-command failure, through one wildcard arm.
  Delivery 31 lands the last family and deletes that arm.
- **No placeholders.** No delivery adds a per-command stub, a placeholder result or a transport-specific
  refusal.

### Platform branches

Linux is delivered by every delivery. Windows and macOS branches inside a delivery's reference ranges are
ported by deliveries 36 and 37. Those deliveries need platform calls for which no locked safe library
exists, and a Windows pseudo-console library without a patched fork. They wait on that decision.

## Testing decisions

- **Unit tests.** Each delivery ports its listed reference unit tests red first under Maestro names (never
  the reference's names) and lists each misfiled row once. Coverage is at least 80% of lines from tests that
  prove behaviour.
- **Integration tests land where they first pass.** Nothing red or ignored reaches `main`.

  | Integration tests | Where they land |
  |---|---|
  | Socket API cases | in process through `run_server` and a temporary socket, in deliveries 13–15 and 17 |
  | Raw client-socket cases | deliveries 29 and 30 |
  | Command cases | deliveries 21–23 |
  | Attached-client cases | delivery 34 |
- **Harness floors.** A test that needs a later harness, such as the server's own test harness, lands with
  that harness.
- **PTY children in tests.** Tests that spawn PTY children use `maestro-test-process` to run a named test as
  the child set-up step.
- **Inputs without a source.** Tests whose subject has no Maestro input are not ported, and each is listed
  once: legacy setting values, legacy snapshot fixtures, registry files, removed flags and server-side
  plugin registration.

## Corrected reference defects

| Defect | Fix | Delivery |
|---|---|---|
| Six POSIX shell quoters, one with a different safe set, plus five other quoters | one quoter in `maestro-process::shell`; the others become its callers | 2 |
| The peer-closed probe consumes a byte and treats data as a disconnect | full-duplex lines and end of input | 1, 13 |
| Two run loops with duplicated scheduling, deferred requests, repeat plans and key handling | one loop | 12, 29, 34 |
| Three atomic JSON writers with different durability | one replacement owner | 6, 7, 27 |
| A second runtime owner beside the registry | topology without runtimes | 3, 12 |
| Two WSL detectors that disagree | one detector over the union of both signal sets | 29 |
| Daemon state mixed with view state; daemon operations inside the input module | the state split above | 12, 27, 28 |
| The footer discovery walk misses standalone bare repositories | the footer and daemon share workspace-owned native repository discovery | 12 |
| Delayed terminal-only delivery rewrites an existing workspace toast | retained toast context changes only on workspace delivery | 12 |
| An unlocked whole-file configuration rewrite can lose a concurrent write | writes through the settings file lock | 8, 12 |

Each fix carries a root-cause table in its delivery report: the reference file and line where the defect
originates, the failing witness before, the passing witness after, and a sweep of other callers.

## Runtime mechanics not copied

Results match the reference. These mechanisms are replaced and each is listed once:

- `libc` calls on Unix (rustix and the async runtime);
- the PTY poll loop and its wake pipe (an async descriptor keeps the same order);
- polling socket reads and the one-byte peer probe (readiness and end of input);
- one thread per connection (tasks);
- the signal-handling crate (the async runtime's signals);
- the terminal library's key types (protocol enums);
- the child `pre_exec` hook (the set-up step above);
- global caches and counters (owned state);
- environment mutation in tests (injected environments);
- the reference's outer request, result and event envelopes (the one protocol).

## Out of scope

- **Built by other deliveries.** The protocol records (on `main`), the terminal emulator and the host
  graphics cache.
- **Forwarding layers** that only delegate: the terminal runtime newtype and the pane facades.
- **Setup the application already owns:**
  - configuration roots and the configuration-path variable;
  - TOML storage and text edits;
  - unknown top-level section reports;
  - the embedded default configuration text;
  - the plugin registry file and its lock;
  - checkout folders and the legacy folder copy;
  - the source grammar for one hosting site and its offline registry glue;
  - the release-notes store;
  - a second release-manifest model;
  - the `curl` subprocess.
- **Inputs no Maestro user holds:** snapshot conversions from older versions and legacy indexed shortcuts.
- **Dead code:** a colour field kept only for backward compatibility.

## Delivery order

| # | Delivery | Blocked by |
|---|---|---|
| 1 | Frame JSONL lines and connect local sockets | — |
| 2 | Spawn PTY children and read process facts | — |
| 3 | Model workspace topology | — |
| 4 | Identify agents from foreground processes | 2 |
| 5 | Track pane agent state | 3, 4 |
| 6 | Save and load the topology snapshot | 3 |
| 7 | Evaluate detection manifests and update the catalog | 4 |
| 8 | Read daemon settings, write daemon logs and manage named daemons | 1, 5, 6 |
| 9 | Model terminal keys and mouse input | 5 |
| 10 | Process pane terminal bytes and reads | 2, 9; the emulator's reads and encoders |
| 11 | Run pane runtimes | 7, 8, 10 |
| 12 | Own daemon state, events and the daemon loop | 6, 11 |
| 13 | Serve the socket API and workspace commands | 1, 12; the shared RPC server |
| 14 | Serve pane and layout commands | 13 |
| 15 | Stream subscriptions and serve waits | 14 |
| 16 | Restore workspaces and resume agents | 14 |
| 17 | Serve agent commands and alternate-screen reads | 14 |
| 18 | Track git status and manage worktrees | 13 |
| 19 | Run workspace actions | 17; application resource loading; the install presenter seam |
| 20 | Install agent integrations | 13 |
| 21 | Dispatch workspace commands, serve and report status | 13; command-line parsing and startup |
| 22 | Run workspace, tab, worktree and pane commands | 15, 18, 21 |
| 23 | Run agent, integration, notification, API and action commands | 19, 20, 22 |
| 24 | Report Maestro pane agent state from inside a pane | 17; the session SDK; interactive start |
| 25 | Decode host terminal input | 9 |
| 26 | Render pane frames and chrome palettes | 10, 12 |
| 27 | Compose the multiplexer screens | 19, 20, 26 |
| 28 | Route multiplexer input and keybindings | 18, 25, 27; the external editor; keybinding publishing |
| 29 | Serve attached clients and stream frames | 16, 22, 28 |
| 30 | Hand off a running daemon to a new executable | 29 |
| 31 | Deliver pane graphics | 29; the emulator's images; the host graphics cache |
| 32 | Attach the thin client to the host terminal | 29; Windows process terminals; interactive exit; the external editor |
| 33 | Self-update across release channels | 21, 27; the package command presenter |
| 34 | Launch, auto-start and attach | 30, 32 |
| 35 | Attach remotely over SSH | 33, 34 |
| 36 | Run on macOS and other Unix systems | 35; the platform decision; platform runners |
| 37 | Run on Windows | 35; the platform decision; platform runners |

## Glossary

- **Pane runtime:** The daemon's live object for one pane terminal: the PTY child, its emulator and its
  agent detection task.
- **Daemon state:** The workspace daemon's single state owner: topology, pane terminals, pane agents and the
  view data that daemon operations touch; shared by every client of one daemon.
- **Daemon loop:** The one task that drains internal events and API requests, runs scheduled tasks and
  serves clients.
- **Detection catalog:** The downloaded, version-checked cache of detection manifests, used ahead of the
  bundled set when newer.
- **Agent view:** A filter and sort over pane agents, set by a client and applied to agent lists.
- **View state:** Multiplexer-only state (navigator, menus, drag, overlays and scroll positions), held by
  the render surface.
- **Render surface:** The multiplexer's adapter that the daemon calls to render a client's frame, route its
  input and react to daemon changes.
- **Live handoff:** Replacing a running daemon by a new executable while every pane keeps running.
- **Remote attach:** Attaching a multiplexer to a workspace daemon on another host over SSH.

Each term is added to `CONTEXT.md` by the delivery that first ships it (7, 11, 12, 17, 27, 29, 30, 35).

## Amendment to the foundation specification

Each edge lands in the first commit of its delivery, with the crate-table row in
[the port specification](maestro-port.md#crates-and-delivery-order) and the enforced graph policy:

| Edge | Reason | Delivery |
|---|---|---|
| `maestro-detection` → `maestro-process` | foreground-job facts | 4 |
| `maestro-workspace` → `maestro-lock` | snapshot writes | 6 |
| `maestro-detection` → `maestro-lock` | catalog commits | 7 |
| `maestro-daemon` → `maestro-terminal` | first daemon use of the emulator (already allowed; the required edge is added with the code) | 10 |
| `maestro-app` → `maestro-terminal` | terminal re-exports for the multiplexer | 26 |
| `maestro-multiplexer` → `maestro-protocol` | frame records for the pane render and the frame encoder | 26 |
| `maestro-daemon` → `maestro-lock` | announcements store | 27 |

- **`maestro-lock`.** Delivery 6 changes its job to "guard file writes (exclusive locks and atomic
  replacement)".
- **No other change.** No crate, upward edge or library is added.
