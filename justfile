set positional-arguments

# Install the pinned tools and both commit hook stages.
setup:
    mise install
    mise exec -- prek install

# Formatting, lint, public documentation and workspace conventions.
check:
    cargo fmt --all
    cargo hakari verify
    cargo clippy --workspace --all-targets --locked -- -D warnings
    RUSTDOCFLAGS="-D warnings -D missing_docs" cargo doc --workspace --no-deps --locked
    cargo test -p maestro-test-conventions --locked
    if [ -f crates/maestro-models/examples/browser_import_check.rs ]; then just check-browser-smoke; fi

# Run all workspace tests.
test *args:
    cargo nextest run --workspace --locked "$@"
    cargo test --workspace --locked --doc "$@"

# Run the same checks as continuous integration.
ci: check test

# Remove Cargo outputs without touching sources.
clean:
    cargo clean

# Compile all delivered workspace members.
build:
    just models-generate
    cargo build --workspace --locked

# Verify a distributable build without publishing it.
prepublish: clean build check

# Capture the index before formatting and verify the captured paths.
pre-commit:
    cargo run --quiet --locked -p maestro-tooling --bin development -- pre-commit

# Remove models compilation outputs only.
models-clean:
    cargo clean -p maestro-models

# Generate the native offline model catalog.
models-generate:
    cargo run --quiet --locked -p maestro-models --example generate_catalog_data

# Generate the catalog once before the explicit models build.
models-build:
    just models-generate
    cargo build -p maestro-models --locked

# Watch models compiler inputs without generating or copying assets.
models-dev:
    cargo pkgid -p maestro-models --locked
    cargo run --quiet --locked -p maestro-tooling --bin development -- watch build -p maestro-models --locked

# Run native models tests with literal argument forwarding.
models-test *args:
    cargo nextest run -p maestro-models --locked "$@"
    cargo test -p maestro-models --locked --doc "$@"

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
    cargo nextest run -p maestro-agent --locked "$@"
    cargo test -p maestro-agent --locked --doc "$@"

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
    cargo nextest run -p maestro-tui --locked "$@"
    cargo test -p maestro-tui --locked --doc "$@"

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
    cargo nextest run -p maestro-app --locked "$@"
    cargo test -p maestro-app --locked --doc "$@"

# Prepare app outputs without publication or root checks.
app-prepublish: app-clean app-build

# Watch existing Rust owners through the native Cargo watch graph.
native-dev:
    cargo run --quiet --locked -p maestro-tooling --bin development -- watch build --workspace --locked

# Watch every delivered owner through the existing native graphs.
[parallel]
dev: native-dev browser-dev

# Select compiler watches without starting an example server.
[parallel]
dev-compile: models-dev browser-dev-compile

# Select the model compiler only.
models-dev-compile: models-dev

# Run the terminal wrapping integration tests explicitly.
tui-test-ansi *args:
    cargo nextest run -p maestro-tui --locked --test integration ansi_wrapping:: "$@"

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

# Build the author example as a native-async extension component.
extension-author-component:
    rustup target add wasm32-wasip2
    cargo build -p maestro-extensions-wasm --example author_component --target wasm32-wasip2 --release --locked

# Compile the real browser entry when its owner has supplied it.
check-browser-smoke:
    cargo run --quiet --locked -p maestro-tooling --bin development -- check-browser-smoke

# Remove only the prepared browser library output.
browser-clean:
    trunk clean --config {{quote(justfile_directory() / "crates/maestro-tooling/tests/fixtures/browser_author/Trunk.toml")}} --dist {{quote(justfile_directory() / "target/maestro-browser/library")}}

# Prepare the isolated browser author library through native tools.
browser-library-build pack=(justfile_directory() / "assets/brand/brand.json") mode="dark" *args:
    shift 2; just _browser-build library {{quote(pack)}} {{quote(mode)}} "$@"

# Watch shared browser inputs and serve the editable example.
browser-dev pack=(justfile_directory() / "assets/brand/brand.json") mode="dark" *args:
    shift 2; MAESTRO_BRAND_PACK={{quote(pack)}} MAESTRO_BRAND_MODE={{quote(mode)}} trunk serve --config {{quote(justfile_directory() / "crates/maestro-tooling/tests/fixtures/browser_author/Trunk.toml")}} --locked --address 127.0.0.1 --dist {{quote(justfile_directory() / "target/maestro-browser/example")}} "$@"

# Watch browser Rust and stylesheet inputs without a server.
browser-dev-compile pack=(justfile_directory() / "assets/brand/brand.json") mode="dark" *args:
    shift 2; MAESTRO_BRAND_PACK={{quote(pack)}} MAESTRO_BRAND_MODE={{quote(mode)}} trunk watch --config {{quote(justfile_directory() / "crates/maestro-tooling/tests/fixtures/browser_author/Trunk.toml")}} --locked --dist {{quote(justfile_directory() / "target/maestro-browser/library")}} "$@"

# Format and strictly check the delivered browser library and author fixture.
browser-check pack=(justfile_directory() / "assets/brand/brand.json") mode="dark":
    cargo fmt -p maestro-web --check
    cargo fmt --manifest-path {{quote(justfile_directory() / "crates/maestro-tooling/tests/fixtures/browser_author/Cargo.toml")}} --check
    cargo clippy --no-deps -p maestro-web --target wasm32-unknown-unknown --locked -- -D warnings
    MAESTRO_BRAND_PACK={{quote(pack)}} MAESTRO_BRAND_MODE={{quote(mode)}} cargo clippy --manifest-path {{quote(justfile_directory() / "crates/maestro-tooling/tests/fixtures/browser_author/Cargo.toml")}} --target wasm32-unknown-unknown --locked -- -D warnings
    just browser-example-check {{quote(pack)}} {{quote(mode)}}

# Type-check shared example declarations without emitting a distribution.
browser-example-check pack=(justfile_directory() / "assets/brand/brand.json") mode="dark":
    MAESTRO_BRAND_PACK={{quote(pack)}} MAESTRO_BRAND_MODE={{quote(mode)}} cargo check --manifest-path {{quote(justfile_directory() / "crates/maestro-tooling/tests/fixtures/browser_author/Cargo.toml")}} --target wasm32-unknown-unknown --locked

# Serve the example through the single shared browser watch graph.
browser-example-dev pack=(justfile_directory() / "assets/brand/brand.json") mode="dark" *args:
    shift 2; just browser-dev {{quote(pack)}} {{quote(mode)}} "$@"

# Prepare the example with the same locked author inputs as the library.
browser-example-build pack=(justfile_directory() / "assets/brand/brand.json") mode="dark" *args:
    shift 2; just _browser-build example {{quote(pack)}} {{quote(mode)}} "$@"

# Serve prepared example bytes without invoking author compilation.
browser-example-preview *args:
    miniserve {{quote(justfile_directory() / "target/maestro-browser/example")}} --interfaces 127.0.0.1 --index index.html --spa "$@"

# Remove only the prepared browser example output.
browser-example-clean:
    trunk clean --config {{quote(justfile_directory() / "crates/maestro-tooling/tests/fixtures/browser_author/Trunk.toml")}} --dist {{quote(justfile_directory() / "target/maestro-browser/example")}}

# Retain source locations and declarations without an optimizer pass.
browser-debug-build pack=(justfile_directory() / "assets/brand/brand.json") mode="dark" *args:
    shift 2; MAESTRO_BRAND_PACK={{quote(pack)}} MAESTRO_BRAND_MODE={{quote(mode)}} trunk build {{quote(justfile_directory() / "crates/maestro-tooling/tests/fixtures/browser_author/debug.html")}} --config {{quote(justfile_directory() / "crates/maestro-tooling/tests/fixtures/browser_author/Trunk.toml")}} --locked --dist {{quote(justfile_directory() / "target/maestro-browser/debug")}} "$@"

# Caller-owned shared build interface, failing before the next build on error.
browser-build pack=(justfile_directory() / "assets/brand/brand.json") mode="dark":
    just browser-library-build {{quote(pack)}} {{quote(mode)}}
    just browser-example-build {{quote(pack)}} {{quote(mode)}}

# One optimized build path owns the fixture, asset inputs and tool pins.
[private]
_browser-build output pack mode *args:
    shift 3; MAESTRO_BRAND_PACK={{quote(pack)}} MAESTRO_BRAND_MODE={{quote(mode)}} trunk build --config {{quote(justfile_directory() / "crates/maestro-tooling/tests/fixtures/browser_author/Trunk.toml")}} --locked --release --dist {{quote(justfile_directory() / "target/maestro-browser" / output)}} "$@"
