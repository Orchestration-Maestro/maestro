#!/usr/bin/env bash
set -euo pipefail
root=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)
# Compile without Cargo so its target runner can bootstrap without recursion.
rustc=$(cd -- "$root" && rustup which rustc)
output="$root/target/repository-tools"
mkdir -p -- "$(dirname -- "$output")"
rebuild=false
[[ -x "$output" ]] || rebuild=true
for source in "$root/crates/maestro-test-conventions/src/bin/repository_tools.rs" "$root"/crates/maestro-test-conventions/src/repository_tools/*.rs; do
    [[ "$source" -nt "$output" ]] && rebuild=true
done
if "$rebuild"; then
    temporary=$(mktemp -d "$(dirname -- "$output")/tooling-bootstrap.XXXXXXXX")
    trap 'rm -rf -- "$temporary"' EXIT
    "$rustc" --edition=2024 "$root/crates/maestro-test-conventions/src/bin/repository_tools.rs" -o "$temporary/repository-tools"
    mv -f -- "$temporary/repository-tools" "$output"
    rm -rf -- "$temporary"
    trap - EXIT
fi
exec "$output" "$@"
