#!/bin/sh
case "$1" in
    metadata) . "$MAESTRO_FAKE_CARGO_SCENARIO" ;;
    -vV) . "$MAESTRO_FAKE_RUSTC_SCENARIO" ;;
    *) exit 2 ;;
esac
