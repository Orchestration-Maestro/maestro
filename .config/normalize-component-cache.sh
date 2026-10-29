#!/bin/sh
# The pinned engine's cache namespace includes its consuming executable's mtime.
set -eu
directory=$1
# Cargo rebuilds an executable only while a dependency's output is newer than it, so an
# executable older than any library output is outdated and must stay that way.
library=
for artifact in "$directory"/deps/*.rlib "$directory"/deps/*.rmeta "$directory"/deps/*.a \
    "$directory"/deps/*.so "$directory"/deps/*.dylib "$directory"/deps/*.dll; do
    [ -f "$artifact" ] || continue
    if [ -z "$library" ] || [ "$artifact" -nt "$library" ]; then
        library=$artifact
    fi
done
found=false
newest=
set --
for artifact in "$directory"/* "$directory"/deps/* "$directory"/examples/*; do
    [ -f "$artifact" ] && [ -x "$artifact" ] || continue
    case "${artifact##*/}" in
        *.exe) ;;
        *.*) continue ;;
    esac
    found=true
    if [ -n "$library" ] && [ "$library" -nt "$artifact" ]; then
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
