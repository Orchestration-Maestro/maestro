## Approved package-pattern libraries

`maestro-pattern` uses `brace-expansion =0.1.1` with no features or transitive
dependencies (MIT OR Apache-2.0). The matcher source is
BlueOak-1.0.0; see [third-party notices](../../THIRD-PARTY-NOTICES.md).

For fixed Unicode class predicates, `maestro-pattern` also uses
`regress =0.12.0` with defaults (`backend-pikevm`, `std`) and
`prohibit-unsafe` on all targets (MIT OR Apache-2.0). Its only normal transitive
dependency is the existing `memchr =2.8.3` (`alloc`, `std`; MIT selected).
No library type enters the pattern operation's public interface.
