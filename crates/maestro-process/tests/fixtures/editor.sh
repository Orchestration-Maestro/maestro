#!/bin/sh
if [ "$1" = --custom ]; then shift; fi
printf '%s\n' "$1" > "$MAESTRO_EDITOR_RESULT"
exit 7
