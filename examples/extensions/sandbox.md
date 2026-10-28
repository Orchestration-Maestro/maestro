# Sandboxed bash

Build this opt-in extension to restrict bash filesystem and network access. Other
tools and installed extensions are not sandboxed.

```sh
cargo build --locked -p maestro-extensions-wasm --example sandbox \
  --target wasm32-wasip2 --release
maestro -e /path/to/sandbox.wasm
```

Linux requires `bash`, `bwrap`, `socat` and `rg` on PATH, with user namespaces
permitted by the host. The default Unix-socket filter supports x86-64 and ARM64.
macOS requires `bash`, `rg` and `sandbox-exec`. Windows is unsupported. The
extension does not install system programs.

Use `--no-sandbox` to disable the wrapper. `/sandbox` displays configured values;
changes take effect at the next session start. Global settings live at
`<agent_dir>/extensions/sandbox.json`; `<cwd>/.maestro/sandbox.json` overrides
them. Permission objects merge shallowly; arrays replace earlier arrays.

```json
{
  "enabled": true,
  "network": {
    "allowedDomains": ["github.com", "*.github.com"],
    "deniedDomains": []
  },
  "filesystem": {
    "denyRead": ["~/.ssh", "~/.aws", "~/.gnupg"],
    "allowWrite": [".", "/tmp"],
    "denyWrite": [".env", ".env.*", "*.pem", "*.key"]
  }
}
```

Denied domains take precedence. Wildcards admit subdomains, not their base domain.
Both HTTP CONNECT and SOCKS5 use the same decision before name resolution.

## Explicit grants

- `network.allowUnixSockets` admits specific paths on macOS; Linux cannot apply
  path-specific socket grants and keeps them blocked unless all sockets are granted.
- `network.allowAllUnixSockets` allows host Unix sockets. This can bypass network
  restrictions through a host service; enable it only for trusted commands.
- `network.allowLocalBinding` permits macOS loopback listeners. Linux already has
  a private loopback network namespace.
- `network.httpProxyPort` and `socksProxyPort` select user-owned localhost proxies.
  The extension neither filters their traffic nor closes their listening ports.
- `filesystem.allowGitConfig` permits repository configuration; hooks remain
  protected. `enableWeakerNestedSandbox` retains the host process namespace on Linux.

`ignoreViolations` is accepted and has no effect yet. It is not an authorization
grant and never allows an otherwise denied operation.

## Boundaries and cleanup

Initialization failure reports an error and leaves native bash available **without
protection**. Do not treat the tool's label as proof that initialization succeeded.

Linux expands existing deny-glob matches before every command. A matching file
created within the same command is not blocked until the next command. Existing
protected paths and executable configuration files remain excluded from writes.

`TMPDIR` defaults to `/tmp/maestro`, or `MAESTRO_TMPDIR` when supplied. Choose a
dedicated directory: it must be owned by the current user, and existing permissions
are tightened to mode 0700. It is writable even with an empty `allowWrite` array.
Temporary artifacts are retained after shutdown. Native path observations reject
invalid UTF-8 rather than silently selecting a different filename.

Session shutdown closes owned proxy connections, stops bridge process groups and
removes only the extension's private control files. It does not delete the
persistent temporary directory or stop user-selected external proxies.

The controlled tests use disposable files and loopback peers, without provider
credentials or internet access:

```sh
cargo nextest run --locked -p maestro --test integration -E 'test(sandbox_example::)'
cargo test --locked -p maestro-extensions-wasm --lib sandbox_permissions_example
cargo test --locked -p maestro-extensions-wasmtime --test integration sandbox_command_
```
