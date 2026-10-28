//! Platform command operands and lossless decoding of native path observations.

/// Expands a supplied glob as data and emits NUL-separated native filenames.
pub(crate) const GLOB: &str = "shopt -s globstar nullglob dotglob || exit 1; IFS=; matches=( $1 ); if ((${#matches[@]})); then printf '%s\\0' \"${matches[@]}\"; fi";

/// Creates or validates an owned private persistent temporary directory.
pub(crate) const TEMPORARY: &str = r#"if [[ ! -e "$1" && ! -L "$1" ]]; then
    mkdir -m 700 -p -- "$1" || exit 1
fi
[[ -d "$1" && -O "$1" ]] || { printf 'Sandbox temporary directory must be a directory you own\n' >&2; exit 1; }
cd -P -- "$1" || exit 1
[[ -d . && -O . ]] || exit 1
chmod 700 -- . || exit 1
printf '%s' "$PWD""#;

/// Decodes a native filename list without silently replacing invalid path bytes.
pub(crate) fn decode_matches(bytes: &[u8]) -> Result<Vec<String>, String> {
    std::str::from_utf8(bytes)
        .map(|paths| {
            paths
                .split('\0')
                .filter(|path| !path.is_empty())
                .map(str::to_owned)
                .collect()
        })
        .map_err(|error| error.to_string())
}
