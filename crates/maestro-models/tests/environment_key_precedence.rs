use credential_assertions::assert_credential_eq;
use maestro_models::{find_env_keys, get_env_api_key};
#[path = "support/credential_assertions.rs"]
mod credential_assertions;
#[path = "support/environment_child.rs"]
mod environment_child;
use environment_child::{Fixture, child};
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
fn provider_table_reports_each_configured_key() {
    if child() {
        let provider = std::env::var("TEST_PROVIDER").unwrap();
        let key = std::env::var("TEST_KEY").unwrap();
        let value = std::env::var("TEST_VALUE").unwrap();
        let expected = (!value.is_empty()).then_some(value);
        assert_eq!(
            find_env_keys(&provider).unwrap(),
            expected.as_ref().map(|_| vec![key])
        );
        assert_credential_eq(
            &(get_env_api_key(&provider).unwrap()),
            &(expected),
            "provider_table_reports_each_configured_key",
        );
        assert_credential_eq(
            &(maestro_models::records::stream::get_env_api_key(&provider).unwrap()),
            &(expected),
            "provider_table_reports_each_configured_key",
        );
        return;
    }
    let fixture = Fixture::new();
    for &(provider, keys) in PROVIDERS {
        for &key in keys {
            for value in ["", "controlled-key"] {
                fixture.run(
                    "provider_table_reports_each_configured_key",
                    &[
                        ("TEST_PROVIDER", provider),
                        ("TEST_KEY", key),
                        ("TEST_VALUE", value),
                        (key, value),
                        ("UNRELATED", ""),
                    ],
                );
            }
            fixture.run(
                "provider_table_reports_each_configured_key",
                &[
                    ("TEST_PROVIDER", provider),
                    ("TEST_KEY", key),
                    ("TEST_VALUE", ""),
                    ("UNRELATED", ""),
                ],
            );
        }
    }
}
#[test]
fn account_tokens_keep_declared_precedence() {
    if child() {
        let provider = std::env::var("TEST_PROVIDER").unwrap();
        let keys = PROVIDERS.iter().find(|(p, _)| *p == provider).unwrap().1;
        let expected: Vec<String> = keys
            .iter()
            .filter(|k| std::env::var(k).is_ok_and(|s| !s.is_empty()))
            .map(|s| (*s).into())
            .collect();
        assert_eq!(
            find_env_keys(&provider).unwrap(),
            (!expected.is_empty()).then_some(expected.clone())
        );
        assert_credential_eq(
            &(get_env_api_key(&provider).unwrap()),
            &(expected.first().map(|k| std::env::var(k).unwrap())),
            "account_tokens_keep_declared_precedence",
        );
        return;
    }
    let fixture = Fixture::new();
    for provider in ["github-copilot", "anthropic"] {
        let keys = PROVIDERS.iter().find(|(p, _)| *p == provider).unwrap().1;
        for mask in 0..(1 << keys.len()) {
            for empty in [false, true] {
                let mut vars = vec![("TEST_PROVIDER", provider)];
                for (i, key) in keys.iter().enumerate().rev() {
                    if mask & (1 << i) != 0 {
                        vars.push((key, *key));
                    } else if empty {
                        vars.push((key, ""));
                    }
                }
                fixture.run("account_tokens_keep_declared_precedence", &vars);
            }
        }
    }
}
#[test]
fn key_strings_keep_nonempty_whitespace() {
    if child() {
        let value = std::env::var("OPENAI_API_KEY").unwrap();
        assert_credential_eq(
            &(get_env_api_key("openai").unwrap()),
            &((!value.is_empty()).then_some(value.clone())),
            "key_strings_keep_nonempty_whitespace",
        );
        assert_eq!(
            find_env_keys("openai").unwrap(),
            (!value.is_empty()).then_some(vec!["OPENAI_API_KEY".into()])
        );
        for provider in ["OPENAI", " openai ", "\u{feff}openai", "\u{85}openai"] {
            assert!(find_env_keys(provider).unwrap().is_none());
            assert!(get_env_api_key(provider).unwrap().is_none());
        }
        return;
    }
    let fixture = Fixture::new();
    for value in ["", "0", " ", "\t\n", "\u{feff}", "\u{85}"] {
        fixture.run(
            "key_strings_keep_nonempty_whitespace",
            &[("OPENAI_API_KEY", value)],
        );
    }
}
#[test]
fn unknown_providers_do_not_search_arbitrary_variables() {
    if child() {
        for provider in ["unknown", "", "openai-codex", "UNKNOWN_API_KEY"] {
            assert!(find_env_keys(provider).unwrap().is_none());
            assert!(get_env_api_key(provider).unwrap().is_none());
        }
        assert!(find_env_keys("amazon-bedrock").unwrap().is_none());
        return;
    }
    Fixture::new().run(
        "unknown_providers_do_not_search_arbitrary_variables",
        &[
            ("UNKNOWN_API_KEY", "key"),
            ("OPENAI_CODEX_API_KEY", "key"),
            ("AWS_PROFILE", "profile"),
            ("AWS_ACCESS_KEY_ID", "id"),
            ("AWS_SECRET_ACCESS_KEY", "secret"),
            ("AWS_BEARER_TOKEN_BEDROCK", "token"),
            ("AWS_CONTAINER_CREDENTIALS_RELATIVE_URI", "relative"),
            ("AWS_CONTAINER_CREDENTIALS_FULL_URI", "full"),
            ("AWS_WEB_IDENTITY_TOKEN_FILE", "file"),
        ],
    );
}
const INHERITED: &[(&str, &str)] = &[
    ("constructor", "function Object() { [native code] }"),
    (
        "__defineGetter__",
        "function __defineGetter__() { [native code] }",
    ),
    (
        "__defineSetter__",
        "function __defineSetter__() { [native code] }",
    ),
    (
        "hasOwnProperty",
        "function hasOwnProperty() { [native code] }",
    ),
    (
        "__lookupGetter__",
        "function __lookupGetter__() { [native code] }",
    ),
    (
        "__lookupSetter__",
        "function __lookupSetter__() { [native code] }",
    ),
    (
        "isPrototypeOf",
        "function isPrototypeOf() { [native code] }",
    ),
    (
        "propertyIsEnumerable",
        "function propertyIsEnumerable() { [native code] }",
    ),
    ("toString", "function toString() { [native code] }"),
    ("valueOf", "function valueOf() { [native code] }"),
    ("__proto__", "[object Object]"),
    (
        "toLocaleString",
        "function toLocaleString() { [native code] }",
    ),
];

#[test]
fn inherited_provider_names_keep_key_coercions() {
    if child() {
        let provider = std::env::var("TEST_PROVIDER").unwrap();
        let key = INHERITED.iter().find(|(p, _)| *p == provider).unwrap().1;
        let value = std::env::var(key).ok().filter(|s| !s.is_empty());
        assert_eq!(
            find_env_keys(&provider).unwrap(),
            value.as_ref().map(|_| vec![key.into()])
        );
        assert_credential_eq(
            &(get_env_api_key(&provider).unwrap()),
            &(value),
            "inherited_provider_names_keep_key_coercions",
        );
        return;
    }
    let fixture = Fixture::new();
    for &(provider, key) in INHERITED {
        fixture.run(
            "inherited_provider_names_keep_key_coercions",
            &[("TEST_PROVIDER", provider)],
        );
        for value in ["", "controlled-key"] {
            fixture.run(
                "inherited_provider_names_keep_key_coercions",
                &[("TEST_PROVIDER", provider), (key, value)],
            );
        }
    }
}
#[test]
fn vertex_key_preempts_all_ambient_requirements() {
    if child() {
        let key = std::env::var("GOOGLE_CLOUD_API_KEY").unwrap();
        assert_eq!(
            find_env_keys("google-vertex").unwrap(),
            (!key.is_empty()).then_some(vec!["GOOGLE_CLOUD_API_KEY".into()])
        );
        assert_credential_eq(
            &(get_env_api_key("google-vertex").unwrap()),
            &((!key.is_empty()).then_some(key)),
            "vertex_key_preempts_all_ambient_requirements",
        );
        return;
    }
    for value in ["key", ""] {
        Fixture::new().run(
            "vertex_key_preempts_all_ambient_requirements",
            &[("GOOGLE_CLOUD_API_KEY", value)],
        );
    }
}
#[test]
fn vertex_marker_requires_file_project_and_location() {
    if child() {
        assert!(find_env_keys("google-vertex").unwrap().is_none());
        let expected = std::env::var("TEST_EXPECT").unwrap() == "yes";
        assert_credential_eq(
            &(get_env_api_key("google-vertex").unwrap()),
            &(expected.then(|| "<authenticated>".into())),
            "vertex_marker_requires_file_project_and_location",
        );
        return;
    }
    let fixture = Fixture::new();
    let file = fixture.root.join("adc");
    for contents in ["", "not JSON"] {
        std::fs::write(&file, contents).unwrap();
        for project_key in ["GOOGLE_CLOUD_PROJECT", "GCLOUD_PROJECT"] {
            for mask in 0..8 {
                let path = if mask & 1 != 0 {
                    file.to_str().unwrap()
                } else {
                    "missing"
                };
                let vars = [
                    ("GOOGLE_APPLICATION_CREDENTIALS", path),
                    (project_key, if mask & 2 != 0 { "project" } else { "" }),
                    (
                        "GOOGLE_CLOUD_LOCATION",
                        if mask & 4 != 0 { "location" } else { "" },
                    ),
                    ("TEST_EXPECT", if mask == 7 { "yes" } else { "no" }),
                ];
                fixture.run("vertex_marker_requires_file_project_and_location", &vars);
            }
        }
    }
    let dir = fixture.root.join("directory");
    std::fs::create_dir(&dir).unwrap();
    for path in [&file, &dir] {
        fixture.run(
            "vertex_marker_requires_file_project_and_location",
            &[
                ("GOOGLE_APPLICATION_CREDENTIALS", path.to_str().unwrap()),
                ("GOOGLE_CLOUD_PROJECT", " "),
                ("GOOGLE_CLOUD_LOCATION", "\t\n"),
                ("TEST_EXPECT", "yes"),
            ],
        );
    }
    #[cfg(unix)]
    {
        let link = fixture.root.join("link");
        std::os::unix::fs::symlink(&file, &link).unwrap();
        fixture.run(
            "vertex_marker_requires_file_project_and_location",
            &[
                ("GOOGLE_APPLICATION_CREDENTIALS", link.to_str().unwrap()),
                ("GOOGLE_CLOUD_PROJECT", "p"),
                ("GOOGLE_CLOUD_LOCATION", "l"),
                ("TEST_EXPECT", "yes"),
            ],
        );
        std::fs::remove_file(&file).unwrap();
        fixture.run(
            "vertex_marker_requires_file_project_and_location",
            &[
                ("GOOGLE_APPLICATION_CREDENTIALS", link.to_str().unwrap()),
                ("GOOGLE_CLOUD_PROJECT", "p"),
                ("GOOGLE_CLOUD_LOCATION", "l"),
                ("TEST_EXPECT", "no"),
            ],
        );
    }
}
fn default_adc(fixture: &Fixture) -> std::path::PathBuf {
    let path = fixture
        .root
        .join("home/.config/gcloud/application_default_credentials.json");
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(&path, "").unwrap();
    path
}
#[test]
fn vertex_explicit_path_masks_default_credentials() {
    if child() {
        let expected = std::env::var("TEST_EXPECT").unwrap() == "yes";
        assert_credential_eq(
            &(get_env_api_key("google-vertex").unwrap()),
            &(expected.then(|| "<authenticated>".into())),
            "vertex_explicit_path_masks_default_credentials",
        );
        return;
    }
    let fixture = Fixture::new();
    default_adc(&fixture);
    std::fs::write(fixture.root.join("relative.json"), "").unwrap();
    for (path, expected) in [
        ("missing", "no"),
        ("", "yes"),
        ("relative.json", "yes"),
        ("~/relative.json", "no"),
    ] {
        fixture.run(
            "vertex_explicit_path_masks_default_credentials",
            &[
                ("GOOGLE_APPLICATION_CREDENTIALS", path),
                ("GOOGLE_CLOUD_PROJECT", "p"),
                ("GOOGLE_CLOUD_LOCATION", "l"),
                ("TEST_EXPECT", expected),
            ],
        );
    }
    std::fs::remove_file(
        fixture
            .root
            .join("home/.config/gcloud/application_default_credentials.json"),
    )
    .unwrap();
    let xdg = fixture
        .root
        .join("config/gcloud/application_default_credentials.json");
    std::fs::create_dir_all(xdg.parent().unwrap()).unwrap();
    std::fs::write(xdg, "").unwrap();
    fixture.run(
        "vertex_explicit_path_masks_default_credentials",
        &[
            ("GOOGLE_CLOUD_PROJECT", "p"),
            ("GOOGLE_CLOUD_LOCATION", "l"),
            ("TEST_EXPECT", "no"),
        ],
    );
}
#[test]
fn vertex_probe_results_persist_within_process() {
    if child() {
        let path = std::env::var("GOOGLE_APPLICATION_CREDENTIALS").unwrap();
        let exists = std::env::var("TEST_EXISTS").unwrap() == "yes";
        let expected = exists.then(|| "<authenticated>".to_owned());
        assert_credential_eq(
            &(get_env_api_key("google-vertex").unwrap()),
            &(expected),
            "vertex_probe_results_persist_within_process",
        );
        if exists {
            std::fs::remove_file(path).unwrap();
        } else {
            std::fs::write(path, "").unwrap();
        }
        assert_credential_eq(
            &(get_env_api_key("google-vertex").unwrap()),
            &(expected),
            "vertex_probe_results_persist_within_process",
        );
        return;
    }
    for exists in [false, true] {
        let fixture = Fixture::new();
        let path = fixture.root.join("adc");
        if exists {
            std::fs::write(&path, "").unwrap();
        }
        fixture.run(
            "vertex_probe_results_persist_within_process",
            &[
                ("GOOGLE_APPLICATION_CREDENTIALS", path.to_str().unwrap()),
                ("GOOGLE_CLOUD_PROJECT", "p"),
                ("GOOGLE_CLOUD_LOCATION", "l"),
                ("TEST_EXISTS", if exists { "yes" } else { "no" }),
            ],
        );
    }
}
#[test]
fn vertex_default_path_normalizes_before_probe() {
    if child() {
        assert_credential_eq(
            &(get_env_api_key("google-vertex").unwrap()),
            &(Some("<authenticated>".into())),
            "vertex_default_path_normalizes_before_probe",
        );
        return;
    }
    let fixture = Fixture::new();
    default_adc(&fixture);
    #[cfg(unix)]
    {
        let other = fixture.root.join("other/nested");
        std::fs::create_dir_all(&other).unwrap();
        std::os::unix::fs::symlink(&other, fixture.root.join("home/link")).unwrap();
        let home = format!("{}//link/.././", fixture.root.join("home").display());
        fixture.run(
            "vertex_default_path_normalizes_before_probe",
            &[
                ("HOME", &home),
                ("GOOGLE_CLOUD_PROJECT", "p"),
                ("GOOGLE_CLOUD_LOCATION", "l"),
            ],
        );
    }
    #[cfg(not(unix))]
    {
        let home = format!("{}\\unused\\..\\.\\", fixture.root.join("home").display());
        fixture.run(
            "vertex_default_path_normalizes_before_probe",
            &[
                ("USERPROFILE", &home),
                ("GOOGLE_CLOUD_PROJECT", "p"),
                ("GOOGLE_CLOUD_LOCATION", "l"),
            ],
        );
    }
}
#[test]
fn bedrock_signals_return_only_authenticated_marker() {
    if child() {
        let expected = std::env::var("TEST_EXPECT").unwrap() == "yes";
        assert!(find_env_keys("amazon-bedrock").unwrap().is_none());
        assert_credential_eq(
            &(get_env_api_key("amazon-bedrock").unwrap()),
            &(expected.then(|| "<authenticated>".into())),
            "bedrock_signals_return_only_authenticated_marker",
        );
        return;
    }
    let fixture = Fixture::new();
    for key in [
        "AWS_PROFILE",
        "AWS_BEARER_TOKEN_BEDROCK",
        "AWS_CONTAINER_CREDENTIALS_RELATIVE_URI",
        "AWS_CONTAINER_CREDENTIALS_FULL_URI",
        "AWS_WEB_IDENTITY_TOKEN_FILE",
    ] {
        for value in ["unread-missing-file-or-url", " ", ""] {
            fixture.run(
                "bedrock_signals_return_only_authenticated_marker",
                &[
                    (key, value),
                    ("TEST_EXPECT", if value.is_empty() { "no" } else { "yes" }),
                ],
            );
        }
    }
    for value in ["key", " ", ""] {
        fixture.run(
            "bedrock_signals_return_only_authenticated_marker",
            &[
                ("AWS_ACCESS_KEY_ID", value),
                ("AWS_SECRET_ACCESS_KEY", value),
                ("TEST_EXPECT", if value.is_empty() { "no" } else { "yes" }),
            ],
        );
    }
    for key in [
        "AWS_ACCESS_KEY_ID",
        "AWS_SECRET_ACCESS_KEY",
        "AWS_SESSION_TOKEN",
        "AWS_REGION",
        "AWS_DEFAULT_PROFILE",
    ] {
        fixture.run(
            "bedrock_signals_return_only_authenticated_marker",
            &[(key, "value"), ("TEST_EXPECT", "no")],
        );
    }
    fixture.run(
        "bedrock_signals_return_only_authenticated_marker",
        &[("TEST_EXPECT", "no")],
    );
}
#[test]
fn environment_helpers_leave_output_streams_empty() {
    if child() {
        assert_credential_eq(
            &(get_env_api_key("fireworks").unwrap()),
            &(Some("controlled-key".into())),
            "environment_helpers_leave_output_streams_empty",
        );
        assert!(find_env_keys("unknown").unwrap().is_none());
        assert_credential_eq(
            &(get_env_api_key("amazon-bedrock").unwrap()),
            &(Some("<authenticated>".into())),
            "environment_helpers_leave_output_streams_empty",
        );
        assert!(get_env_api_key("google-vertex").unwrap().is_none());
        std::process::exit(0);
    }
    let output = Fixture::new().run(
        "environment_helpers_leave_output_streams_empty",
        &[
            ("FIREWORKS_API_KEY", "controlled-key"),
            ("AWS_PROFILE", "unread-profile"),
        ],
    );
    assert_eq!(output.stderr, b"");
    assert_eq!(output.stdout, b"\nrunning 1 test\n");
}
#[test]
fn authentication_guide_preserves_complete_text() {
    let expected = "# Request authentication\n\nModel invocation forwards a supplied `StreamOptions.api_key` unchanged. It does\nnot select credentials, check empty secrets, authorize secret-free endpoints,\noverlay headers or call an auth resolver. Resolve the credential owner explicitly\nbefore supplying an adapter's api key. See [provider credentials](credentials.md).\n\n`AuthResolver`, `RequestAuth`, `SecretString`, `AuthStatus`, `TokenExchange` and\n`TokenExchangeResult` remain the credential owner's records and interfaces.\n`SecretString::expose()` is deliberate access; its Debug output is a fixed marker,\nnot memory zeroization. Source labels and credential names must be non-secret.\nConfigured status is metadata, not validation or a prediction of request success.\n\nThe credential owner supplies precedence, persistence, helper execution and\ncancellation behavior. Token exchange is an explicit credential primitive, not\nan invocation side effect. Adapters are responsible for keeping supplied secrets\nout of generated content and diagnostics. Supplied option/header maps are\nsensitive surfaces and must not be logged.\n\n## Environment credentials\n\nfind_env_keys reports the nonempty environment-variable names for a provider in precedence order; it does not return their values or list ambient credential signals. get_env_api_key returns the first value or the ambient marker `<authenticated>`. Both return Result with None for ordinary absence. Provider names are exact and values are not trimmed.\n\nAnthropic checks ANTHROPIC_OAUTH_TOKEN before ANTHROPIC_API_KEY. GitHub Copilot checks COPILOT_GITHUB_TOKEN, GH_TOKEN and GITHUB_TOKEN in that order. Fireworks checks FIREWORKS_API_KEY. The built-in provider table also retains its inherited-name string coercions; an unknown provider otherwise has no key.\n\nGoogle Vertex first checks GOOGLE_CLOUD_API_KEY. Without that key, it requires an existing GOOGLE_APPLICATION_CREDENTIALS path, or the conventional home/.config/gcloud/application_default_credentials.json path when no explicit path is set, plus GOOGLE_CLOUD_PROJECT or GCLOUD_PROJECT and GOOGLE_CLOUD_LOCATION. Only existence is checked; an explicit missing path does not fall back. The completed existence probe is cached for the process lifetime.\n\nAmazon Bedrock recognizes AWS_PROFILE, paired AWS_ACCESS_KEY_ID and AWS_SECRET_ACCESS_KEY, AWS_BEARER_TOKEN_BEDROCK, AWS_CONTAINER_CREDENTIALS_RELATIVE_URI, AWS_CONTAINER_CREDENTIALS_FULL_URI or AWS_WEB_IDENTITY_TOKEN_FILE. Any nonempty alternative returns `<authenticated>` without loading or refreshing credentials. These signals never appear in find_env_keys.\n\nNative lookup uses the process environment and native filesystem. The empty-environment proc recovery branch is restricted to a Bun host and caches both successful and failed reads; ordinary native Rust execution is not that host. A temporarily unavailable native filesystem facility does not cache absence. Browser hosts have no native credential files; a missing process object produces ReferenceError with message process is not defined when the selected branch reads it. An ordinary unknown provider can still return None without reading process.\n\nThe helpers emit no console output, execute no secret command, perform no network request and impose no global authentication gate. They expose environment values only through the deliberate value lookup. Stored credentials, login, token refresh and provider invocation remain separate operations.\n\n```rust\nuse maestro_models::{find_env_keys, get_env_api_key};\n\nassert!(find_env_keys(\"controlled-unknown-provider\").unwrap().is_none());\nassert!(get_env_api_key(\"controlled-unknown-provider\").unwrap().is_none());\n```\n";
    assert_eq!(
        include_str!("../../../docs/request-authentication.md"),
        format!(
            "{expected}{}",
            "\nBrowser environment shims use JavaScript truthiness for key discovery and ambient signals, including boolean and numeric values. Value lookup returns JavaScript string coercion only after selecting and rereading a key; discovery and ambient checks do not coerce values. Thrown JSON-compatible host values retain their payload, and Error values retain name, message, stack, code, errno and cause.\n"
        )
    );
}
