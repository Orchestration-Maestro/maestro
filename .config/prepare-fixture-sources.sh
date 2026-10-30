#!/bin/sh
# Vendors the registry crates the tooling fixtures resolve, so their private Cargo homes never
# touch the shared package cache. Prints the source configuration's path and hands it to nextest.
# Cargo reads every vendored package on each run, so the set holds only the fixture closure: the
# crates named by the registry_sources manifest, at the versions the workspace lock selects, and
# the browser fixture's crates. A directory keyed by those inputs is reused across runs; one
# preparation per checkout runs at a time.
set -eu
fixtures=crates/maestro-tooling/tests/fixtures
key=$(cat Cargo.lock "$fixtures/browser_author/Cargo.lock" "$fixtures/registry_sources/Cargo.toml" |
    cksum | cut -d ' ' -f 1)
directory=$PWD/target/fixture-sources/$key
if [ ! -f "$directory/config.toml" ]; then
    rm -rf "$directory"
    mkdir -p "$directory"
    cp -R "$fixtures/registry_sources" "$directory/closure"
    cp Cargo.lock "$directory/closure/Cargo.lock"
    "${CARGO:-cargo}" vendor --offline --manifest-path "$directory/closure/Cargo.toml" \
        --sync "$fixtures/browser_author/Cargo.toml" "$directory/vendor" \
        >"$directory/vendor.toml" 2>"$directory/vendor.log" || {
        cat "$directory/vendor.log" >&2
        exit 1
    }
    { cat "$directory/vendor.toml"; printf '\n[net]\noffline = true\n'; } >"$directory/config.toml"
fi
printf 'MAESTRO_TEST_CARGO_SOURCES=%s\n' "$directory/config.toml" >>"${NEXTEST_ENV:-/dev/null}"
printf '%s\n' "$directory/config.toml"
