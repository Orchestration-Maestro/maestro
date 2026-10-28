#!/bin/sh
# Nextest supplies its active artifact directory in the platform library search path.
set -eu
case "$(uname -s)" in
    Darwin) paths=${NEXTEST_DYLD_FALLBACK_LIBRARY_PATH-}; separator=: ;;
    MINGW*|MSYS*|CYGWIN*) paths=$PATH; separator=: ;;
    *) paths=${NEXTEST_LD_LIBRARY_PATH-}; separator=: ;;
esac
# Compile in the consuming executable's cache namespace, not only the setup example's.
prepare() {
    "$@"
    [ -n "${NEXTEST_ENV-}" ] || return 0
    while IFS= read -r entry; do
        export "$entry"
    done < "$NEXTEST_ENV"
    test=compilation_cache::warm_prepared_components
    for binary in "$directory"/deps/integration-*; do
        [ -f "$binary" ] && [ -x "$binary" ] || continue
        if "$binary" --list --exact "$test" | grep -Fq "$test: test"; then
            "$binary" --ignored --exact "$test" --nocapture
        fi
    done
}

IFS=$separator
for directory in $paths; do
    [ -d "$directory/deps" ] || continue
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
