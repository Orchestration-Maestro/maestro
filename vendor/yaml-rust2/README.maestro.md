# Local YAML dependency

This is the complete published yaml-rust2 0.13.0 archive from source commit
`e12069447073e0c8dd9ceab415159c4c92927534`, with archive SHA-256
`57e5b818a27a4cd30884ea380857a5e56f7ec3ba24a3990a3cc0b95af3238e18`.
The local scanner correction decodes adjacent four-digit high/low surrogate
escapes as one Unicode scalar, with a scanner regression test.

[Issue #381](https://github.com/Orchestration-Maestro/maestro/issues/381) tracks
contributing the correction upstream and removing the path patch after an
upstream release includes it. The original licence files and archive content
are retained; the scanner correction, its test and this file are the only changes.
