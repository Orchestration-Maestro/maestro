#!/bin/sh
# Vendors every registry crate the tooling fixtures resolve, so their private Cargo homes never
# touch the shared package cache. Prints the source configuration's path and hands it to nextest.
# A directory keyed by the lock contents is reused across runs; one preparation per checkout runs
# at a time.
set -eu
fixture=crates/maestro-tooling/tests/fixtures/browser_author
key=$(cat Cargo.lock "$fixture/Cargo.lock" | cksum | cut -d ' ' -f 1)
directory=$PWD/target/fixture-sources/$key
if [ ! -f "$directory/config.toml" ]; then
    rm -rf "$directory"
    mkdir -p "$directory"
    "${CARGO:-cargo}" vendor --locked --offline --sync "$fixture/Cargo.toml" \
        "$directory/vendor" >"$directory/vendor.toml" 2>"$directory/vendor.log" || {
        cat "$directory/vendor.log" >&2
        exit 1
    }
    { cat "$directory/vendor.toml"; printf '\n[net]\noffline = true\n'; } >"$directory/config.toml"
fi
printf 'MAESTRO_TEST_CARGO_SOURCES=%s\n' "$directory/config.toml" >>"${NEXTEST_ENV:-/dev/null}"
printf '%s\n' "$directory/config.toml"
