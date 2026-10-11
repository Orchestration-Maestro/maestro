## Approved OAuth primitive libraries

`maestro-models` uses base64 `=0.23.1` (defaults off, `std`), sha2 `=0.11.0`
(defaults off) and getrandom `=0.4.3` (defaults; browser `wasm_js` retained).
All are MIT OR Apache-2.0. The hashing closure is digest `=0.11.3`
(`default`, `block-api`), block-buffer `=0.12.1`, crypto-common `=0.2.2`,
hybrid-array `=0.4.15` and typenum `=1.20.1` (`const-generics`), also MIT OR
Apache-2.0. No third-party type appears in the OAuth primitive interface.
