# Quote command text instead of argv: MSYS strips single quotes from native arguments.
set unstable
set lists

# Install the pinned tools and both commit hook stages.
setup:
    mise install
    mise exec -- prek install

# Formatting, lint, public documentation and the checkout's workspace conventions.
check:
    cargo fmt --all
    just _check --workspace
    if [ -f crates/maestro-models/examples/browser_import_check.rs ]; then just check-browser-smoke; fi

# Shared checks for explicit Cargo package-selection arguments.
[private]
_check *selection:
    cargo hakari verify
    @cargo clippy {{quote(selection)}} --all-targets --locked -- -D warnings
    @RUSTDOCFLAGS="-D warnings -D missing_docs" cargo doc {{quote(selection)}} --no-deps --locked
    cargo test -p maestro-test-conventions --locked --test integration -- --exact architecture::real_workspace_obeys_crate_rules dependency_locks::maestro_locked_resolution_keeps_checkout_clean

# Run all workspace tests.
test *args:
    @cargo nextest run --workspace --locked {{quote(args)}}
    @cargo test --workspace --locked --doc {{quote(args)}}

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
    @cargo nextest run -p maestro-models --locked {{quote(args)}}
    @cargo test -p maestro-models --locked --doc {{quote(args)}}

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
    @cargo nextest run -p maestro-agent --locked {{quote(args)}}
    @cargo test -p maestro-agent --locked --doc {{quote(args)}}

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
    @cargo nextest run -p maestro-tui --locked {{quote(args)}}
    @cargo test -p maestro-tui --locked --doc {{quote(args)}}

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
    @cargo nextest run -p maestro-app --locked {{quote(args)}}
    @cargo test -p maestro-app --locked --doc {{quote(args)}}

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
    @cargo nextest run -p maestro-tui --locked --test integration ansi_wrapping:: {{quote(args)}}

# Compile standalone prerequisites before copying prepared assets.
build-binary:
    just tui-build
    just models-build
    just agent-build
    just app-build
    cargo build -p maestro --locked
    just copy-binary-assets

# Copy prepared library assets.
copy-assets $source="crates/maestro-app" $destination="":
    cargo run --quiet --locked -p maestro-tooling --bin development -- copy-assets "$source" "${destination:-$(cargo run --quiet --locked -p maestro-tooling --bin development -- asset-output app)}"

# Copy prepared standalone assets and the supplied viewer bundle.
copy-binary-assets $source="crates/maestro-app" $destination="" $viewer="target/maestro-viewer" $package=(justfile_directory()):
    cargo run --quiet --locked -p maestro-tooling --bin development -- copy-binary-assets "$source" "${destination:-$(cargo run --quiet --locked -p maestro-tooling --bin development -- asset-output binary)}" "$viewer" "$package"

# Run the explicitly selected non-provider route in the invocation directory.
[no-cd]
test-offline *args:
    @cargo run --quiet --locked --manifest-path {{quote(justfile_directory() / "Cargo.toml")}} -p maestro-tooling --bin development -- test-offline {{quote(args)}}

# Launch source without changing the invocation directory.
[no-cd]
run-source *args:
    @cargo run --quiet --locked --manifest-path {{quote(justfile_directory() / "Cargo.toml")}} -p maestro-tooling --bin development -- run-source {{quote(args)}}

# Launch source with Windows switch comparison.
[no-cd]
run-source-windows *args:
    @cargo run --quiet --locked --manifest-path {{quote(justfile_directory() / "Cargo.toml")}} -p maestro-tooling --bin development -- run-source-windows {{quote(args)}}

# Build the author example as a native-async extension component.
extension-author-component:
    rustup target add wasm32-wasip2
    cargo build -p maestro-extensions-wasm --example author_component --target wasm32-wasip2 --release --locked

# Compile the real browser entry when its owner has supplied it.
check-browser-smoke:
    cargo run --quiet --locked -p maestro-tooling --bin development -- check-browser-smoke

# Remove only the prepared browser library output.
browser-clean:
    trunk clean --config {{quote(justfile_directory() / "crates/maestro-tooling/tests/fixtures/browser_author/Trunk.json")}} --dist {{quote(justfile_directory() / "target/maestro-browser/library")}}

# Prepare the isolated browser author library through native tools.
browser-library-build pack=(justfile_directory() / "assets/brand/brand.json") mode="dark" *args:
    @just _browser-build library {{quote(pack)}} {{quote(mode)}} {{quote(args)}}

# Watch shared browser inputs and serve the editable example.
browser-dev pack=(justfile_directory() / "assets/brand/brand.json") mode="dark" *args:
    @just _browser-watch serve {{quote(pack)}} {{quote(mode)}} --address 127.0.0.1 {{quote(args)}}

# Watch browser Rust and stylesheet inputs without a server.
browser-dev-compile pack=(justfile_directory() / "assets/brand/brand.json") mode="dark" *args:
    @just _browser-watch watch {{quote(pack)}} {{quote(mode)}} {{quote(args)}}

# Format and strictly check the delivered browser library and author fixture.
browser-check pack=(justfile_directory() / "assets/brand/brand.json") mode="dark":
    cargo run --quiet --locked -p maestro-tooling --bin development -- browser-author check {{quote(pack)}} {{quote(mode)}}

# Type-check shared example declarations without emitting a distribution.
browser-example-check pack=(justfile_directory() / "assets/brand/brand.json") mode="dark":
    cargo run --quiet --locked -p maestro-tooling --bin development -- browser-author example-check {{quote(pack)}} {{quote(mode)}}

# Serve the example through the single shared browser watch graph.
browser-example-dev pack=(justfile_directory() / "assets/brand/brand.json") mode="dark" *args:
    @just browser-dev {{quote(pack)}} {{quote(mode)}} {{quote(args)}}

# Prepare the example with the same locked author inputs as the library.
browser-example-build pack=(justfile_directory() / "assets/brand/brand.json") mode="dark" *args:
    @just _browser-build example {{quote(pack)}} {{quote(mode)}} {{quote(args)}}

# Serve prepared example bytes without invoking author compilation.
browser-example-preview *args:
    @miniserve {{quote(justfile_directory() / "target/maestro-browser/example")}} --interfaces 127.0.0.1 --index index.html --spa {{quote(args)}}

# Remove only the prepared browser example output.
browser-example-clean:
    trunk clean --config {{quote(justfile_directory() / "crates/maestro-tooling/tests/fixtures/browser_author/Trunk.json")}} --dist {{quote(justfile_directory() / "target/maestro-browser/example")}}

# Retain source locations and declarations without an optimizer pass.
browser-debug-build pack=(justfile_directory() / "assets/brand/brand.json") mode="dark" *args:
    @cargo run --quiet --locked -p maestro-tooling --bin development -- browser-author debug-build {{quote(pack)}} {{quote(mode)}} {{quote(args)}}

# Caller-owned shared build interface, failing before the next build on error.
browser-build pack=(justfile_directory() / "assets/brand/brand.json") mode="dark":
    just browser-library-build {{quote(pack)}} {{quote(mode)}}
    just browser-example-build {{quote(pack)}} {{quote(mode)}}

# One library operation prepares and dispatches the native author tools.
[private]
_browser-build output pack mode *args:
    @cargo run --quiet --locked -p maestro-tooling --bin development -- browser-author {{output}}-build {{quote(pack)}} {{quote(mode)}} {{quote(args)}}

# Trunk owns the only browser watch graph after shared native preparation.
[private]
_browser-watch command pack mode *args:
    @cargo run --quiet --locked -p maestro-tooling --bin development -- browser-author {{command}} {{quote(pack)}} {{quote(mode)}} {{quote(args)}}

# Build the runnable browser chat example, then serve it on loopback.
browser-chat-example *args:
    cargo build --locked -p maestro-web --example chat --target wasm32-unknown-unknown --release
    wasm-bindgen {{quote(justfile_directory() / "target/wasm32-unknown-unknown/release/examples/chat.wasm")}} --target web --out-dir {{quote(justfile_directory() / "target/maestro-browser/chat")}} --out-name chat
    cp {{quote(justfile_directory() / "crates/maestro-web/examples/chat/index.html")}} {{quote(justfile_directory() / "target/maestro-browser/chat/index.html")}}
    @miniserve {{quote(justfile_directory() / "target/maestro-browser/chat")}} --interfaces 127.0.0.1 --index index.html {{quote(args)}}

# Build and execute the retained sandbox exports with supplied browser tools.
browser-test:
    node crates/maestro-web/tests/sandbox_loading_support/run.cjs
    node crates/maestro-app/tests/browser/analysis_page.cjs

# Render the extension index from each example's own documentation.
example-index:
    cargo run --quiet --locked -p maestro-tooling --bin development -- example-index

# Check the tracked extension index without rewriting it.
example-index-check:
    cargo run --quiet --locked -p maestro-tooling --bin development -- example-index --check
