# Manifest extension placement

## Decision

Manifest mode is a default-on base package, `maestro-manifest`: a package crate in
the `maestro` workspace, released with Maestro like the other base packages and
never a dependency of a core crate. The core holds no manifest code. It offers five
generic hooks: a read-only base settings layer with locked keys, a decision hook
asked before every governed effect, a global effective view for the workspace
daemon, load-time contributions from base packages (which load first), and host
operations over the existing settings and package owners. Instruction files come
from the manifest: the manifest package installs the ones a user picks into the
local instruction folders, and a separate rules system loads them. Workflow files run on the runner of Maestro's subagent capability, and
authored content, including the default workflows, lives in manifests.
A broken manifest fails the manifest package's load, which refuses sessions
through the foundation's extension-load failure.

## Trade-off

The core already had every owner the manifest needs (settings, package manager,
resource loader, model registry), and putting the policy there would have needed
no new hooks. It would also have made the core carry organization policy that
internal mode never uses, against the rule that every non-core capability ships
as a default-on package. The hooks cost a load-order rule (base packages load first but keep their
place in the extension list, so conflicts resolve as before),
a persisted base layer for the daemon, which has no extension host, and host
operations so the package never copies settings or package logic. In return the
core stays generic: any package can publish a base layer or a decider, and
internal mode only gains the base-package load order. The agent learns about
manifests and workflows from skills shipped by those packages, so the system
prompt stays unchanged.
