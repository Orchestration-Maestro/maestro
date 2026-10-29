#!/bin/sh
# The pinned engine's cache namespace includes its consuming executable's mtime.
set -eu
directory=$1
extensions="rlib rmeta a so dylib dll"
# One line per workspace crate: its name, then it and every workspace crate it builds on,
# with hyphens as underscores. Registry crates are left out: a change in one reaches
# its dependents through the fingerprint hash, so their output times do not matter.
crates=$(awk '
    /^\[\[package\]\]/ { name = ""; listing = 0; next }
    /^name = / { name = $3; gsub(/"/, "", name); members[name] = 1; next }
    /^source = / { delete members[name]; next }
    /^dependencies = \[/ { listing = 1; next }
    listing && /^\]/ { listing = 0; next }
    listing { dependency = $1; gsub(/[",]/, "", dependency); edges[name] = edges[name] " " dependency }
    END {
        for (member in members) {
            delete seen
            seen[member] = 1
            stack[1] = member
            depth = 1
            line = member
            while (depth > 0) {
                current = stack[depth--]
                underscored = current
                gsub(/-/, "_", underscored)
                line = line " " underscored
                count = split(edges[current], listed, " ")
                for (index_ = 1; index_ <= count; index_++) {
                    if ((listed[index_] in members) && !(listed[index_] in seen)) {
                        seen[listed[index_]] = 1
                        stack[++depth] = listed[index_]
                    }
                }
            }
            print line
        }
    }' Cargo.lock 2>/dev/null) || crates=
# The newest library output, for an executable whose crate the lock file does not name.
library=
for extension in $extensions; do
    for artifact in "$directory"/deps/*."$extension"; do
        [ -f "$artifact" ] || continue
        if [ -z "$library" ] || [ "$artifact" -nt "$library" ]; then
            library=$artifact
        fi
    done
done
# Cargo rebuilds an executable only while a library output it builds on is newer than it,
# so an outdated executable must keep its time. A library's own test binary is built from
# the crate and not on its output.
outdated() {
    source=
    if [ -r "$1.d" ]; then
        IFS= read -r line <"$1.d" || true
        source=${line#*: }
        source=${source%% *}
    fi
    package=${source#crates/}
    package=${package%%/*}
    if [ "$package" = "$source" ] || ! known=$(printf '%s\n' "$crates" | grep "^$package "); then
        [ -n "$library" ] && [ "$library" -nt "$1" ]
        return
    fi
    own=$(printf '%s' "$package" | tr - _)
    named=false
    for member in $known; do
        if [ "$named" = false ]; then
            named=true
            continue
        fi
        if [ "$member" = "$own" ] && [ "$source" = "crates/$package/src/lib.rs" ]; then
            continue
        fi
        for extension in $extensions; do
            for output in "$directory"/deps/lib"$member"-*."$extension"; do
                if [ -f "$output" ] && [ "$output" -nt "$1" ]; then
                    return 0
                fi
            done
        done
    done
    return 1
}
found=false
stale=false
newest=
set --
for artifact in "$directory"/* "$directory"/deps/* "$directory"/examples/*; do
    [ -f "$artifact" ] && [ -x "$artifact" ] || continue
    case "${artifact##*/}" in
        *.exe) ;;
        *.*) continue ;;
    esac
    found=true
    if outdated "$artifact"; then
        case "$artifact" in
            "$directory"/examples/prepare_author_component \
                | "$directory"/examples/prepare_author_component.exe) stale=true ;;
        esac
        continue
    fi
    set -- "$@" "$artifact"
    if [ -z "$newest" ] || [ "$artifact" -nt "$newest" ]; then
        newest=$artifact
    fi
done
if [ "$found" = false ]; then
    echo "No consuming executables found in $directory." >&2
    exit 1
fi
# Advance older outputs only, so they remain newer than their compiler inputs.
for artifact do
    touch -r "$newest" "$artifact"
done
# The setup example warms code under the shared time, so an outdated one cannot serve; the
# launcher rebuilds it.
if [ "$stale" = true ]; then
    echo "The setup example is older than a library it builds on." >&2
    exit 3
fi
