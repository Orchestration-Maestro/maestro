//! The files and environment of a profile row: configuration and credentials files, a web
//! identity token, an SSO token cache and a credential process that counts its runs.

use std::fmt::Write as _;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};

use crate::TestResult;
use crate::profile_credentials::Input;

/// SHA-1 of the SSO session name `controlled`, the name of its token cache file.
const SESSION_CACHE: &str = "547ff52d6c8b5cc815ced9a661d370a81cedfca5";
/// SHA-1 of the SSO start URL, the name of its token cache file.
const START_URL_CACHE: &str = "75567da3b6824c56ac2533fa2ad4a97a99c10974";
const ROLE_ARN: &str = "arn:aws:iam::123456789012:role";

/// The credential process: counts each run, then answers as its first argument says.
const PROCESS: &str = r#"#!/bin/sh
echo run >> "$(dirname "$0")/process-count"
runs=$(wc -l < "$(dirname "$0")/process-count")
case "$1" in
fail) exit 7 ;;
once) if [ "$runs" -eq 1 ]; then exit 7; fi ;;
invalid_json) printf broken; exit 0 ;;
esac
version=1
if [ "$1" = wrong_version ]; then version=2; fi
secret='"SecretAccessKey":"controlled-secret",'
if [ "$1" = missing_key ]; then secret=''; fi
expiry=''
if [ "$1" = expired ]; then expiry=',"Expiration":"2000-01-01T00:00:00Z"'; fi
printf '{"Version":%s,"AccessKeyId":"process-key",%s"SessionToken":"process-token"%s}' "$version" "$secret" "$expiry"
"#;

/// A row's directory, removed when dropped.
pub struct Workdir(PathBuf);

impl Drop for Workdir {
    fn drop(&mut self) {
        std::fs::remove_dir_all(&self.0).ok();
    }
}

/// Run `install` so the script is executable without this process ever holding it open.
fn install_script(directory: &Path) -> TestResult {
    let source = directory.join("process-source");
    std::fs::write(&source, PROCESS)?;
    let status = Command::new("install")
        .args(["-m", "0700"])
        .arg(&source)
        .arg(directory.join("process"))
        .status()?;
    assert!(status.success(), "install failed");
    Ok(())
}

impl Workdir {
    /// Create the directory of a row and write everything its input asks for.
    pub fn create(input: &Input) -> TestResult<Self> {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let path = std::env::temp_dir().join(format!(
            "maestro-profile-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir(&path)?;
        let directory = Self(path);
        directory.write(input)?;
        Ok(directory)
    }

    fn file(&self, name: &str) -> PathBuf {
        self.0.join(name)
    }

    fn command(&self, mode: &str) -> String {
        format!("{} {mode}", self.file("process").display())
    }

    fn write(&self, input: &Input) -> TestResult {
        std::fs::write(self.file("token"), "controlled-web-token")?;
        std::fs::write(self.file("process-count"), "")?;
        install_script(&self.0)?;
        if input.sso_cache.is_some()
            || matches!(input.sso.as_deref(), Some("valid" | "session" | "expired"))
        {
            self.write_sso_cache(input)?;
        }
        std::fs::write(self.file("config"), self.config(input))?;
        let credentials = if input.credentials_override.is_some() {
            "[selected]\naws_access_key_id = credentials-key\naws_secret_access_key = credential-secret\n"
        } else if input.split.is_some() {
            "[selected]\naws_secret_access_key = controlled-secret\n"
        } else {
            ""
        };
        std::fs::write(self.file("credentials"), credentials)?;
        Ok(())
    }

    fn write_sso_cache(&self, input: &Input) -> TestResult {
        let cache = self.0.join(".aws/sso/cache");
        std::fs::create_dir_all(&cache)?;
        let name = if input.sso.as_deref() == Some("session") {
            SESSION_CACHE
        } else {
            START_URL_CACHE
        };
        let (token, expires) = if input.sso.as_deref() == Some("expired") {
            ("expired-sso-token", "2000-01-01T00:00:00Z")
        } else {
            ("controlled-sso-token", "2099-01-01T00:00:00Z")
        };
        std::fs::write(
            cache.join(format!("{name}.json")),
            format!(r#"{{"accessToken":"{token}","expiresAt":"{expires}"}}"#),
        )?;
        Ok(())
    }

    /// The number of times the credential process ran.
    pub fn process_calls(&self) -> TestResult<usize> {
        Ok(std::fs::read_to_string(self.file("process-count"))?
            .lines()
            .count())
    }

    fn token_file(&self, mode: Option<&str>) -> String {
        let name = if mode == Some("missing") || mode == Some("missing_file") {
            "token-missing"
        } else {
            "token"
        };
        self.file(name).display().to_string()
    }

    /// The configuration file text of a row.
    fn config(&self, input: &Input) -> String {
        let mut config = if input.absent.is_some() {
            String::new()
        } else {
            "[profile selected]\nregion = us-east-1\n".to_owned()
        };
        config += &self.selected_fields(input);
        if let Some(base) = &input.base {
            let _ = write!(config, "[profile base]\n{}", self.base_fields(base));
        }
        if input.sso.as_deref() == Some("session") {
            config += &Self::session_section(input);
        }
        match &input.omit_sso {
            Some(field) => config
                .lines()
                .filter(|line| !line.starts_with(&format!("{field} =")))
                .fold(String::new(), |text, line| text + line + "\n"),
            None => config,
        }
    }

    fn session_section(input: &Input) -> String {
        let mut lines = vec![
            "sso_start_url = https://controlled.awsapps.com/start",
            "sso_region = us-east-1",
        ];
        if input.session_decoy.is_some() {
            lines.extend(["sso_account_id = 999999999999", "sso_role_name = decoy"]);
        }
        let kept = lines.into_iter().filter(|line| {
            input
                .session_omit
                .as_ref()
                .is_none_or(|field| !line.starts_with(&format!("{field} =")))
        });
        kept.fold("[sso-session controlled]\n".to_owned(), |text, line| {
            text + line + "\n"
        })
    }

    fn selected_fields(&self, input: &Input) -> String {
        let mut fields = String::new();
        if input.keys.is_some() {
            fields +=
                "aws_access_key_id = profile-key\naws_secret_access_key = controlled-secret\n";
        }
        if input.session_token.is_some() {
            fields += "aws_session_token = static-session-token\n";
        }
        match input.partial.as_deref() {
            Some("access") => fields += "aws_access_key_id = incomplete-key\n",
            Some("secret") => fields += "aws_secret_access_key = incomplete-secret\n",
            Some("empty") => fields += "aws_access_key_id =\naws_secret_access_key =\n",
            _ => {}
        }
        if let Some(mode) = &input.process {
            let _ = writeln!(fields, "credential_process = {}", self.command(mode));
        }
        if input.role.is_some() || input.web.is_some() {
            let _ = writeln!(
                fields,
                "role_arn = {ROLE_ARN}/selected\nrole_session_name = controlled"
            );
        }
        for (key, value) in [
            ("credential_source", &input.source),
            ("source_profile", &input.source_profile),
            ("duration_seconds", &input.duration),
            ("external_id", &input.external_id),
        ] {
            if let Some(value) = value {
                let _ = writeln!(fields, "{key} = {value}");
            }
        }
        if input.mfa.is_some() {
            fields += "mfa_serial = controlled-mfa\n";
        }
        if let Some(web) = &input.web {
            let _ = writeln!(
                fields,
                "web_identity_token_file = {}",
                self.token_file(web.mode())
            );
        }
        fields += &Self::sso_fields(input);
        if input.login.is_some() {
            fields += "login_session = arn:aws:iam::123456789012:user/controlled\n";
        }
        fields
    }

    fn sso_fields(input: &Input) -> String {
        let mut fields = String::new();
        if let Some(sso) = input.sso.as_deref() {
            fields += "sso_account_id = 123456789012\n";
            fields += match sso {
                "invalid" => "",
                "session" | "missing_session" => {
                    "sso_role_name = selected\nsso_session = controlled\n"
                }
                _ => {
                    "sso_role_name = selected\nsso_start_url = https://controlled.awsapps.com/start\nsso_region = us-east-1\n"
                }
            };
        }
        if input.inline_sso.is_some() {
            fields += "sso_start_url = https://decoy.awsapps.com/start\nsso_region = us-west-2\n";
        }
        fields
    }

    fn base_fields(&self, base: &str) -> String {
        let keys = "aws_access_key_id = source-key\naws_secret_access_key = controlled-secret\n";
        match base {
            "static" => keys.to_owned(),
            "partial" => "aws_access_key_id = source-key\n".to_owned(),
            "process_fail" => format!("credential_process = {}\n", self.command("fail")),
            "cycle" => format!("role_arn = {ROLE_ARN}/base\nsource_profile = selected\n"),
            "environment" => "credential_source = Environment\n".to_owned(),
            "web" => format!(
                "role_arn = {ROLE_ARN}/web\nrole_session_name = controlled\nweb_identity_token_file = {}\n",
                self.token_file(None)
            ),
            "sso" => "sso_account_id = 123456789012\nsso_role_name = selected\nsso_start_url = https://controlled.awsapps.com/start\nsso_region = us-east-1\n".to_owned(),
            "static_role" => format!("{keys}role_arn = {ROLE_ARN}/base\nsource_profile = absent\n"),
            "role" | "role_region" => {
                let region = if base == "role_region" {
                    "region = eu-west-1\n"
                } else {
                    ""
                };
                format!(
                    "role_arn = {ROLE_ARN}/base\nsource_profile = bottom\nrole_session_name = base-session\n{region}[profile bottom]\n{keys}"
                )
            }
            _ => String::new(),
        }
    }

    /// The variables of the child that runs a row, with every service at `endpoint`.
    pub fn environment(&self, input: &Input, endpoint: &str) -> Vec<(String, String)> {
        let path = |name: &str| self.file(name).display().to_string();
        let mut vars = vec![
            ("HOME".to_owned(), self.0.display().to_string()),
            ("AWS_CONFIG_FILE".to_owned(), path("config")),
            (
                "AWS_SHARED_CREDENTIALS_FILE".to_owned(),
                path("credentials"),
            ),
            (
                "AWS_EC2_METADATA_DISABLED".to_owned(),
                input.disabled.clone().unwrap_or_else(|| {
                    (if input.instance.is_some() {
                        "false"
                    } else {
                        "true"
                    })
                    .to_owned()
                }),
            ),
            ("AWS_REGION".to_owned(), "us-east-1".to_owned()),
            ("AWS_ENDPOINT_URL_STS".to_owned(), endpoint.to_owned()),
            ("AWS_ENDPOINT_URL_SSO".to_owned(), endpoint.to_owned()),
            (
                "AWS_EC2_METADATA_SERVICE_ENDPOINT".to_owned(),
                endpoint.to_owned(),
            ),
        ];
        let mut set = |name: &str, value: String| vars.push((name.to_owned(), value));
        if input.env_keys.is_some() {
            set("AWS_ACCESS_KEY_ID", "environment-key".to_owned());
            set("AWS_SECRET_ACCESS_KEY", "controlled-secret".to_owned());
        }
        if input.env_partial.is_some() {
            set("AWS_ACCESS_KEY_ID", "environment-key".to_owned());
        }
        if input.container.is_some() {
            set(
                "AWS_CONTAINER_CREDENTIALS_FULL_URI",
                format!("{endpoint}/container"),
            );
        }
        if let Some(web) = &input.env_web {
            set("AWS_WEB_IDENTITY_TOKEN_FILE", self.token_file(web.mode()));
            set("AWS_ROLE_SESSION_NAME", "controlled".to_owned());
            if web.mode() != Some("missing_role") {
                set("AWS_ROLE_ARN", format!("{ROLE_ARN}/web"));
            }
        }
        vars
    }
}
