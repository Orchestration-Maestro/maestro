#!/bin/sh
printf '%s\n' "$*" >> "$MAESTRO_NATIVE_LOG"
if [ "${MAESTRO_NATIVE_FAIL:-}" = 1 ] && [ "${1:-}" != --version ]; then
    printf '%s\n' 'controlled native tool failure' >&2
    exit 17
fi
exec "$MAESTRO_NATIVE_TOOL" "$@"
