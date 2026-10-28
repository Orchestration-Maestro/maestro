//! Native macOS authorization profile serialization.
use super::config_data::SandboxConfig;
use serde_json::Value;
use std::path::Path;

/// Serializes resolved permissions and the selected local proxy endpoints.
pub(crate) fn profile(
    config: &SandboxConfig,
    ports: [u16; 2],
    temporary: &str,
    cwd: &str,
    tag: &str,
) -> String {
    let mut rules = vec![
        "(version 1)".into(),
        format!("(deny default (with message {}))", quoted(tag)),
        BASE.into(),
    ];
    network(&mut rules, &config.network, ports);
    rules.push("(allow file-read*)".into());
    for path in &config.filesystem.deny_read {
        rule(&mut rules, "deny", "file-read*", path, tag);
        moves(&mut rules, path, tag);
    }
    for path in config
        .filesystem
        .allow_write
        .iter()
        .map(String::as_str)
        .chain([temporary])
    {
        rule(&mut rules, "allow", "file-write*", path, tag);
    }
    let mut denied = config.filesystem.deny_write.clone();
    for path in protected(config.filesystem.allow_git_config) {
        denied.push(format!("{cwd}/{path}"));
        denied.push(format!("**/{path}"));
        denied.push(format!("**/{path}/**"));
    }
    for path in denied {
        rule(&mut rules, "deny", "file-write*", &path, tag);
        moves(&mut rules, &path, tag);
    }
    rules.join("\n")
}

/// Retains explicit Unix and loopback grants without opening other internet destinations.
fn network(rules: &mut Vec<String>, config: &super::network::NetworkConfig, ports: [u16; 2]) {
    if config.allow_local_binding {
        for operation in ["network-bind", "network-inbound", "network-outbound"] {
            rules.push(format!("(allow {operation} (local ip \"localhost:*\"))"));
        }
    }
    if config.allow_all_unix_sockets {
        rules.push("(allow network* (subpath \"/\"))".into());
    } else {
        for path in &config.allow_unix_sockets {
            rules.push(format!("(allow network* (subpath {}))", quoted(path)));
        }
    }
    if config.allowed_domains.is_empty() {
        return;
    }
    for port in ports {
        for operation in ["network-bind", "network-inbound"] {
            rules.push(format!(
                "(allow {operation} (local ip \"localhost:{port}\"))"
            ));
        }
        rules.push(format!(
            "(allow network-outbound (remote ip \"localhost:{port}\"))"
        ));
    }
}

/// Immutable executable-configuration exclusions, with repository config independently granted.
fn protected(allow_git_config: bool) -> Vec<&'static str> {
    super::config_data::PROTECTED
        .iter()
        .copied()
        .filter(|path| !allow_git_config || *path != ".git/config")
        .collect()
}

/// Uses JSON string serialization for the profile language's quoted operands.
fn quoted(value: &str) -> String {
    Value::String(value.into()).to_string()
}

/// Distinguishes native subpaths from anchored glob expressions.
fn filter(path: &str) -> String {
    if path.contains(['*', '?', '[', ']']) {
        format!("(regex {})", quoted(&glob_regex(path)))
    } else {
        format!("(subpath {})", quoted(path))
    }
}

/// Adds one authorization statement with its diagnostic identity.
fn rule(rules: &mut Vec<String>, action: &str, operation: &str, path: &str, tag: &str) {
    rules.push(format!(
        "({action} {operation} {} (with message {}))",
        filter(path),
        quoted(tag)
    ));
}

/// Prevents rename-based bypass through a protected path's static ancestors.
fn moves(rules: &mut Vec<String>, path: &str, tag: &str) {
    rule(rules, "deny", "file-write-unlink", path, tag);
    let prefix = path.split(['*', '?', '[', ']']).next().unwrap_or_default();
    let glob = prefix.len() != path.len();
    let mut ancestor = if glob && prefix.ends_with('/') {
        Some(Path::new(prefix.trim_end_matches('/')))
    } else {
        Path::new(prefix).parent()
    };
    while let Some(path) = ancestor {
        let Some(path) = path
            .to_str()
            .filter(|path| !path.is_empty() && *path != "/")
        else {
            break;
        };
        rules.push(format!(
            "(deny file-write-unlink (literal {}) (with message {}))",
            quoted(path),
            quoted(tag)
        ));
        ancestor = Path::new(path).parent();
    }
}

/// Converts path globs into the native profile's anchored regular expressions.
fn glob_regex(pattern: &str) -> String {
    let mut output = String::from("^");
    let mut chars = pattern.chars().peekable();
    while let Some(character) = chars.next() {
        match character {
            '*' if chars.peek() == Some(&'*') => {
                chars.next();
                if chars.peek() == Some(&'/') {
                    chars.next();
                    output.push_str("(.*/)?");
                } else {
                    output.push_str(".*");
                }
            }
            '*' => output.push_str("[^/]*"),
            '?' => output.push_str("[^/]"),
            '[' if chars.clone().any(|character| character == ']') => {
                output.push('[');
                output.extend(chars.by_ref().take_while(|character| *character != ']'));
                output.push(']');
            }
            character => {
                if ".^$+{}()|\\[]".contains(character) {
                    output.push('\\');
                }
                output.push(character);
            }
        }
    }
    output.push('$');
    output
}

/// Essential native process, system service and device permissions.
const BASE: &str = r#"(allow process-exec)
(allow process-fork)
(allow process-info* (target same-sandbox))
(allow signal (target same-sandbox))
(allow mach-priv-task-port (target same-sandbox))
(allow user-preference-read)
(allow mach-lookup
  (global-name "com.apple.audio.systemsoundserver")
  (global-name "com.apple.distributed_notifications@Uv3")
  (global-name "com.apple.FontObjectsServer")
  (global-name "com.apple.fonts")
  (global-name "com.apple.logd")
  (global-name "com.apple.lsd.mapdb")
  (global-name "com.apple.PowerManagement.control")
  (global-name "com.apple.system.logger")
  (global-name "com.apple.system.notification_center")
  (global-name "com.apple.trustd.agent")
  (global-name "com.apple.system.opendirectoryd.libinfo")
  (global-name "com.apple.system.opendirectoryd.membership")
  (global-name "com.apple.bsd.dirhelper")
  (global-name "com.apple.securityd.xpc")
  (global-name "com.apple.coreservices.launchservicesd")
)
(allow ipc-posix-shm)
(allow ipc-posix-sem)
(allow iokit-open
  (iokit-registry-entry-class "IOSurfaceRootUserClient")
  (iokit-registry-entry-class "RootDomainUserClient")
  (iokit-user-client-class "IOSurfaceSendRight")
)
(allow iokit-get-properties)
(allow system-socket (require-all (socket-domain AF_SYSTEM) (socket-protocol 2)))
(allow sysctl-read
  (sysctl-name "hw.activecpu")
  (sysctl-name "hw.busfrequency_compat")
  (sysctl-name "hw.byteorder")
  (sysctl-name "hw.cacheconfig")
  (sysctl-name "hw.cachelinesize_compat")
  (sysctl-name "hw.cpufamily")
  (sysctl-name "hw.cpufrequency")
  (sysctl-name "hw.cpufrequency_compat")
  (sysctl-name "hw.cputype")
  (sysctl-name "hw.l1dcachesize_compat")
  (sysctl-name "hw.l1icachesize_compat")
  (sysctl-name "hw.l2cachesize_compat")
  (sysctl-name "hw.l3cachesize_compat")
  (sysctl-name "hw.logicalcpu")
  (sysctl-name "hw.logicalcpu_max")
  (sysctl-name "hw.machine")
  (sysctl-name "hw.memsize")
  (sysctl-name "hw.ncpu")
  (sysctl-name "hw.nperflevels")
  (sysctl-name "hw.packages")
  (sysctl-name "hw.pagesize_compat")
  (sysctl-name "hw.pagesize")
  (sysctl-name "hw.physicalcpu")
  (sysctl-name "hw.physicalcpu_max")
  (sysctl-name "hw.tbfrequency_compat")
  (sysctl-name "hw.vectorunit")
  (sysctl-name "kern.argmax")
  (sysctl-name "kern.bootargs")
  (sysctl-name "kern.hostname")
  (sysctl-name "kern.maxfiles")
  (sysctl-name "kern.maxfilesperproc")
  (sysctl-name "kern.maxproc")
  (sysctl-name "kern.ngroups")
  (sysctl-name "kern.osproductversion")
  (sysctl-name "kern.osrelease")
  (sysctl-name "kern.ostype")
  (sysctl-name "kern.osvariant_status")
  (sysctl-name "kern.osversion")
  (sysctl-name "kern.secure_kernel")
  (sysctl-name "kern.tcsm_available")
  (sysctl-name "kern.tcsm_enable")
  (sysctl-name "kern.usrstack64")
  (sysctl-name "kern.version")
  (sysctl-name "kern.willshutdown")
  (sysctl-name "machdep.cpu.brand_string")
  (sysctl-name "machdep.ptrauth_enabled")
  (sysctl-name "security.mac.lockdown_mode_state")
  (sysctl-name "sysctl.proc_cputype")
  (sysctl-name "vm.loadavg")
  (sysctl-name-prefix "hw.optional.arm")
  (sysctl-name-prefix "hw.optional.arm.")
  (sysctl-name-prefix "hw.optional.armv8_")
  (sysctl-name-prefix "hw.perflevel")
  (sysctl-name-prefix "kern.proc.all")
  (sysctl-name-prefix "kern.proc.pgrp.")
  (sysctl-name-prefix "kern.proc.pid.")
  (sysctl-name-prefix "machdep.cpu.")
  (sysctl-name-prefix "net.routetable.")
)
(allow sysctl-write
  (sysctl-name "kern.tcsm_enable")
)
(allow distributed-notification-post)
(allow mach-lookup (global-name "com.apple.SecurityServer"))
(allow file-ioctl (literal "/dev/null"))
(allow file-ioctl (literal "/dev/zero"))
(allow file-ioctl (literal "/dev/random"))
(allow file-ioctl (literal "/dev/urandom"))
(allow file-ioctl (literal "/dev/dtracehelper"))
(allow file-ioctl (literal "/dev/tty"))
(allow file-ioctl file-read-data file-write-data
  (require-all
    (literal "/dev/null")
    (vnode-type CHARACTER-DEVICE)
  )
)
"#;
