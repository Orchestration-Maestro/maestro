#!/bin/sh
# Nextest supplies its active artifact directory in the platform library search path.
set -eu
case "$(uname -s)" in
    Darwin) paths=${NEXTEST_DYLD_FALLBACK_LIBRARY_PATH-}; separator=: ;;
    MINGW*|MSYS*|CYGWIN*) paths=${NEXTEST_PATH-}; separator=';' ;;
    *) paths=${NEXTEST_LD_LIBRARY_PATH-}; separator=: ;;
esac
IFS=$separator
for directory in $paths; do
    if [ "$separator" = ';' ]; then
        directory=$(cygpath -u "$directory")
    fi
    [ -d "$directory/deps" ] || continue
    profile=$(basename "$directory")
    [ "$profile" != debug ] || profile=dev
    exec "${CARGO:-cargo}" run --quiet --locked -p maestro-extensions-wasm \
        --example prepare_author_component --target-dir "$(dirname "$directory")" \
        --profile "$profile"
done
echo 'Cannot resolve the nextest artifact directory from its library search path.' >&2
exit 1
