# Tool output libraries

`maestro-tools` uses workspace-pinned `serde =1.0.229` with derive and default
`std` for output serialization. Records use camelCase field names; reasons use
lowercase strings. No output record is deserialized by the product.

Workspace-pinned `ryu-js =1.0.3` supplies number spelling and one-decimal fixed
formatting for [size text](../tools-read.md#line-clipping-and-size-text). Use its
Apache-2.0 license option. It also formats image coordinate notes to two places. Serde and its derive closure have permitted MIT or
Apache-2.0 options; unicode-ident retains Unicode-3.0.

Tests use workspace `serde_json =1.0.151` with `preserve_order` and
`float_roundtrip` to compare independently recorded output trees.

Native [streaming output](../tools-read.md#native-streaming) additionally uses
`maestro-path` for lexical filename joining and `getrandom =0.4.3`, with no extra
features, for artifact identifiers. Its native dependency closure uses existing
`cfg-if =1.0.5`, `libc =0.2.190` and target-specific `r-efi =6.0.0`; use their
MIT or Apache-2.0 license options. Files and buffered writes use the standard
library. The streaming module is excluded from browser builds.

## Tool definitions and read

Generic definitions and wrappers build on native and browser targets with the
internal `maestro-agent`, `maestro-models`, `maestro-theme` and `maestro-tui`
crates, workspace `serde_json` (now also a normal dependency) and workspace
`futures-util =0.3.34` (`std`, `sink`). The read family is native-only:
`icu_normalizer =2.3.0` with `compiled_data` supplies NFD for read paths and NFKC for
[edit matching](../tools-edit.md#original-snapshot-matching), `tokio =1.53.3` with
`rt`, `fs` and `sync` supplies file reads, file writes, blocking probes and the
one-shot wake-ups of the [file mutation queue](../tools-edit.md#file-mutation-queue)
(`sync` adds no package and was already enabled in the workspace), and on Unix
`rustix =1.1.5` with `std` and `fs` checks readability for reads/previews and
read/write access for edits without opening the file.
Other targets open the file. The write family is native-only as well. No package
or version beyond the existing lockfile is added.
Read selection spells floating-point line numbers with `ryu-js`.

On Unix a present `HOME` is used verbatim, including an empty value, and
[`std::env::home_dir`](https://doc.rust-lang.org/std/env/fn.home_dir.html)
runs only when `HOME` is absent; Windows always uses that platform lookup. A missing entry is a lookup error returned only for `~` paths.

## Directory listing

The [`ls` tool](../tools-search.md) adds no package, feature or version. It orders names with
`maestro-theme`'s [collator](../theme.md#files-environment-and-sorting), which uses the theme crate's
existing `icu_collator`/`icu_locale_core` pins, reads native names through the shared
directory listing (`maestro-listing`), spells numbers with `ryu-js` and runs blocking file effects on the
existing `tokio` blocking pool. Its call and result renderers reuse the read renderers' shared
display helpers rather than adding their own.

## Display strings

`maestro-text` and `maestro-tools` use `regex =1.13.1` with default features (MIT or Apache-2.0), already
in the lockfile through `maestro-models`, for the text leaf's cached expression that removes the terminal
escape sequences it matches from [display text](../tools-read.md#display-helpers). Its existing
closure is `regex-automata 0.4.18`, `regex-syntax 0.8.11`, `aho-corasick 1.1.5` and
`memchr 2.8.3`; the last two are Unlicense OR MIT, used under MIT. No other package is added.

## Image libraries

Image operations use `image =0.24.9` (MIT or Apache-2.0) directly, with
`default-features = false`. The `png` and `jpeg` features supply decoding and
candidate encoding; `gif`, `webp`, `bmp`, `ico`, `pnm`, `tiff`, `hdr`, `dds`
and `farbfeld` retain byte-decoder inputs. `dds` enables `dxt` for its compressed
pixel blocks. These inputs are available to conversion and resizing independently
of the MIME detector's supported formats.
The [byte decoder](https://docs.rs/image/0.24.9/image/fn.load_from_memory.html)
provides fallible decoding. RGBA images use Lanczos3 resizing, axis flips and
integer quarter-turns from `image::imageops`; `DynamicImage::write_to` encodes
PNG or JPEG at the requested quality. The same features build on native and
browser targets, without a browser-specific codec wrapper.

`kamadak-exif =0.6.1` (BSD-2-Clause), declared as `exif`, owns TIFF/JPEG metadata decoding. Ordered
`fields()` selection uses the first retained primary orientation, with
partial-result recovery for metadata errors. The utility owns only the WebP chunk
envelope and optional prefix.
`base64 =0.23.1` uses defaults off plus `std` here (MIT or Apache-2.0); shared
workspace features also enable its defaults. The image library uses
`jpeg-decoder =0.3.2` (MIT or Apache-2.0) with defaults off and no extra features;
there is no direct decoder declaration or cargo-machete exception.

Native JPEG decoding uses runtime-selected SIMD IDCT and colour conversion,
which can differ from the scalar decoding path. The measured maximum absolute
per-channel difference across the archived eighteen-image corpus is 3. The three
reencoded JPEG-input observations compare format, dimensions and pixels within
that bound; deterministic paths retain exact encoded-byte comparisons.
Workspace `num-traits =0.2.19` supplies checked rounded geometry conversion;
its existing image-library dependency is reused, with no new package or version.

### Resolved image dependency closure

The table records the resolved native/browser feature union for the image,
base64, EXIF and number roots. Choose MIT or Apache-2.0 when offered (including
0BSD/Zlib/Unlicense alternatives); retain BSD, Apache-2.0 and Unicode-3.0 when
those are the applicable terms. This is dependency attribution, not an API or
extra capability.

| Package | Version | Resolved features | License expression |
|---|---|---|---|
| adler2 | 2.0.1 | none | 0BSD OR MIT OR Apache-2.0 |
| autocfg | 1.5.1 | none | Apache-2.0 OR MIT |
| base64 | 0.23.1 | alloc, default, simd-unsafe, std | MIT OR Apache-2.0 |
| bitflags | 1.3.2 | default | MIT/Apache-2.0 |
| bytemuck | 1.25.2 | extern_crate_alloc | Zlib OR Apache-2.0 OR MIT |
| byteorder | 1.5.0 | default, std | Unlicense OR MIT |
| cfg-if | 1.0.5 | none | MIT OR Apache-2.0 |
| color_quant | 1.1.0 | none | MIT |
| crc32fast | 1.5.2 | default, std | MIT OR Apache-2.0 |
| fdeflate | 0.3.7 | none | MIT OR Apache-2.0 |
| flate2 | 1.1.10 | any_impl, default, miniz_oxide, runtime_detection, rust_backend | MIT OR Apache-2.0 |
| gif | 0.13.3 | color_quant, default, raii_no_panic, std | MIT OR Apache-2.0 |
| image | 0.24.9 | bmp, dds, dxt, farbfeld, gif, hdr, ico, jpeg, png, pnm, tiff, webp | MIT OR Apache-2.0 |
| jpeg-decoder | 0.3.2 | none | MIT OR Apache-2.0 |
| kamadak-exif | 0.6.1 | none | BSD-2-Clause |
| miniz_oxide | 0.8.9 | default, simd, simd-adler32, with-alloc | MIT OR Zlib OR Apache-2.0 |
| miniz_oxide | 0.9.1 | default, simd, simd-adler32, with-alloc | MIT OR Zlib OR Apache-2.0 |
| mutate_once | 0.1.2 | none | BSD-2-Clause |
| num-traits | 0.2.19 | default, i128, std | MIT OR Apache-2.0 |
| png | 0.17.16 | none | MIT OR Apache-2.0 |
| ryu-js | 1.0.3 | none | Apache-2.0 OR BSL-1.0 |
| simd-adler32 | 0.3.10 | const-generics, default, std | MIT |
| tiff | 0.9.1 | none | MIT |
| weezl | 0.1.12 | alloc, default, std | MIT OR Apache-2.0 |
| zlib-rs | 0.6.8 | rust-allocator, std | Zlib |

## Edit diff

The native [numbered edit diff](../tools-edit.md#numbered-diffs-and-read-only-previews)
and portable [word-level rendering](../tools-edit.md#diff-rendering)
share one owned sequence-selection core, derived from diff 8.0.4.
It adds no runtime dependency. The BSD-3-Clause attribution is retained in
[the third-party notices](../../THIRD-PARTY-NOTICES.md#edit-diff).

## Search program acquisition

Native `maestro-tools` uses `zip =9.0.3` (MIT), default features disabled,
with `deflate-flate2`, and `owo-colors =4.2.1` (MIT, no optional features).
Tar extraction stays the system `tar` command. HTTP uses the existing models
transport rather than a second client. The shared
[archive closure](document-formats.md) retains the spreadsheet parser's deflate
features; the direct tool edge adds no codecs.

Default ZIP extraction strips SUID, SGID and sticky bits, rejects symlink targets
that escape the extraction destination (including through an intermediate
symlink), and avoids permission changes through extracted symlinks. Contained
symlinks remain supported. These upstream security fixes are covered by
[RUSTSEC-2026-0367](https://rustsec.org/advisories/RUSTSEC-2026-0367.html),
[RUSTSEC-2026-0368](https://rustsec.org/advisories/RUSTSEC-2026-0368.html) and
[RUSTSEC-2026-0369](https://rustsec.org/advisories/RUSTSEC-2026-0369.html), including
the subsequent non-streaming permission fix in ZIP 9.0.1. Installer staging and
publication ownership are unchanged; no whole-filesystem atomicity is promised.

## File search

Native `maestro-tools` uses workspace-pinned `globset =0.4.20` (MIT or Unlicense;
use MIT) with default features disabled, and workspace-pinned `regex-syntax =0.8.11`
(MIT or Apache-2.0) with only `std`. fd matches a full-path glob against the native
absolute path, which Windows spells with `\`, so on Windows the find tool compiles
the glob with globset as fd does, parses that regex with regex-syntax, widens every
separator it matches to `/` or `\`, and passes the printed regex to fd. Other hosts
pass the glob unchanged. The closure adds `bstr =1.13.1` (MIT or Apache-2.0);
`aho-corasick`, `regex-automata` and `regex-syntax` were already locked.

## Local shell operations

Native [local shell operations](../tools-process.md#local-shell-operations) use
`indexmap =2.14.2` with default features for ordered native environment entries
and detached process IDs. Use its MIT or Apache-2.0 license option. Native process
effects reuse [the process owner](process.md).
