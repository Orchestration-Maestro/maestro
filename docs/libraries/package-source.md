## Approved package-source libraries

`maestro-packages` uses `git-url-parse =0.6.0` with defaults (`url`, MIT),
`url =2.5.8` and `percent-encoding =2.3.2` with defaults (MIT OR Apache-2.0),
plus inherited workspace serde_json for raw scoped settings and `raw_value`
for deferred package-manifest field selection. This feature adds no package. The selected parser
adds `getset =0.1.7` and `nom =8.0.0` (MIT); it reuses the existing URL/IDNA
closure and the `unicode-ident =1.0.24` hold. Outside browser targets it also uses
Tokio `=1.53.2` (MIT; defaults off, `rt`, `process`, `io-util`, `time` and `macros`) to wait for package
commands and capture their streams. All targets enable `sync` for worker results.
Unix adds rustix `=1.1.5`, defaults off, `std` and `process`, for ordinary
termination. Native tests enable Tokio `test-util`. The native constructor takes
a caller-owned `Rc<tokio::task::LocalSet>` and retains it weakly. Serde derive is test-only
here, as is Tokio's runtime builder. Internal dependencies are settings, resources
and the shared path utility. Native effects use the standard library and Tokio and
remain outside the browser target; browser callers supply `PackageOperations`.

Deterministic temporary cache names use `sha2 =0.11.0`, defaults off
(MIT OR Apache-2.0). This direct edge reuses the locked SHA-256 dependency;
it adds no resolved package or feature.
