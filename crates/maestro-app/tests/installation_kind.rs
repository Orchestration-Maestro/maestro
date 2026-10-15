//! Installation evidence precedence without process execution.
#![cfg(test)]

use maestro_app::configuration::config::installation::{
    InstallMethod, Installation, PackageLayout, detect_install_method,
};
use serde::Deserialize;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Case {
    input: Input,
    expected: Method,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct Input {
    module_dir: Option<String>,
    exec_path: Option<String>,
    layout: Layout,
}
#[derive(Deserialize)]
#[serde(rename_all = "lowercase")]
enum Layout {
    Package,
    Standalone,
}
#[derive(Deserialize)]
#[serde(rename_all = "kebab-case")]
enum Method {
    BunBinary,
    Npm,
    Pnpm,
    Yarn,
    Bun,
    Unknown,
}
#[test]
fn installation_detection_orders_package_evidence() {
    let cases: Vec<Case> = serde_json::from_str(include_str!(
        "fixtures/configuration/installation_detection_orders_package_evidence.json"
    ))
    .unwrap();
    assert_eq!(cases.len(), 14);
    for case in cases {
        let installation = Installation {
            module_dir: case
                .input
                .module_dir
                .unwrap_or_else(|| "/package/src".to_owned()),
            executable: case
                .input
                .exec_path
                .unwrap_or_else(|| "/usr/bin/node".to_owned()),
            layout: match case.input.layout {
                Layout::Package => PackageLayout::Package,
                Layout::Standalone => PackageLayout::Standalone,
            },
        };
        let expected = match case.expected {
            Method::BunBinary => InstallMethod::BunBinary,
            Method::Npm => InstallMethod::Npm,
            Method::Pnpm => InstallMethod::Pnpm,
            Method::Yarn => InstallMethod::Yarn,
            Method::Bun => InstallMethod::Bun,
            Method::Unknown => InstallMethod::Unknown,
        };
        assert_eq!(detect_install_method(&installation), expected);
    }
}
