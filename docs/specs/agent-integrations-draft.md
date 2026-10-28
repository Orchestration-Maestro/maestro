# Agent integrations: draft amendment

Status: held for the shared base-package host and native file-permission support.
This draft does not register a package or claim an installer is available.
It accompanies the implementation when those prerequisites are available.

## Proposed glossary entry

**Agent integration:** a managed hook or extension installed into a supported
external agent's configuration, with availability, installed-version status and
update guidance. It reports pane-session identity and, where the target supports
it, lifecycle status. Maestro's own pane presence is separate.

## Proposed package contract

Add `maestro-agent-integrations` as a default-on base component package using the
existing base-source loader and its disable filters. It is a workspace member of
class `package`, exposes `factory` from its library and builds one `component`
example export. Core and dedicated crates never link it. Its internal dependency
is `maestro-extensions-wasm`; no peer package dependency is needed.

The package owns target registry, configuration editors, status checks and shipped
templates. The application routes command-line and API requests through the same
registered operation. Local installation works without a conversation or running
daemon and never starts either. The daemon retains native report admission.

Install, reinstall and uninstall preserve unrelated user configuration. Malformed
or duplicate-key configuration fails before publication. The qualified native
files facade owns atomic publication and modes; guest code does not implement a
second temporary-file/rename sequence. Target paths must never enter Maestro's
configuration root, including an overridden root. Maestro does not install an
external-agent integration into itself.

Replace the exclusion of other agents' hook files in the package specification
with this managed-integration contract. Keep native Maestro presence separate
from external integration installation. Update the workspace-daemon integration
row to name this package as installer policy owner, not the daemon.

## Delivery slices

Sizes below are estimates of new production Rust, excluding tests, documentation
and template data. They are not source-line acceptance requirements.

| Slice | Scope and behavior witnesses | Planned size | Gate |
|---|---|---|---|
| 1 | Package membership, factory/component export, default-on loading, disable and reload cleanup through a real admitted headless component; glossary and specification amendment travel with code | 200–350 lines | Shared base-package host |
| 2 | Target registry, prerequisite and deterministic path resolution, installed-version status, recommendations, JSON/JSONC/YAML/TOML editors; fixture homes and seeded unrelated settings, malformed/duplicate-key rejection and protected-root checks | 1,800–2,400 lines | Shared host registration and qualified native files |
| 3 | Target install/reinstall/uninstall, modes and legacy cleanup, shipped templates and application routing; executed installed templates against a controlled real endpoint, canonical identity/sequence/transcript reports, idempotence and no daemon auto-start | 1,800–2,400 lines plus about 2,810 template data lines | Shared host registration, qualified native files and socket server |

No empty factory, placeholder package, alternate file-publication mechanism or
second host registry is introduced while these gates remain closed.
