//! Linux filesystem mounts and trusted network bridges for the native bash owner.
use super::{
    config_data::PROTECTED,
    platform,
    platform_commands::{GLOB, decode_matches},
    runtime::PlatformContext,
};
use maestro_extensions_wasm::{
    ExtensionAPI,
    tools::path_utils::{expand_path, guest_path, resolve},
};
use maestro_text::shell_quote;
use std::{fs::Metadata, path::Path};

/// Waits for the trusted namespace listeners, using the existing command's cancellation/deadline.
const READY: &str = r#"while :; do
    ready=0
    while read -r slot local_address remote_address state rest; do
        if [[ $state == 0A && ( $local_address == 0100007F:0C38 || $local_address == 0100007F:0438 ) ]]; then
            ((ready+=1))
        fi
    done < /proc/net/tcp
    ((ready == 2)) && break
    kill -0 "$http_bridge" "$socks_bridge" 2>/dev/null || exit 1
    sleep 0.01
done"#;

impl PlatformContext {
    /// Expands current deny matches before every execution, then builds both namespace stages.
    pub(crate) async fn wrap_linux(
        &self,
        command: &str,
        temporary: &str,
        authored_temporary: &str,
    ) -> Result<String, String> {
        let filesystem = self.filesystem(temporary).await?;
        let inner = self.inner(&filesystem, command);
        let script = self.bridge_script(&inner);
        let mut outer: Vec<String> = [
            "bwrap",
            "--new-session",
            "--die-with-parent",
            "--unshare-net",
        ]
        .map(str::to_owned)
        .into();
        outer.extend(filesystem);
        outer.extend(["--dev", "/dev"].map(str::to_owned));
        if self.weaker {
            // The trusted inner launcher needs writable UID maps before restoring read-only mounts.
            outer.extend(["--bind", "/proc", "/proc"].map(str::to_owned));
        } else {
            outer.extend(["--unshare-pid", "--proc", "/proc"].map(str::to_owned));
        }
        for (key, value) in self.environment(authored_temporary, [3128, 1080]) {
            outer.extend(["--setenv".into(), key, value]);
        }
        outer.extend(["--".into(), self.shell.clone(), "-c".into(), script]);
        Ok(argv(&outer))
    }

    /// Builds the inner namespace that applies the filter after trusted bridge startup.
    fn inner(&self, filesystem: &[String], command: &str) -> Vec<String> {
        let mut inner: Vec<String> = ["bwrap", "--die-with-parent"].map(str::to_owned).into();
        inner.extend(filesystem.iter().cloned());
        inner.extend(["--dev", "/dev"].map(str::to_owned));
        if !self.weaker {
            inner.extend(["--unshare-pid", "--proc", "/proc"].map(str::to_owned));
        }
        if !self.network.allow_all_unix_sockets {
            inner.extend(["--seccomp", "3"].map(str::to_owned));
        }
        inner.extend(["--", &self.shell, "-c", command].map(str::to_owned));
        inner
    }

    /// Starts trusted listeners and waits for their bound sockets before executing user input.
    fn bridge_script(&self, inner: &[String]) -> String {
        let http = format!("{}/http.sock", self.control_directory);
        let socks = format!("{}/socks.sock", self.control_directory);
        let first = argv(&[
            "socat".into(),
            "TCP4-LISTEN:3128,bind=127.0.0.1,fork,reuseaddr".into(),
            format!("UNIX-CONNECT:{http}"),
        ]);
        let second = argv(&[
            "socat".into(),
            "TCP4-LISTEN:1080,bind=127.0.0.1,fork,reuseaddr".into(),
            format!("UNIX-CONNECT:{socks}"),
        ]);
        let filter = if self.network.allow_all_unix_sockets {
            String::new()
        } else {
            format!(
                "exec 3<{}\n",
                shell_quote(&format!("{}/unix-block.bpf", self.control_directory))
            )
        };
        format!(
            "{first} &\nhttp_bridge=$!\n{second} &\nsocks_bridge=$!\n{READY}\n{filter}exec {}",
            argv(inner)
        )
    }

    /// Starts read-only, admits write paths, and applies exact and expanded exclusions.
    async fn filesystem(&self, temporary: &str) -> Result<Vec<String>, String> {
        let mut args: Vec<String> = ["--ro-bind", "/", "/"].map(str::to_owned).into();
        let mut allowed = Vec::new();
        for path in self
            .filesystem
            .allow_write
            .iter()
            .map(String::as_str)
            .chain([temporary])
        {
            let path = self.normalize(path)?;
            if metadata(&self.api, &path)?.is_some() {
                args.extend(["--bind", &path, &path].map(str::to_owned));
                allowed.push(path);
            }
        }
        let mut denied = self.matches(&self.filesystem.deny_write).await?;
        denied.extend(self.mandatory().await?);
        for path in denied {
            self.deny_write(&mut args, &allowed, &path, temporary)?;
        }
        for path in self.matches(&self.filesystem.deny_read).await? {
            if Path::new(temporary).starts_with(&path) {
                return Err("Sandbox temporary directory conflicts with denyRead; choose another MAESTRO_TMPDIR".into());
            }
            let Some(metadata) = metadata(&self.api, &path)? else {
                continue;
            };
            if metadata.is_dir() {
                args.extend(["--tmpfs".into(), path]);
            } else {
                args.extend(
                    [
                        "--ro-bind",
                        &format!("{}/empty", self.control_directory),
                        &path,
                    ]
                    .map(str::to_owned),
                );
            }
        }
        args.extend(
            [
                "--ro-bind",
                &self.control_directory,
                &self.control_directory,
            ]
            .map(str::to_owned),
        );
        Ok(args)
    }

    /// Protects existing files and the first missing component within an admitted write path.
    fn deny_write(
        &self,
        args: &mut Vec<String>,
        allowed: &[String],
        path: &str,
        temporary: &str,
    ) -> Result<(), String> {
        if !allowed.iter().any(|root| Path::new(path).starts_with(root)) {
            return Ok(());
        }
        if Path::new(temporary).starts_with(path) {
            return Err("Sandbox temporary directory conflicts with denyWrite; choose another MAESTRO_TMPDIR".into());
        }
        if metadata(&self.api, path)?.is_some() {
            args.extend(["--ro-bind", path, path].map(str::to_owned));
            return Ok(());
        }
        let mut missing = path.to_owned();
        while let Some(parent) = Path::new(&missing)
            .parent()
            .and_then(Path::to_str)
            .map(str::to_owned)
        {
            match metadata(&self.api, &parent)? {
                Some(found) if !found.is_dir() => {
                    args.extend(["--ro-bind", &parent, &parent].map(str::to_owned));
                    return Ok(());
                }
                Some(_) => break,
                None => missing = parent,
            }
        }
        args.extend(
            [
                "--ro-bind",
                &format!("{}/empty", self.control_directory),
                &missing,
            ]
            .map(str::to_owned),
        );
        Ok(())
    }

    /// Resolves a supplied path with the existing native path owner.
    pub(crate) fn normalize(&self, path: &str) -> Result<String, String> {
        resolve(&self.api, &[&self.cwd, &expand_path(&self.api, path)?])
    }

    /// Uses bash's existing glob expansion, never shell evaluation of an authored pattern.
    async fn matches(&self, patterns: &[String]) -> Result<Vec<String>, String> {
        let mut matches = Vec::new();
        for pattern in patterns {
            let path = self.normalize(pattern)?;
            if !path.contains(['*', '?', '[', ']']) {
                matches.push(path);
                continue;
            }
            let output = platform::bytes(
                &self.api,
                &self.shell,
                &["-c".into(), GLOB.into(), "--".into(), path],
                &[0],
            )
            .await?;
            matches.extend(decode_matches(&output)?);
        }
        Ok(matches)
    }

    /// Reuses ripgrep for nested executable configuration files, retaining its three-level scope.
    async fn mandatory(&self) -> Result<Vec<String>, String> {
        let protected: Vec<_> = PROTECTED
            .iter()
            .copied()
            .filter(|path| !self.filesystem.allow_git_config || *path != ".git/config")
            .collect();
        let mut paths = protected
            .iter()
            .map(|path| self.normalize(path))
            .collect::<Result<Vec<_>, _>>()?;
        let mut args: Vec<String> = [
            "--files",
            "--hidden",
            "--no-ignore",
            "--null",
            "--max-depth",
            "3",
        ]
        .map(str::to_owned)
        .into();
        for pattern in protected {
            args.extend([
                "--iglob".into(),
                format!("**/{pattern}"),
                "--iglob".into(),
                format!("**/{pattern}/**"),
            ]);
        }
        args.push(self.cwd.clone());
        let output = platform::bytes(&self.api, "rg", &args, &[0, 1]).await?;
        for path in decode_matches(&output)? {
            paths.push(self.normalize(&path)?);
        }
        Ok(paths)
    }

    /// Supplies the proxy settings recognized by package managers and command-line clients.
    pub(crate) fn environment(&self, temporary: &str, ports: [u16; 2]) -> Vec<(String, String)> {
        let mut env = vec![
            ("SANDBOX_RUNTIME".into(), "1".into()),
            ("TMPDIR".into(), temporary.into()),
        ];
        let no_proxy = "localhost,127.0.0.1,::1,*.local,.local,169.254.0.0/16,10.0.0.0/8,172.16.0.0/12,192.168.0.0/16";
        for key in ["NO_PROXY", "no_proxy"] {
            env.push((key.into(), no_proxy.into()));
        }
        for key in [
            "HTTP_PROXY",
            "HTTPS_PROXY",
            "http_proxy",
            "https_proxy",
            "DOCKER_HTTP_PROXY",
            "DOCKER_HTTPS_PROXY",
        ] {
            env.push((key.into(), format!("http://localhost:{}", ports[0])));
        }
        for key in [
            "ALL_PROXY",
            "all_proxy",
            "FTP_PROXY",
            "ftp_proxy",
            "GRPC_PROXY",
            "grpc_proxy",
        ] {
            env.push((key.into(), format!("socks5h://localhost:{}", ports[1])));
        }
        for (key, value) in [
            ("RSYNC_PROXY", format!("localhost:{}", ports[1])),
            ("CLOUDSDK_PROXY_TYPE", "https".into()),
            ("CLOUDSDK_PROXY_ADDRESS", "localhost".into()),
            ("CLOUDSDK_PROXY_PORT", ports[0].to_string()),
        ] {
            env.push((key.into(), value));
        }
        env.push((
            "MAESTRO_SANDBOX_HOST_HTTP_PROXY_PORT".into(),
            self.ports[0].to_string(),
        ));
        env.push((
            "MAESTRO_SANDBOX_HOST_SOCKS_PROXY_PORT".into(),
            self.ports[1].to_string(),
        ));
        env
    }
}

/// Serializes literal program arguments through the shared text owner's POSIX quoter.
pub(crate) fn argv(arguments: &[String]) -> String {
    arguments
        .iter()
        .map(|value| shell_quote(value))
        .collect::<Vec<_>>()
        .join(" ")
}

/// Reports missing paths separately from permission or host filesystem errors.
fn metadata(api: &ExtensionAPI, path: &str) -> Result<Option<Metadata>, String> {
    match std::fs::metadata(guest_path(api, path)?) {
        Ok(metadata) => Ok(Some(metadata)),
        Err(error)
            if matches!(
                error.kind(),
                std::io::ErrorKind::NotFound | std::io::ErrorKind::NotADirectory
            ) =>
        {
            Ok(None)
        }
        Err(error) => Err(format!("Cannot inspect sandbox path {path}: {error}")),
    }
}
