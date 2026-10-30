#!/bin/sh
# Builds the browser graph the browser author tests share, once before they run.
#
# A shard starts with an empty target directory and no compile cache. Without this
# script the first browser test of the shard compiles the native bootstrap and the
# WebAssembly fixture itself, while a thousand other tests compete for the same cores,
# and the slowest of them reaches the hang limit. The recipes below run in this checkout
# with the runner's instrumentation removed, as the tests remove it from their own
# children, and in the target directory the tests are told to use, spelled the same way
# because Cargo records that spelling in what it compiles. A recipe that fails here only
# loses the saving: the tests run the same recipes and report their own failures.
set -eu
target="$PWD/target/browser-author"
if [ -n "${__CARGO_LLVM_COV_RUSTC_WRAPPER-}" ]; then
    if [ -n "${__CARGO_LLVM_COV_RUSTC_WRAPPER_PRE_EXISTING-}" ]; then
        RUSTC_WRAPPER=$__CARGO_LLVM_COV_RUSTC_WRAPPER_PRE_EXISTING
        export RUSTC_WRAPPER
    else
        unset RUSTC_WRAPPER
    fi
fi
unset RUSTFLAGS CARGO_ENCODED_RUSTFLAGS LLVM_PROFILE_FILE CARGO_LLVM_COV \
    CARGO_LLVM_COV_TARGET_DIR CARGO_LLVM_COV_BUILD_DIR CARGO_LLVM_COV_SHOW_ENV \
    __CARGO_LLVM_COV_RUSTC_WRAPPER __CARGO_LLVM_COV_RUSTC_WRAPPER_PRE_EXISTING
attempt() {
    "$@" || echo "warning: $* failed; the browser tests report it themselves" >&2
}
export CARGO_TARGET_DIR="$target" CARGO_BUILD_JOBS=3 TRUNK_SKIP_VERSION_CHECK=true
# The library lint of `browser-check`, without its formatting repair, which would rewrite sources.
attempt cargo clippy --no-deps -p maestro-web --target wasm32-unknown-unknown --locked -- -D warnings
for recipe in browser-example-check browser-library-build browser-debug-build; do
    attempt just "$recipe"
done
printf 'MAESTRO_BROWSER_TARGET_DIR=%s\n' "$target" >>"${NEXTEST_ENV:-/dev/null}"
