#!/bin/sh
# Nextest supplies its active artifact directory in the platform library search path.
set -eu
case "$(uname -s)" in
    Darwin) paths=${NEXTEST_DYLD_FALLBACK_LIBRARY_PATH-}; separator=: ;;
    MINGW*|MSYS*|CYGWIN*) paths=$PATH; separator=: ;;
    *) paths=${NEXTEST_LD_LIBRARY_PATH-}; separator=: ;;
esac
# Later setup scripts do not inherit the builder's exported environment.
prepare() {
    "$@"
    [ -n "${NEXTEST_ENV-}" ] || return 0
    : "${NEXTEST_RUN_ID:?missing nextest run identifier}"
    cp "$NEXTEST_ENV" "$directory/author-component-$NEXTEST_RUN_ID.env"
}

# Compile only the selected consumer family's artifact in its own executable namespace.
warm() {
    prepared="$directory/author-component-${NEXTEST_RUN_ID-}.env"
    if [ ! -r "$prepared" ]; then
        echo "Missing prepared component paths; author-component setup must run first: $prepared" >&2
        return 1
    fi
    while IFS= read -r entry; do
        export "$entry"
    done < "$prepared"
    export MAESTRO_WARM_COMPONENT="$1"
    test=compilation_cache::warm_selected_component
    found=false
    for binary in "$directory"/deps/integration-*; do
        [ -f "$binary" ] && [ -x "$binary" ] || continue
        if "$binary" --list --exact "$test" | grep -Fq "$test: test"; then
            found=true
            "$binary" --ignored --exact "$test" --nocapture
        fi
    done
    if [ "$found" = false ]; then
        echo 'Cannot find the runtime component cache warm entry point.' >&2
        return 1
    fi
}

IFS=$separator
for directory in $paths; do
    [ -d "$directory/deps" ] || continue
    if [ "$#" -gt 0 ]; then
        warm "$1"
        exit 0
    fi
    # The test build's own example keeps its resolved features; a single-package
    # `cargo run` resolves fewer and recompiles the engine. Use it only while it is
    # newer than its sources, so a filtered build never runs outdated preparation.
    for example in "$directory/examples/prepare_author_component" \
        "$directory/examples/prepare_author_component.exe"; do
        [ -f "$example" ] || continue
        fresh=true
        for source in crates/maestro-extensions-wasm/examples/prepare_author_component.rs \
            crates/maestro-extensions-wasm/src/tests/build.rs \
            crates/maestro-extensions-wasm/src/tests/build/components.rs \
            crates/maestro-extensions-wasm/src/tests/build/cargo.rs \
            crates/maestro-tooling/src/example_files.rs \
            crates/maestro-extensions-wasm/Cargo.toml Cargo.lock; do
            [ "$example" -nt "$source" ] || fresh=false
        done
        if [ "$fresh" = true ]; then
            prepare "$example"
            exit 0
        fi
    done
    profile=$(basename "$directory")
    [ "$profile" != debug ] || profile=dev
    prepare "${CARGO:-cargo}" run --quiet --locked -p maestro-extensions-wasm \
        --example prepare_author_component --target-dir "$(dirname "$directory")" \
        --profile "$profile"
    exit 0
done
echo 'Cannot resolve the nextest artifact directory from its library search path.' >&2
exit 1
