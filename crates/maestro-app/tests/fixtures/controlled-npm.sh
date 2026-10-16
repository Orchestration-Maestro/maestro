#!/bin/sh
# A deterministic local package-command fixture; never runs a package manager.
set -eu
line="$0"
for argument in "$@"; do line="$line	$argument"; done
printf '%s\t%s\n' "$PWD" "$line" >> "$MAESTRO_COMMAND_LOG"
test "$1" = --quiet
case "$2" in
    root) printf '%s\n' "$MAESTRO_NPM_ROOT" ;;
    view) printf '"2.0.0"' ;;
    install) exit 0 ;;
    *) exit 1 ;;
esac
