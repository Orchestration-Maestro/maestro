set positional-arguments

# Select the thin adapter for the host shell; all policy stays in Rust.
tooling := if os() == "windows" { "pwsh -NoProfile -File scripts/run-isolated.ps1" } else { "scripts/run-isolated.sh" }

# Install the pinned tools and both commit hook stages.
setup:
    mise install
    mise exec -- prek install

# Formatting, lint, public documentation and workspace conventions.
check:
    {{tooling}} isolate cargo fmt --all --check
    {{tooling}} isolate cargo clippy --workspace --all-targets --locked -- -D warnings
    {{tooling}} docs
    {{tooling}} isolate cargo test -p maestro-test-conventions --locked

# Run all workspace tests.
test *args:
    @echo "Running tests without API keys..."
    {{tooling}} isolate cargo test --workspace --locked "$@"

# Run the same checks as continuous integration.
ci: check test

# Format the workspace, then re-stage the files staged for the commit.
format-staged:
    {{tooling}} isolate {{tooling}} format-staged

# Capture staged paths before formatting, then check without the full suite.
pre-commit:
    {{tooling}} isolate {{tooling}} pre-commit

# Remove Cargo's declared artifact directory only.
clean:
    {{tooling}} isolate cargo clean

# Build every existing workspace member in dependency order.
build:
    {{tooling}} isolate cargo build --workspace --locked

# Rehearse author preparation without publishing.
prepublish: clean build check

# Watch delivered package owners after source/configuration changes.
dev:
    {{tooling}} isolate cargo run -p maestro-test-conventions --bin dev_watch --locked -- cargo --package maestro-models --package maestro-agent

# Watch the compiler-only subset; the browser owner activates its own selection.
dev-tsc:
    {{tooling}} isolate cargo run -p maestro-test-conventions --bin dev_watch --locked -- cargo --package maestro-models

# Watch one package while Cargo builds its actual dependencies.
dev-package package:
    {{tooling}} isolate cargo run -p maestro-test-conventions --bin dev_watch --locked -- cargo --package "$1"

# Inactive until its owning product artifacts exist.
browser-smoke:
    {{tooling}} inactive browser-smoke "#113 + browser/application product owners"

# Inactive until its owning product artifacts exist.
generate-models:
    {{tooling}} inactive generate-models "#135"

# Inactive until its owning product artifacts exist.
component-build:
    {{tooling}} inactive component-build "component author"

# Inactive until its owning product artifacts exist.
examples-check:
    {{tooling}} inactive examples-check "application"

# Inactive until its owning product artifacts exist.
profile-tui:
    {{tooling}} inactive profile-tui "#167"

# Inactive until its owning product artifacts exist.
profile-rpc:
    {{tooling}} inactive profile-rpc "#167"

# Inactive until its owning product artifacts exist.
binary:
    {{tooling}} inactive binary "#111"

# Inactive until its owning product artifacts exist.
assets:
    {{tooling}} inactive assets "application"

# Inactive until its owning product artifacts exist.
binary-assets:
    {{tooling}} inactive binary-assets "application"
