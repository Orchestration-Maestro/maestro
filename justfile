set positional-arguments

# Install the pinned tools and both commit hook stages.
setup:
    mise install
    mise exec -- prek install

# Formatting, lint, public documentation and workspace conventions.
check:
    cargo fmt --all
    cargo clippy --workspace --all-targets --locked -- -D warnings
    RUSTDOCFLAGS="-D warnings -D missing_docs" cargo doc --workspace --no-deps --locked
    cargo test -p maestro-test-conventions --locked
    if [ -f crates/maestro-models/examples/browser_import_check.rs ]; then just check-browser-smoke; fi

# Run all workspace tests.
test *args:
    cargo test --workspace --locked "$@"

# Run the same checks as continuous integration.
ci: check test

# Remove Cargo outputs without touching sources.
clean:
    cargo clean

# Compile all delivered workspace members.
build:
    cargo build --workspace --locked

# Verify a distributable build without publishing it.
prepublish: clean build check

# Capture the index before formatting and verify the captured paths.
pre-commit:
    cargo run --quiet --locked -p maestro-tooling --bin development -- pre-commit

# Remove models compilation outputs only.
models-clean:
    cargo clean -p maestro-models

# Compile the models package.
models-build:
    cargo build -p maestro-models --locked

# Watch models compiler inputs without generating or copying assets.
models-dev:
    cargo pkgid -p maestro-models --locked
    cargo run --quiet --locked -p maestro-tooling --bin development -- watch build -p maestro-models --locked

# Run native models tests with literal argument forwarding.
models-test *args:
    cargo test -p maestro-models --locked "$@"

# Prepare models outputs without publication or root checks.
models-prepublish: models-clean models-build

# Remove agent compilation outputs only.
agent-clean:
    cargo clean -p maestro-agent

# Compile the agent package.
agent-build:
    cargo build -p maestro-agent --locked

# Watch agent compiler inputs without generating or copying assets.
agent-dev:
    cargo pkgid -p maestro-agent --locked
    cargo run --quiet --locked -p maestro-tooling --bin development -- watch build -p maestro-agent --locked

# Run native agent tests with literal argument forwarding.
agent-test *args:
    cargo test -p maestro-agent --locked "$@"

# Prepare agent outputs without publication or root checks.
agent-prepublish: agent-clean agent-build

# Remove tui compilation outputs only.
tui-clean:
    cargo clean -p maestro-tui

# Compile the tui package.
tui-build:
    cargo build -p maestro-tui --locked

# Watch tui compiler inputs without generating or copying assets.
tui-dev:
    cargo pkgid -p maestro-tui --locked
    cargo run --quiet --locked -p maestro-tooling --bin development -- watch build -p maestro-tui --locked

# Run native tui tests with literal argument forwarding.
tui-test *args:
    cargo test -p maestro-tui --locked "$@"

# Prepare tui outputs without publication or root checks.
tui-prepublish: tui-clean tui-build

# Remove app compilation outputs only.
app-clean:
    cargo clean -p maestro-app
    cargo run --quiet --locked -p maestro-tooling --bin development -- clean-assets

# Compile the app package.
app-build:
    cargo build -p maestro-app --locked
    just copy-assets

# Watch app compiler inputs without generating or copying assets.
app-dev:
    cargo pkgid -p maestro-app --locked
    cargo run --quiet --locked -p maestro-tooling --bin development -- watch build -p maestro-app --locked

# Run native app tests with literal argument forwarding.
app-test *args:
    cargo test -p maestro-app --locked "$@"

# Prepare app outputs without publication or root checks.
app-prepublish: app-clean app-build

# Watch every delivered owner, retaining compiler output.
dev:
    cargo run --quiet --locked -p maestro-tooling --bin development -- watch build --workspace --locked

# Select the model compiler without regeneration or asset copying.
dev-compile: models-dev

# Select the model compiler only.
models-dev-compile: models-dev

# Run the terminal wrapping subset explicitly.
tui-test-ansi *args:
    cargo test -p maestro-tui --locked wrap_ansi "$@"

# Compile standalone prerequisites before copying prepared assets.
build-binary:
    just tui-build
    just models-build
    just agent-build
    just app-build
    cargo build -p maestro --locked
    just copy-binary-assets

# Copy prepared library assets.
copy-assets source="crates/maestro-app" destination="":
    cargo run --quiet --locked -p maestro-tooling --bin development -- copy-assets "$1" "${2:-$(cargo run --quiet --locked -p maestro-tooling --bin development -- asset-output app)}"

# Copy prepared standalone assets and the supplied viewer bundle.
copy-binary-assets source="crates/maestro-app" destination="" viewer="target/maestro-viewer":
    cargo run --quiet --locked -p maestro-tooling --bin development -- copy-binary-assets "$1" "${2:-$(cargo run --quiet --locked -p maestro-tooling --bin development -- asset-output binary)}" "$3"

# Run the explicitly selected non-provider route in the invocation directory.
[no-cd]
test-offline *args:
    cargo run --quiet --locked --manifest-path {{quote(justfile_directory() / "Cargo.toml")}} -p maestro-tooling --bin development -- test-offline "$@"

# Launch source without changing the invocation directory.
[no-cd]
run-source *args:
    cargo run --quiet --locked --manifest-path {{quote(justfile_directory() / "Cargo.toml")}} -p maestro-tooling --bin development -- run-source "$@"

# Launch source with Windows switch comparison.
[no-cd]
run-source-windows *args:
    cargo run --quiet --locked --manifest-path {{quote(justfile_directory() / "Cargo.toml")}} -p maestro-tooling --bin development -- run-source-windows "$@"

# Compile the real browser entry when its owner has supplied it.
check-browser-smoke:
    cargo run --quiet --locked -p maestro-tooling --bin development -- check-browser-smoke

# Build the example extension component and run the callback prototype checks.
extension-callbacks-prototype:
    rustup target add wasm32-wasip2
    cargo build --manifest-path prototypes/extension-callbacks/Cargo.toml --locked -p extension-example-component --target wasm32-wasip2
    cargo test --manifest-path prototypes/extension-callbacks/Cargo.toml --locked --workspace
