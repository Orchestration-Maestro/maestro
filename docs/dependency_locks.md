# Dependency locks

Cargo owns the workspace's generated `Cargo.lock`. Commit it with the manifests
that require a dependency change; never hand-edit package records, checksums or
sources, delete it to resolve a rebase, or run a blanket dependency update.
Start from the current main branch's lock when reconciling changes.

## Verify and update

From the repository root, verify the complete delivered graph without changing
the lock or contacting a registry:

```sh
mise exec -- cargo metadata --offline --locked --format-version 1
```

Offline resolution requires the existing registry metadata and packages in the
local Cargo cache. A missing cache is not evidence of a stale lock. For an
intentional manifest change, let Cargo update only the required resolution:

```sh
mise exec -- cargo metadata --format-version 1
mise exec -- cargo metadata --offline --locked --format-version 1
mise exec -- cargo hakari generate
mise exec -- cargo hakari manage-deps
```

Review the manifest and lock diffs together, including package versions, sources
and newly requested features. Preserve the owning library's exact pins and
approved features. See [feature unification](development.md#feature-unification)
for the native unifier's target scope and exclusions. New members update the
closure when delivered; absent crates do not get placeholder lock entries.

## What the graph means

Full metadata includes package identities, versions, source identities, declared
licence expressions and dependency declarations with their owner, kind, target
and requested features. Its resolved graph records dependency edges and enabled
features. The default unfiltered graph is an all-target view with default
features, not proof that every feature builds on every target. Use
`--filter-platform <host-triple>` for a host projection and `--features <name>`
for explicit optional activation; these select different dimensions.

Registry checksums live in `Cargo.lock`; path dependencies have no registry
checksum. A lock records the native dependency closure, not a foreign package
tree or proof that a library implements its promised behavior.

The `dependency_locks` integration target in `maestro-test-conventions` verifies:

- Complete delivered-workspace metadata resolves twice with equal typed packages
  and graphs, leaving tracked manifests and lock bytes unchanged.
- Registry-free fixture resolution retains normal, build, development, disabled
  optional and inactive-target lock entries. Cold/cached graphs agree; optional
  activation and host projection select edges independently, retaining the
  ordinary dependency's requested feature.
- Independently stale declarations in each of those five classes fail locked
  resolution without rewriting the lock. A missing lock fails without creating
  it; restoring each input returns to the original passing graph.

Run the focused target:

```sh
mise exec -- cargo test -p maestro-test-conventions --locked --test dependency_locks
```

These tests invoke Cargo metadata, not recursive compilation or package installs.
Existing shared CI owns locked builds, platform checks and coverage.

## Licences and delivery ownership

[Approved library documents](libraries/) and owning manifests record selected
libraries, licences and feature choices. [Third-party notices](../THIRD-PARTY-NOTICES.md)
retain existing attributions. For dual licences, select an approved alternative;
for conjunctions, retain every required operand. Retain required copyright,
licence, disclaimer and applicable NOTICE texts when distributing artifacts.
Declared metadata alone does not prove redistribution compliance.

The shared dependency check owns licence, source and advisory validation;
shared unused-dependency checks and local feature-unification/conventions checks
keep their existing responsibilities. There is no second lock validator or
local licence policy here. Native assets and browser/component author tools stay
with their existing library, asset and packaging owners; lock verification does
not enable network retrieval or broaden dependency features.

Prepared package metadata, platform payloads, archive integrity, required notices
and consumer use without author tools await the real packaging implementation.
Executable startup awaits the composition root. This guide does not claim those
installation or runtime capabilities are qualified by metadata resolution.
