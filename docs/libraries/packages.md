# Package discovery libraries

`maestro-packages` uses `ignore =0.4.23` with default features disabled for
conventional resource-directory ignore rules. It reuses the existing lockfile
closure: aho-corasick 1.1.5, bstr 1.13.1, crossbeam-deque 0.8.8, crossbeam-epoch 0.9.21,
crossbeam-utils 0.8.23, globset 0.4.20, ignore 0.4.23, log 0.4.34, memchr 2.8.3,
proc-macro2 1.0.107, quote 1.0.47, regex-automata 0.4.18, regex-syntax 0.8.11, same-file
1.0.6, serde_core 1.0.229, serde_derive 1.0.229, syn 3.0.6, unicode-ident 1.0.24, walkdir
2.5.0, winapi-util 0.1.11, windows-link 0.2.1, windows-sys 0.61.2.
The closure uses MIT, Apache-2.0 or Unicode-3.0 license branches.

Source parsing and process dependencies are documented in
[package-source libraries](package-source.md); matcher dependencies in
[file-pattern libraries](package-pattern.md).
