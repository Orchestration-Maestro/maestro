//! Configuration cases that run in a child process holding only the case's environment.

use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use maestro_models::StreamOptions;
use maestro_models::providers::cloud::amazon_bedrock::{BedrockOptions, stream_bedrock};

use crate::bedrock;
use crate::chat::{TestResult, block_on};
use crate::child_process;
use crate::transport;

/// What a case expects the SDK to do.
pub enum Expect {
    /// Send a signed request to a URL starting so, signed for a region with an access key.
    Signed {
        url: String,
        region: &'static str,
        key: &'static str,
    },
    /// Send a request whose Authorization header starts so.
    Authorized(&'static str),
    /// Fail before anything reaches the sender.
    Fails,
}

impl Expect {
    /// A request signed for `region` to the standard host of that region.
    pub fn standard(region: &'static str, key: &'static str) -> Self {
        let url = format!("https://bedrock-runtime.{region}.amazonaws.com/model/");
        Self::Signed { url, region, key }
    }

    /// A request signed for `region` to a URL starting with `url`.
    pub fn signed(url: &str, region: &'static str, key: &'static str) -> Self {
        Self::Signed {
            url: url.to_owned(),
            region,
            key,
        }
    }
}

/// One configuration case.
pub struct Case {
    pub id: &'static str,
    pub base_url: &'static str,
    pub region: Option<&'static str>,
    pub profile: Option<&'static str>,
    pub bearer: Option<&'static str>,
    pub env: &'static [(&'static str, &'static str)],
    pub expect: Expect,
}

impl Case {
    /// A case with no options and no environment, to be adjusted by struct update.
    pub const fn new(id: &'static str, base_url: &'static str, expect: Expect) -> Self {
        Self {
            id,
            base_url,
            region: None,
            profile: None,
            bearer: None,
            env: &[],
            expect,
        }
    }
}

/// Shared-config and credentials files holding two distinguishable synthetic profiles.
pub struct AwsFiles(PathBuf);

impl AwsFiles {
    pub fn create(test: &str) -> TestResult<Self> {
        Self::create_with(
            test,
            "[default]\nregion = ap-south-1\n[profile selected]\nregion = us-west-2\n",
            "[default]\naws_access_key_id = controlled-default\naws_secret_access_key = controlled-secret\n\
             [selected]\naws_access_key_id = controlled-selected\naws_secret_access_key = controlled-secret\n",
        )
    }

    /// Create exclusively owned configuration files for a synthetic profile.
    pub fn create_with(test: &str, config: &str, credentials: &str) -> TestResult<Self> {
        let dir =
            std::env::temp_dir().join(format!("maestro-bedrock-{test}-{}", std::process::id()));
        std::fs::create_dir(&dir)?;
        let files = Self(dir);
        std::fs::write(files.0.join("config"), config)?;
        std::fs::write(files.0.join("credentials"), credentials)?;
        Ok(files)
    }

    /// The environment of a child: the files, no metadata service, and the case's variables.
    pub fn environment(&self, case: &Case) -> Vec<(String, String)> {
        let path = |name: &str| self.0.join(name).to_string_lossy().into_owned();
        let mut vars = vec![
            ("HOME".to_owned(), self.0.to_string_lossy().into_owned()),
            ("AWS_CONFIG_FILE".to_owned(), path("config")),
            (
                "AWS_SHARED_CREDENTIALS_FILE".to_owned(),
                path("credentials"),
            ),
            ("AWS_EC2_METADATA_DISABLED".to_owned(), "true".to_owned()),
        ];
        vars.extend(
            case.env
                .iter()
                .map(|(k, v)| ((*k).to_owned(), (*v).to_owned())),
        );
        vars
    }
}

impl Drop for AwsFiles {
    fn drop(&mut self) {
        std::fs::remove_dir_all(&self.0).ok();
    }
}

/// Run every case of `test` in its own child, or the case this process was started for.
pub fn run_cases(test: &str, cases: &[Case]) -> TestResult {
    if let Some(id) = child_process::child_case() {
        let case = cases
            .iter()
            .find(|case| case.id == id)
            .ok_or("unknown case")?;
        return block_on(true, check(case));
    }
    let files = AwsFiles::create(test)?;
    for case in cases {
        let vars = files.environment(case);
        let vars: Vec<(&str, &str)> = vars.iter().map(|(k, v)| (k.as_str(), v.as_str())).collect();
        child_process::rerun(test, case.id, &vars)?;
    }
    Ok(())
}

/// The options of a case: the controlled sender, a counting payload hook and the case's inputs.
fn options_of(
    case: &Case,
    scripted: &transport::Transport,
    hooked: &Arc<AtomicUsize>,
) -> BedrockOptions {
    let hooked = Arc::clone(hooked);
    BedrockOptions {
        common: StreamOptions {
            fetch: Some(scripted.fetch.clone()),
            on_payload: Some(Arc::new(move |payload, _| {
                hooked.fetch_add(1, Ordering::SeqCst);
                Box::pin(std::future::ready(Ok(payload)))
            })),
            ..StreamOptions::default()
        },
        region: case.region.map(str::to_owned),
        profile: case.profile.map(str::to_owned),
        bearer_token: case.bearer.map(str::to_owned),
        ..BedrockOptions::default()
    }
}

/// Run one case and check what the SDK sent.
async fn check(case: &Case) -> TestResult {
    let id = case.id;
    let hooked = Arc::new(AtomicUsize::new(0));
    let scripted = bedrock::serve(bedrock::answer("answer")?);
    let options = options_of(case, &scripted, &hooked);
    let stream = stream_bedrock(
        bedrock::model(case.base_url)?,
        bedrock::context("question")?,
        Some(options),
    );
    let (_, message) = transport::drain(&stream).await?;
    assert_eq!(hooked.load(Ordering::SeqCst), 1, "{id}: hook");
    let requests = scripted.seen()?;
    if let Expect::Fails = case.expect {
        assert!(requests.is_empty(), "{id}: reached the sender");
        let failed = message.error_message.as_deref();
        assert!(failed.is_some_and(|text| !text.is_empty()), "{id}: error");
        return Ok(());
    }
    assert_eq!(message.error_message, None, "{id}: failed");
    let request = requests.first().ok_or("no request")?;
    let authorization = request.headers.get("authorization").ok_or("unsigned")?;
    match &case.expect {
        Expect::Signed { url, region, key } => {
            assert!(
                request.url.starts_with(url.as_str()),
                "{id}: url {}",
                request.url
            );
            assert_eq!(
                bedrock::credential_scope(authorization),
                Some((*key, *region)),
                "{id}: scope"
            );
        }
        Expect::Authorized(prefix) => {
            assert!(authorization.starts_with(prefix), "{id}: authorization");
        }
        Expect::Fails => {}
    }
    Ok(())
}
