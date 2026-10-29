#!/bin/sh
# Nextest supplies its active artifact directory in the platform library search path.
set -eu
prepared=false
if [ "${TEST_ARCHIVE-}" = 1 ] || {
    [ -n "${TEST_PREPARE_OUTPUT-}" ] && [ -f "$TEST_PREPARE_OUTPUT/components.json" ]
}; then
    prepared=true
fi
case "$(uname -s)" in
    Darwin) paths=${NEXTEST_DYLD_FALLBACK_LIBRARY_PATH-}; separator=: ;;
    MINGW*|MSYS*|CYGWIN*) paths=$PATH; separator=: ;;
    *) paths=${NEXTEST_LD_LIBRARY_PATH-}; separator=: ;;
esac
IFS=$separator
for directory in $paths; do
    [ -d "$directory/deps" ] || continue
    if [ "$prepared" = true ]; then
        # The staged example is as new as the archive's executables, so status 3 cannot occur.
        TEST_PREPARE_OUTPUT=${TEST_PREPARE_OUTPUT:-target/test-setup}
        export TEST_PREPARE_OUTPUT
        python3 "$(dirname "$0")/prepare-test-setup.py" export "$directory"
        sh "$(dirname "$0")/normalize-component-cache.sh" "$directory"
        # The example takes the prepared components from its environment, so it only warms.
        while IFS= read -r entry; do export "$entry"; done <"$NEXTEST_ENV"
        exec "$directory/examples/prepare_author_component"
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
            crates/maestro-extensions-wasm/src/tests/build/*.rs \
            crates/maestro-tooling/src/example_files.rs \
            crates/maestro-tooling/src/example_files/*.rs \
            crates/maestro-extensions-wasm/Cargo.toml Cargo.lock; do
            [ "$example" -nt "$source" ] || fresh=false
        done
        if [ "$fresh" = true ]; then
            status=0
            sh "$(dirname "$0")/normalize-component-cache.sh" "$directory" || status=$?
            if [ "$status" -eq 0 ]; then
                exec "$example"
            fi
            # Status 3: a library the example builds on is newer, so only a rebuild updates it.
            [ "$status" -eq 3 ] || exit "$status"
        fi
    done
    profile=$(basename "$directory")
    [ "$profile" != debug ] || profile=dev
    "${CARGO:-cargo}" build --quiet --locked -p maestro-extensions-wasm \
        --example prepare_author_component --target-dir "$(dirname "$directory")" \
        --profile "$profile"
    example="$directory/examples/prepare_author_component"
    [ -f "$example" ] || example="$example.exe"
    # Cargo ignores some library outputs that normalization counts, such as the metadata a
    # lint run writes, so the example can stay outdated. That only costs a cache miss.
    status=0
    sh "$(dirname "$0")/normalize-component-cache.sh" "$directory" || status=$?
    if [ "$status" -eq 3 ]; then
        echo 'Cargo kept the setup example; test binaries may compile components again.' >&2
    elif [ "$status" -ne 0 ]; then
        exit "$status"
    fi
    exec "$example"
done
echo 'Cannot resolve the nextest artifact directory from its library search path.' >&2
exit 1
