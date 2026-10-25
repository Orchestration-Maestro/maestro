## Approved subscription listener libraries

`maestro-models` uses native-only Hyper `=1.11.1` (defaults off, `server`,
`http1`), hyper-util `=0.1.21` (defaults off, `tokio`) and http-body-util
`=0.1.5` (defaults off). Native Tokio `=1.53.3` adds `net` to its existing
`rt-multi-thread`, `sync` and `time` features. All four libraries are MIT;
no server dependency is declared for the browser target and no third-party
type appears in the authorization interface.
