# Native clipboard library

`maestro-app` pins `clipboard-rs =0.2.4` with default features disabled, under MIT.
It is included on Windows and Unix excluding iOS, Android and Emscripten; wasm32
links no clipboard library. Unsupported hosts retain tool and terminal fallback.
Each native text write, image probe and PNG read creates a fresh context. Native admission caches eligibility,
not a context; the default module is available only on macOS, Linux and Windows.
It is disabled in Termux and requires a display on Linux.

The image dependency is unconditional, but no additional image or watcher feature
is enabled. The adapter uses image availability and PNG encoding from the native
library; raw helper conversion reuses the tools codec owner. Native temporary
paths use the existing UUID 1.27.0 with defaults disabled and `std`, `v4` enabled.
Application pixel tests use the existing image 0.24.9 with only `png` requested.
Existing base64 0.23.1 uses
`std` without defaults, futures-util 0.3.34 retains workspace features, and Tokio
1.53.3 provides native `rt`, `time` and `process` without defaults.

The original text-clipboard integration added 39 registry packages. Image
reading reuses that resolution without adding, removing or replacing registry
package versions. The desktop closure includes X11, Objective-C and
Windows bindings plus image decoding dependencies. Their license alternatives
are MIT, Apache-2.0, BSD-3-Clause, Zlib or Unlicense, except exactly these
Windows backend packages under BSL-1.0:

| Package | Version |
|---|---|
| clipboard-win | 5.4.3 |
| error-code | 3.4.0 |
| windows-win | 3.0.0 |

Shared CI's default license allow list includes BSL-1.0, so these packages need
no exception. Attribution and the full license text are in
[third-party notices](../../THIRD-PARTY-NOTICES.md#native-clipboard-windows-backend).
