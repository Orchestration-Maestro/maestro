use maestro_resources::*;
#[path = "support/resource_operations.rs"]
mod support;
use support::Controlled;
fn native_dir(name: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("maestro-path-{}-{name}", std::process::id()));
    std::fs::create_dir(&dir).unwrap();
    dir
}
#[cfg(unix)]
fn link(target: &std::path::Path, path: &std::path::Path) {
    std::os::unix::fs::symlink(target, path).unwrap();
}

#[test]
fn ordinary_file_gets_real_path() {
    let d = native_dir("file");
    let p = d.join("file");
    std::fs::write(&p, "text").unwrap();
    assert_eq!(
        canonicalize_path(p.to_str().unwrap(), &NativeResourceOperations),
        std::fs::canonicalize(&p).unwrap().to_str().unwrap()
    );
    std::fs::remove_dir_all(d).unwrap();
}

#[test]
fn file_link_gets_target_path() {
    #[cfg(unix)]
    {
        let d = native_dir("file-link");
        let p = d.join("file");
        std::fs::write(&p, "text").unwrap();
        let l = d.join("link");
        link(&p, &l);
        assert_eq!(
            canonicalize_path(l.to_str().unwrap(), &NativeResourceOperations),
            std::fs::canonicalize(&p).unwrap().to_str().unwrap()
        );
        std::fs::remove_dir_all(d).unwrap();
    }
}

#[test]
fn directory_link_gets_target_path() {
    #[cfg(unix)]
    {
        let d = native_dir("dir-link");
        let p = d.join("target");
        std::fs::create_dir(&p).unwrap();
        let l = d.join("link");
        link(&p, &l);
        assert_eq!(
            canonicalize_path(l.to_str().unwrap(), &NativeResourceOperations),
            std::fs::canonicalize(&p).unwrap().to_str().unwrap()
        );
        std::fs::remove_dir_all(d).unwrap();
    }
}

#[test]
fn missing_target_keeps_input_path() {
    assert_eq!(
        canonicalize_path("/missing/../target", &NativeResourceOperations),
        "/missing/../target"
    );
}

#[test]
fn dangling_link_keeps_input_path() {
    #[cfg(unix)]
    {
        let d = native_dir("dangling");
        let l = d.join("link");
        link(&d.join("absent"), &l);
        assert_eq!(
            canonicalize_path(l.to_str().unwrap(), &NativeResourceOperations),
            l.to_str().unwrap()
        );
        std::fs::remove_dir_all(d).unwrap();
    }
}

#[test]
fn bare_name_stays_local() {
    assert!(is_local_path("some-package"));
}

#[test]
fn relative_name_stays_local() {
    assert!(is_local_path("./foo"));
}

#[test]
fn npm_prefix_is_nonlocal() {
    assert!(!is_local_path("npm:package"));
}

#[test]
fn git_prefix_is_nonlocal() {
    assert!(!is_local_path("git://repo"));
}

#[test]
fn https_prefix_is_nonlocal() {
    assert!(!is_local_path("https://example.com"));
}

#[test]
fn local_prefixes_are_exact_and_trimmed() {
    for prefix in ["npm:", "git:", "https:", "http:", "github:", "ssh:"] {
        assert!(!is_local_path(&format!(" {prefix}foo ")));
        assert!(is_local_path(&prefix.to_uppercase()));
    }
    assert!(is_local_path(""));
    assert!(is_local_path("\u{feff}npm:x"));
    assert!(!is_local_path("\u{85}npm:x\u{85}"));
}

#[test]
fn explicit_paths_follow_platform_resolution() {
    let mut ops = Controlled::skill("---\ndescription: text\n---");
    let paths = [
        "~",
        "~/x",
        "~x",
        "",
        " ",
        ".",
        "..",
        "a//b",
        "/absolute//raw",
        "\u{feff}file",
        "\u{85}file",
    ]
    .map(str::to_owned);
    let r = load_skills(
        LoadSkillsOptions {
            cwd: "/work",
            agent_dir: "/agent",
            skill_paths: &paths,
            include_defaults: false,
            config_dir_name: ".maestro",
            home: "/home",
        },
        &ops,
    )
    .unwrap();
    #[cfg(unix)]
    assert_eq!(
        r.diagnostics
            .iter()
            .map(|d| d.path.as_deref().unwrap())
            .collect::<Vec<_>>(),
        vec![
            "/home",
            "/home/x",
            "/home/x",
            "/work/",
            "/work/",
            "/work",
            "/work/..",
            "/work/a/b",
            "/absolute//raw",
            "/work/\u{feff}file",
            "/work/file"
        ]
    );
    ops.cwd_error = true;
    let r = load_skills(
        LoadSkillsOptions {
            cwd: "relative",
            agent_dir: "agent",
            skill_paths: &[],
            include_defaults: false,
            config_dir_name: ".maestro",
            home: "/home",
        },
        &ops,
    );
    assert_eq!(r.unwrap_err().message.as_deref(), Some("cwd failed"));
}
