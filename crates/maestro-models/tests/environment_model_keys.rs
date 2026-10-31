#[cfg(not(target_arch = "wasm32"))]
use crate::environment_children as children;

#[cfg(not(target_arch = "wasm32"))]
use children::{Directory, TestResult, child, run, vars};

#[cfg(not(target_arch = "wasm32"))]
const PRECEDENCE: &str = "maestro_environment_keys_follow_provider_precedence";

#[cfg(not(target_arch = "wasm32"))]
const PROVIDERS: &[(&str, &[&str])] = &[
    ("github-copilot", &["COPILOT_GITHUB_TOKEN"]),
    ("anthropic", &["ANTHROPIC_OAUTH_TOKEN", "ANTHROPIC_API_KEY"]),
    ("openai", &["OPENAI_API_KEY"]),
    ("azure-openai-responses", &["AZURE_OPENAI_API_KEY"]),
    ("deepseek", &["DEEPSEEK_API_KEY"]),
    ("google", &["GEMINI_API_KEY"]),
    ("google-vertex", &["GOOGLE_CLOUD_API_KEY"]),
    ("groq", &["GROQ_API_KEY"]),
    ("cerebras", &["CEREBRAS_API_KEY"]),
    ("xai", &["XAI_API_KEY"]),
    ("openrouter", &["OPENROUTER_API_KEY"]),
    ("vercel-ai-gateway", &["AI_GATEWAY_API_KEY"]),
    ("zai", &["ZAI_API_KEY"]),
    ("mistral", &["MISTRAL_API_KEY"]),
    ("minimax", &["MINIMAX_API_KEY"]),
    ("minimax-cn", &["MINIMAX_CN_API_KEY"]),
    ("moonshotai", &["MOONSHOT_API_KEY"]),
    ("moonshotai-cn", &["MOONSHOT_API_KEY"]),
    ("huggingface", &["HF_TOKEN"]),
    ("fireworks", &["FIREWORKS_API_KEY"]),
    ("opencode", &["OPENCODE_API_KEY"]),
    ("opencode-go", &["OPENCODE_API_KEY"]),
    ("kimi-coding", &["KIMI_API_KEY"]),
    ("cloudflare-workers-ai", &["CLOUDFLARE_API_KEY"]),
    ("cloudflare-ai-gateway", &["CLOUDFLARE_API_KEY"]),
    ("xiaomi", &["XIAOMI_API_KEY"]),
    ("xiaomi-token-plan-cn", &["XIAOMI_TOKEN_PLAN_CN_API_KEY"]),
    ("xiaomi-token-plan-ams", &["XIAOMI_TOKEN_PLAN_AMS_API_KEY"]),
    ("xiaomi-token-plan-sgp", &["XIAOMI_TOKEN_PLAN_SGP_API_KEY"]),
];

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn maestro_environment_keys_follow_provider_precedence() -> TestResult {
    if child()? {
        return Ok(());
    }
    let directory = Directory::new()?;
    for (provider, keys) in PROVIDERS {
        assert_presence_combinations(&directory, provider, keys)?;
    }
    for provider in [
        "",
        "unknown",
        "OPENAI",
        " openai",
        "openai ",
        "toString",
        "constructor",
        "__proto__",
        "OPENAI_API_KEY",
    ] {
        run(
            maestro_test_process::TestProcess::current(PRECEDENCE)?
                .env_clear()
                .case("*"),
            &directory,
            &vars(
                &[("OPENAI_API_KEY", "unrelated"), (provider, "unrelated")]
                    .into_iter()
                    .filter(|(key, _)| !key.is_empty())
                    .collect::<Vec<_>>(),
            ),
            provider,
            (None, None),
        )?;
    }
    for value in [
        "0",
        " ",
        "\t",
        "\u{feff}",
        "\u{85}",
        "\u{2003}",
        "second-independent-value",
    ] {
        run(
            maestro_test_process::TestProcess::current(PRECEDENCE)?
                .env_clear()
                .case("*"),
            &directory,
            &vars(&[("OPENAI_API_KEY", value)]),
            "openai",
            (Some(vec!["OPENAI_API_KEY"]), Some(value)),
        )?;
    }
    Ok(())
}

#[cfg(not(target_arch = "wasm32"))]
const COPILOT: &str = "maestro_copilot_keys_ignore_generic_github_tokens";

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn maestro_copilot_keys_ignore_generic_github_tokens() -> TestResult {
    if child()? {
        return Ok(());
    }
    let directory = Directory::new()?;
    for generic in [
        &[("GH_TOKEN", "generic")][..],
        &[("GITHUB_TOKEN", "generic")],
        &[("GH_TOKEN", "generic"), ("GITHUB_TOKEN", "generic")],
    ] {
        run(
            maestro_test_process::TestProcess::current(COPILOT)?
                .env_clear()
                .case("*"),
            &directory,
            &vars(generic),
            "github-copilot",
            (None, None),
        )?;
    }
    run(
        maestro_test_process::TestProcess::current(COPILOT)?
            .env_clear()
            .case("*"),
        &directory,
        &vars(&[
            ("GH_TOKEN", "generic"),
            ("COPILOT_GITHUB_TOKEN", "dedicated"),
        ]),
        "github-copilot",
        (Some(vec!["COPILOT_GITHUB_TOKEN"]), Some("dedicated")),
    )?;
    Ok(())
}

#[cfg(not(target_arch = "wasm32"))]
fn assert_presence_combinations(
    directory: &Directory,
    provider: &str,
    keys: &[&str],
) -> TestResult {
    for mask in 0..(1 << keys.len()) {
        let present: Vec<&str> = keys
            .iter()
            .enumerate()
            .filter(|(index, _)| mask & (1 << index) != 0)
            .map(|(_, key)| *key)
            .collect();
        let variables: Vec<_> = keys
            .iter()
            .enumerate()
            .map(|(index, key)| {
                (
                    (*key).into(),
                    if mask & (1 << index) == 0 {
                        "".into()
                    } else {
                        format!("controlled-{key}").into()
                    },
                )
            })
            .collect();
        let first = present.first().map(|key| format!("controlled-{key}"));
        run(
            maestro_test_process::TestProcess::current(PRECEDENCE)?
                .env_clear()
                .case("*"),
            directory,
            &variables,
            provider,
            ((!present.is_empty()).then_some(present), first.as_deref()),
        )?;
    }
    run(
        maestro_test_process::TestProcess::current(PRECEDENCE)?
            .env_clear()
            .case("*"),
        directory,
        &[],
        provider,
        (None, None),
    )?;
    Ok(())
}

#[cfg(not(target_arch = "wasm32"))]
const VERTEX: &str = "maestro_vertex_keys_distinguish_explicit_and_ambient_credentials";

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn maestro_vertex_keys_distinguish_explicit_and_ambient_credentials() -> TestResult {
    if child()? {
        return Ok(());
    }
    let directory = Directory::new()?;
    let credential = directory.path().join("credentials");
    std::fs::write(&credential, b"deliberately not JSON")?;
    assert_vertex_requirements(&directory, &credential)?;
    assert_vertex_paths(&directory, &credential)?;
    assert_vertex_cache(&directory)?;
    #[cfg(unix)]
    assert_vertex_native_paths(&directory)?;
    Ok(())
}

#[cfg(not(target_arch = "wasm32"))]
fn vertex_variables(path: &std::path::Path) -> Vec<(std::ffi::OsString, std::ffi::OsString)> {
    let mut variables = vars(&[
        ("GOOGLE_CLOUD_PROJECT", "project"),
        ("GOOGLE_CLOUD_LOCATION", "location"),
    ]);
    variables.push((
        "GOOGLE_APPLICATION_CREDENTIALS".into(),
        path.as_os_str().to_owned(),
    ));
    variables
}

#[cfg(not(target_arch = "wasm32"))]
fn vertex_run(
    directory: &Directory,
    variables: &[(std::ffi::OsString, std::ffi::OsString)],
    expected: Option<&str>,
) -> TestResult {
    run(
        maestro_test_process::TestProcess::current(VERTEX)?
            .env_clear()
            .case("*"),
        directory,
        variables,
        "google-vertex",
        (None, expected),
    )?;
    Ok(())
}

#[cfg(not(target_arch = "wasm32"))]
fn assert_vertex_requirements(directory: &Directory, path: &std::path::Path) -> TestResult {
    for project in ["GOOGLE_CLOUD_PROJECT", "GCLOUD_PROJECT"] {
        let mut variables = vars(&[(project, "project"), ("GOOGLE_CLOUD_LOCATION", "location")]);
        variables.push((
            "GOOGLE_APPLICATION_CREDENTIALS".into(),
            path.as_os_str().to_owned(),
        ));
        vertex_run(directory, &variables, Some("<authenticated>"))?;
    }
    for key in ["GOOGLE_CLOUD_PROJECT", "GOOGLE_CLOUD_LOCATION"] {
        for absent in [true, false] {
            let mut variables = vertex_variables(path);
            variables.retain(|(name, _)| name != key);
            if !absent {
                variables.push((key.into(), "".into()));
            }
            vertex_run(directory, &variables, None)?;
        }
    }
    let mut variables = vertex_variables(path);
    variables.extend(vars(&[
        ("GOOGLE_CLOUD_PROJECT", ""),
        ("GCLOUD_PROJECT", "fallback"),
    ]));
    vertex_run(directory, &variables, Some("<authenticated>"))?;
    run(
        maestro_test_process::TestProcess::current(VERTEX)?
            .env_clear()
            .case("*"),
        directory,
        &vars(&[
            ("GOOGLE_CLOUD_API_KEY", "explicit"),
            ("GOOGLE_APPLICATION_CREDENTIALS", "missing"),
        ]),
        "google-vertex",
        (Some(vec!["GOOGLE_CLOUD_API_KEY"]), Some("explicit")),
    )?;
    Ok(())
}

#[cfg(not(target_arch = "wasm32"))]
#[cfg_attr(not(unix), allow(unused_variables))]
fn assert_vertex_paths(directory: &Directory, credential: &std::path::Path) -> TestResult {
    vertex_run(
        directory,
        &vertex_variables(std::path::Path::new("credentials")),
        Some("<authenticated>"),
    )?;
    vertex_run(
        directory,
        &vertex_variables(directory.path()),
        Some("<authenticated>"),
    )?;
    let default = directory
        .path()
        .join(".config/gcloud/application_default_credentials.json");
    std::fs::create_dir_all(default.parent().ok_or("missing parent")?)?;
    for exists in [false, true] {
        if exists {
            std::fs::write(&default, b"not JSON")?;
        }
        for explicit in [None, Some("")] {
            let mut variables = vars(&[
                ("GOOGLE_CLOUD_PROJECT", "project"),
                ("GOOGLE_CLOUD_LOCATION", "location"),
                ("GOOGLE_CLOUD_API_KEY", ""),
            ]);
            if let Some(explicit) = explicit {
                variables.extend(vars(&[("GOOGLE_APPLICATION_CREDENTIALS", explicit)]));
            }
            vertex_run(directory, &variables, exists.then_some("<authenticated>"))?;
        }
    }
    vertex_run(
        directory,
        &vertex_variables(&directory.path().join("missing")),
        None,
    )?;
    #[cfg(unix)]
    {
        let link = directory.path().join("link");
        std::os::unix::fs::symlink(credential, &link)?;
        vertex_run(directory, &vertex_variables(&link), Some("<authenticated>"))?;
        std::fs::remove_file(credential)?;
        vertex_run(directory, &vertex_variables(&link), None)?;
    }
    Ok(())
}

#[cfg(not(target_arch = "wasm32"))]
fn assert_vertex_cache(directory: &Directory) -> TestResult {
    let path = directory.path().join("cached");
    let mut variables = vertex_variables(&path);
    variables.push(("MAESTRO_ENV_CREATE".into(), path.as_os_str().to_owned()));
    vertex_run(directory, &variables, None)?;
    let mut variables = vertex_variables(&path);
    variables.push(("MAESTRO_ENV_REMOVE".into(), path.as_os_str().to_owned()));
    vertex_run(directory, &variables, Some("<authenticated>"))?;
    Ok(())
}

#[cfg(not(target_arch = "wasm32"))]
#[cfg(unix)]
fn assert_vertex_native_paths(directory: &Directory) -> TestResult {
    use std::os::unix::ffi::{OsStrExt, OsStringExt};
    let name = std::ffi::OsString::from_vec(b"native-\xff".to_vec());
    // A host may refuse names that are not valid UTF-8; every UTF-8 subcase stays
    // unconditional.
    if !directory.keeps_distinct(&[name.as_os_str()])? {
        eprintln!("host refuses the name bytes {:?}", name.as_bytes());
        return Ok(());
    }
    let home = directory.path().join(&name);
    let path = home.join(".config/gcloud/application_default_credentials.json");
    std::fs::create_dir_all(path.parent().ok_or("missing parent")?)?;
    std::fs::write(&path, b"not JSON")?;
    vertex_run(directory, &vertex_variables(&path), Some("<authenticated>"))?;
    let mut variables = vars(&[
        ("GOOGLE_CLOUD_PROJECT", "project"),
        ("GOOGLE_CLOUD_LOCATION", "location"),
    ]);
    variables.push(("HOME".into(), home.into_os_string()));
    vertex_run(directory, &variables, Some("<authenticated>"))?;
    Ok(())
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn maestro_bedrock_configuration_signals_remain_distinct() -> TestResult {
    if child()? {
        return Ok(());
    }
    let directory = Directory::new()?;
    let singles = [
        "AWS_PROFILE",
        "AWS_BEARER_TOKEN_BEDROCK",
        "AWS_CONTAINER_CREDENTIALS_RELATIVE_URI",
        "AWS_CONTAINER_CREDENTIALS_FULL_URI",
        "AWS_WEB_IDENTITY_TOKEN_FILE",
    ];
    bedrock_run(&directory, &[], None)?;
    for key in singles {
        bedrock_run(
            &directory,
            &vars(&[(key, "no-existing-file-required")]),
            Some("<authenticated>"),
        )?;
    }
    for key in singles.into_iter().chain([
        "AWS_ACCESS_KEY_ID",
        "AWS_SECRET_ACCESS_KEY",
        "AWS_SESSION_TOKEN",
    ]) {
        bedrock_run(&directory, &vars(&[(key, "")]), None)?;
    }
    for key in [
        "AWS_ACCESS_KEY_ID",
        "AWS_SECRET_ACCESS_KEY",
        "AWS_SESSION_TOKEN",
    ] {
        bedrock_run(&directory, &vars(&[(key, "alone")]), None)?;
    }
    for session in [None, Some(""), Some("session")] {
        let mut variables = vars(&[
            ("AWS_ACCESS_KEY_ID", "access"),
            ("AWS_SECRET_ACCESS_KEY", "secret"),
        ]);
        if let Some(session) = session {
            variables.extend(vars(&[("AWS_SESSION_TOKEN", session)]));
        }
        bedrock_run(&directory, &variables, Some("<authenticated>"))?;
    }
    for pair in [
        [
            ("AWS_ACCESS_KEY_ID", ""),
            ("AWS_SECRET_ACCESS_KEY", "secret"),
        ],
        [
            ("AWS_ACCESS_KEY_ID", "access"),
            ("AWS_SECRET_ACCESS_KEY", ""),
        ],
    ] {
        bedrock_run(&directory, &vars(&pair), None)?;
    }
    Ok(())
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn maestro_fireworks_catalog_and_keys_remain_complete() -> TestResult {
    use maestro_models::{ModelCost, ModelInput, get_model};
    if child()? {
        return Ok(());
    }
    let model = get_model("fireworks", "accounts/fireworks/models/kimi-k2p6")
        .ok_or("missing Kimi model")?;
    assert_eq!(model.id, "accounts/fireworks/models/kimi-k2p6");
    assert_eq!(model.api, "anthropic-messages");
    assert_eq!(model.provider, "fireworks");
    assert_eq!(model.base_url, "https://api.fireworks.ai/inference");
    assert!(model.reasoning);
    assert_eq!(model.input, [ModelInput::Text, ModelInput::Image]);
    assert_eq!(model.context_window.to_bits(), 262_000.0_f64.to_bits());
    assert_eq!(model.max_tokens.to_bits(), 262_000.0_f64.to_bits());
    assert_eq!(
        model.cost,
        ModelCost {
            input: 0.95,
            output: 4.0,
            cache_read: 0.16,
            cache_write: 0.0
        }
    );
    let router = get_model("fireworks", "accounts/fireworks/routers/kimi-k2p5-turbo")
        .ok_or("missing turbo router")?;
    assert_eq!(router.id, "accounts/fireworks/routers/kimi-k2p5-turbo");
    assert_eq!(router.api, "anthropic-messages");
    assert_eq!(router.base_url, "https://api.fireworks.ai/inference");
    assert_eq!(router.input, [ModelInput::Text, ModelInput::Image]);
    let directory = Directory::new()?;
    run(
        maestro_test_process::TestProcess::current(
            "maestro_fireworks_catalog_and_keys_remain_complete",
        )?
        .env_clear()
        .case("*"),
        &directory,
        &vars(&[("FIREWORKS_API_KEY", "test-fireworks-key")]),
        "fireworks",
        (Some(vec!["FIREWORKS_API_KEY"]), Some("test-fireworks-key")),
    )?;
    Ok(())
}

#[cfg(not(target_arch = "wasm32"))]
fn bedrock_run(
    directory: &Directory,
    variables: &[(std::ffi::OsString, std::ffi::OsString)],
    expected: Option<&str>,
) -> TestResult {
    run(
        maestro_test_process::TestProcess::current(
            "maestro_bedrock_configuration_signals_remain_distinct",
        )?
        .env_clear()
        .case("*"),
        directory,
        variables,
        "amazon-bedrock",
        (None, expected),
    )
}

#[cfg(not(target_arch = "wasm32"))]
/// Cleared children preserve native operands and caller profile precedence.
#[test]
fn cleared_environment_child_keeps_native_values_and_profile_order() -> TestResult {
    const NAME: &str = "cleared_environment_child_keeps_native_values_and_profile_order";
    const PROFILE: &str = "inherited-%p-%m.profraw";
    const OVERRIDE: &str = "explicit-%p-%m.profraw";
    if maestro_test_process::selection().is_child() {
        let stage = ["outer", "inherited", "override"]
            .into_iter()
            .find(|stage| maestro_test_process::selection().runs_case(stage))
            .expect("selected native stage");
        if stage != "outer" {
            return native_environment_observations(if stage == "inherited" {
                PROFILE
            } else {
                OVERRIDE
            });
        }
        assert_eq!(std::env::var("LLVM_PROFILE_FILE")?, PROFILE);
        let directory = Directory::new()?;
        let before: std::collections::BTreeMap<_, _> = std::env::vars_os().collect();
        for (stage, profile) in [("inherited", None), ("override", Some(OVERRIDE))] {
            let mut variables = vars(&[("MAESTRO_NATIVE_EMPTY", "")]);
            variables.extend(profile.map(|value| ("LLVM_PROFILE_FILE".into(), value.into())));
            #[cfg(unix)]
            {
                use std::os::unix::ffi::OsStringExt as _;
                variables.push((
                    "MAESTRO_NATIVE_BYTES".into(),
                    std::ffi::OsString::from_vec(vec![b'a', 0xff]),
                ));
            }
            run(
                maestro_test_process::TestProcess::current(NAME)?
                    .env_clear()
                    .case(stage),
                &directory,
                &variables,
                "unknown-native-provider",
                (None, None),
            )?;
        }
        let unchanged = std::env::vars_os().collect::<std::collections::BTreeMap<_, _>>() == before;
        assert!(unchanged, "child launches changed the outer environment");
        return Ok(());
    }
    let output = maestro_test_process::TestProcess::current(NAME)?
        .nocapture()
        .case("outer")
        .env("LLVM_PROFILE_FILE", PROFILE)
        .env("MAESTRO_NATIVE_SENTINEL", "ambient")
        .output()?;
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    Ok(())
}

#[cfg(not(target_arch = "wasm32"))]
/// Observe native operands in the actual cleared child before consuming its expectation.
fn native_environment_observations(profile: &str) -> TestResult {
    let cwd = std::env::current_dir()?;
    assert_eq!(std::env::var_os("HOME").as_deref(), Some(cwd.as_os_str()));
    assert_eq!(
        std::env::var_os("USERPROFILE").as_deref(),
        Some(cwd.as_os_str())
    );
    assert_eq!(std::env::var("LLVM_PROFILE_FILE")?, profile);
    assert!(std::env::var_os("MAESTRO_NATIVE_SENTINEL").is_none());
    assert_eq!(std::env::var_os("MAESTRO_NATIVE_EMPTY"), Some("".into()));
    #[cfg(unix)]
    {
        use std::os::unix::ffi::OsStrExt as _;
        assert_eq!(
            std::env::var_os("MAESTRO_NATIVE_BYTES")
                .ok_or("missing native bytes")?
                .as_bytes(),
            &[b'a', 0xff]
        );
    }
    assert_eq!(
        std::env::var("MAESTRO_ENV_EXPECTATION")?,
        "[\"unknown-native-provider\",null,null]"
    );
    assert!(child()?);
    Ok(())
}
