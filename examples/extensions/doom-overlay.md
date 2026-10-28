# DOOM overlay

Play DOOM in a terminal overlay with `/doom-overlay [WAD path]`. Q or Escape
pauses the retained game and closes the view. Reopening resumes the same game;
changing the WAD replaces the engine before opening the new selection.

## WAD files

An explicit path overrides discovery. Without one, the example checks the
component's directory, `./doom1.wad`, `./DOOM1.WAD`, `~/doom1.wad` and
`~/.doom/doom1.wad`, in that order. If no local file exists, it downloads the
shareware WAD beside the component. Custom WAD files are not required to match
the shareware checksum.

The download uses the [PureDOOM mirror pinned at commit
355cfbd16fac119718879239336ee2ea408886bd](https://raw.githubusercontent.com/Daivuk/PureDOOM/355cfbd16fac119718879239336ee2ea408886bd/doom1.wad).
It must match the official DOOM shareware v1.9 file:

- Size: 4,196,020 bytes.
- SHA-256: `1d7d43be501e67d927e415e0b8f3e29c3bf33075e859721816f652a526cac771`.
- MD5 identification: `f0cefca49926d00903cf57551d901abe`.

The example rejects a checksum mismatch before creating its cache file. A
network, body or filesystem failure produces a notification without opening a
game. No game data ships with Maestro.
