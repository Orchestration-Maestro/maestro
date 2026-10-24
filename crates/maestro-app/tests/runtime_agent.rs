//! Native runtime identification.
#![cfg(test)]

use maestro_app::configuration::identification::get_maestro_user_agent;
use serde::Deserialize;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Case {
    input: Input,
    expected: String,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Input {
    version: String,
    platform: String,
    arch: String,
}
#[test]
fn user_agent_reports_native_runtime() {
    let cases: Vec<Case> = serde_json::from_str(include_str!(
        "fixtures/configuration/user_agent_reports_native_runtime.json"
    ))
    .unwrap();
    assert_eq!(cases.len(), 12);
    for case in cases {
        let i = case.input;
        assert_eq!(
            get_maestro_user_agent(&i.version, &i.platform, &i.arch),
            case.expected
        );
    }
    for (platform, arch, expected) in [
        ("windows", "x86_64", "maestro/2 (win32; rust; x64)"),
        ("macos", "aarch64", "maestro/2 (darwin; rust; arm64)"),
        ("linux", "x86", "maestro/2 (linux; rust; ia32)"),
        ("freebsd", "riscv64", "maestro/2 (freebsd; rust; riscv64)"),
    ] {
        assert_eq!(get_maestro_user_agent("2", platform, arch), expected);
    }
}
