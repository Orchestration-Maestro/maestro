#[path = "support/environment_children.rs"]
mod children;

use children::{Directory, TestResult, child, run, vars};

const PRECEDENCE: &str = "maestro_environment_keys_follow_provider_precedence";

const PROVIDERS: &[(&str, &[&str])] = &[
    (
        "github-copilot",
        &["COPILOT_GITHUB_TOKEN", "GH_TOKEN", "GITHUB_TOKEN"],
    ),
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
            PRECEDENCE,
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
            PRECEDENCE,
            &directory,
            &vars(&[("OPENAI_API_KEY", value)]),
            "openai",
            (Some(vec!["OPENAI_API_KEY"]), Some(value)),
        )?;
    }
    Ok(())
}

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
            PRECEDENCE,
            directory,
            &variables,
            provider,
            ((!present.is_empty()).then_some(present), first.as_deref()),
        )?;
    }
    run(PRECEDENCE, directory, &[], provider, (None, None))?;
    Ok(())
}

const VERTEX: &str = "maestro_vertex_keys_distinguish_explicit_and_ambient_credentials";

#[test]
fn maestro_vertex_keys_distinguish_explicit_and_ambient_credentials() -> TestResult {
    if child()? {
        return Ok(());
    }
    let directory = Directory::new()?;
    let credential = directory.0.join("credentials");
    std::fs::write(&credential, b"deliberately not JSON")?;
    assert_vertex_requirements(&directory, &credential)?;
    assert_vertex_paths(&directory, &credential)?;
    assert_vertex_cache(&directory)?;
    #[cfg(unix)]
    assert_vertex_native_paths(&directory)?;
    Ok(())
}

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

fn vertex_run(
    directory: &Directory,
    variables: &[(std::ffi::OsString, std::ffi::OsString)],
    expected: Option<&str>,
) -> TestResult {
    run(
        VERTEX,
        directory,
        variables,
        "google-vertex",
        (None, expected),
    )?;
    Ok(())
}

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
        VERTEX,
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

fn assert_vertex_paths(directory: &Directory, credential: &std::path::Path) -> TestResult {
    vertex_run(
        directory,
        &vertex_variables(std::path::Path::new("credentials")),
        Some("<authenticated>"),
    )?;
    vertex_run(
        directory,
        &vertex_variables(&directory.0),
        Some("<authenticated>"),
    )?;
    let default = directory
        .0
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
        &vertex_variables(&directory.0.join("missing")),
        None,
    )?;
    #[cfg(unix)]
    {
        let link = directory.0.join("link");
        std::os::unix::fs::symlink(credential, &link)?;
        vertex_run(directory, &vertex_variables(&link), Some("<authenticated>"))?;
        std::fs::remove_file(credential)?;
        vertex_run(directory, &vertex_variables(&link), None)?;
    }
    Ok(())
}

fn assert_vertex_cache(directory: &Directory) -> TestResult {
    let path = directory.0.join("cached");
    let mut variables = vertex_variables(&path);
    variables.push(("MAESTRO_ENV_CREATE".into(), path.as_os_str().to_owned()));
    vertex_run(directory, &variables, None)?;
    let mut variables = vertex_variables(&path);
    variables.push(("MAESTRO_ENV_REMOVE".into(), path.as_os_str().to_owned()));
    vertex_run(directory, &variables, Some("<authenticated>"))?;
    Ok(())
}

#[cfg(unix)]
fn assert_vertex_native_paths(directory: &Directory) -> TestResult {
    use std::os::unix::ffi::OsStringExt;
    let home = directory
        .0
        .join(std::ffi::OsString::from_vec(b"native-\xff".to_vec()));
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
        "maestro_fireworks_catalog_and_keys_remain_complete",
        &directory,
        &vars(&[("FIREWORKS_API_KEY", "test-fireworks-key")]),
        "fireworks",
        (Some(vec!["FIREWORKS_API_KEY"]), Some("test-fireworks-key")),
    )?;
    Ok(())
}

fn bedrock_run(
    directory: &Directory,
    variables: &[(std::ffi::OsString, std::ffi::OsString)],
    expected: Option<&str>,
) -> TestResult {
    run(
        "maestro_bedrock_configuration_signals_remain_distinct",
        directory,
        variables,
        "amazon-bedrock",
        (None, expected),
    )
}
