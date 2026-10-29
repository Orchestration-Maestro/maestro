#!/bin/sh
# The pinned engine's cache namespace includes its consuming executable's mtime.
set -eu
directory=$1
newest=
set --
for artifact in "$directory"/* "$directory"/deps/* "$directory"/examples/*; do
    [ -f "$artifact" ] && [ -x "$artifact" ] || continue
    case "${artifact##*/}" in
        *.exe) ;;
        *.*) continue ;;
    esac
    set -- "$@" "$artifact"
    if [ -z "$newest" ] || [ "$artifact" -nt "$newest" ]; then
        newest=$artifact
    fi
done
if [ -z "$newest" ]; then
    echo "No consuming executables found in $directory." >&2
    exit 1
fi
# Advance older outputs only, so they remain newer than their compiler inputs.
for artifact do
    touch -r "$newest" "$artifact"
done
