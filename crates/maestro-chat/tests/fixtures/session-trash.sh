#!/bin/sh
case "$MAESTRO_NATIVE_MODE" in
  zero) exit 0 ;;
  leading-dash)
    printf '%s\n' "$@" > "$MAESTRO_TRASH_LOG"
    exit 0
    ;;
  removed-nonzero)
    /bin/rm -- "$1"
    exit 1
    ;;
  *)
    printf 'trash native failure\nsecond line\n' >&2
    exit 1
    ;;
esac
