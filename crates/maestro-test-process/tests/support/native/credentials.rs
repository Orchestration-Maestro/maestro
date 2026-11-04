//! Finite credential-process scenarios selected by a sibling record.

use std::ffi::OsString;
use std::io;

use maestro_test_process::child::{leave, write_stdout};

/// The file an AWS credential process appends one line to on every run.
const RUNS: &str = "process-count";
/// The status a controlled failing process ends with.
const FAILURE: i32 = 7;
/// The trailing document field of a process whose credentials already expired.
const EXPIRED: &str = r#","Expiration":"2000-01-01T00:00:00Z""#;
/// The document a Google external account executable source prints.
const GOOGLE_SOURCE: &str = concat!(
    r#"{"version":1,"success":true,"token_type":"urn:ietf:params:oauth:token-type:jwt","#,
    r#""id_token":"subject-exec","expiration_time":4102444800}"#,
);

/// What the installed alias stands in for.
#[derive(Clone, Copy)]
pub(super) enum Scenario {
    /// An AWS credential process, whose variant arrives as the single operand.
    AwsProcess,
    /// A Google external account executable source, which takes no operand.
    GoogleExecutable,
}

/// A credentials record, parsed completely before any file or stream changes.
pub(super) struct Record {
    /// The protocol the installed alias serves.
    scenario: Scenario,
}

impl Record {
    /// Read the record, rejecting unknown scenarios and extra lines.
    pub(super) fn parse(text: &str) -> io::Result<Self> {
        let scenario = match text.lines().collect::<Vec<_>>().as_slice() {
            ["credentials", "AwsProcess"] => Scenario::AwsProcess,
            ["credentials", "GoogleExecutable"] => Scenario::GoogleExecutable,
            _ => return Err(io::Error::other("unknown credentials scenario")),
        };
        Ok(Self { scenario })
    }
}

/// How one run of an AWS credential process answers.
#[derive(Clone, Copy)]
enum Answer {
    /// A complete credential document.
    Complete,
    /// End with the failure status and no output.
    Fail,
    /// Fail once, then answer completely.
    Once,
    /// Print text that is not JSON.
    InvalidJson,
    /// Name a version the reader refuses.
    WrongVersion,
    /// Omit the secret key.
    MissingKey,
    /// Expire long before the reader's clock.
    Expired,
}

impl Answer {
    /// Parse the variant operand; an absent operand is a complete document.
    fn parse(operands: &[OsString]) -> io::Result<Self> {
        let variant = match operands {
            [] => return Ok(Self::Complete),
            [variant] => variant
                .to_str()
                .ok_or_else(|| io::Error::other("a credential-process variant must be text"))?,
            _ => {
                return Err(io::Error::other(
                    "a credential process takes at most one variant",
                ));
            }
        };
        match variant {
            "ok" => Ok(Self::Complete),
            "fail" => Ok(Self::Fail),
            "once" => Ok(Self::Once),
            "invalid_json" => Ok(Self::InvalidJson),
            "wrong_version" => Ok(Self::WrongVersion),
            "missing_key" => Ok(Self::MissingKey),
            "expired" => Ok(Self::Expired),
            _ => Err(io::Error::other("unknown credential-process variant")),
        }
    }
}

/// One credential document with the requested version, secret presence and trailing field.
fn document(version: u8, secret: bool, tail: &str) -> String {
    let secret = if secret {
        r#""SecretAccessKey":"controlled-secret","#
    } else {
        ""
    };
    format!(
        r#"{{"Version":{version},"AccessKeyId":"process-key",{secret}"SessionToken":"process-token"{tail}}}"#
    )
}

/// The output and status of one run, once the run has been counted.
fn outcome(answer: Answer, runs: usize) -> (String, i32) {
    match answer {
        Answer::Fail => (String::new(), FAILURE),
        Answer::Once if runs == 1 => (String::new(), FAILURE),
        Answer::Once | Answer::Complete => (document(1, true, ""), 0),
        Answer::InvalidJson => ("broken".to_owned(), 0),
        Answer::WrongVersion => (document(2, true, ""), 0),
        Answer::MissingKey => (document(1, false, ""), 0),
        Answer::Expired => (document(1, true, EXPIRED), 0),
    }
}

/// Append this run to the count beside the installed alias and report how many ran.
fn count_run() -> io::Result<usize> {
    let path = std::env::current_exe()?.with_file_name(RUNS);
    let mut text = match std::fs::read_to_string(&path) {
        Ok(text) => text,
        Err(error) if error.kind() == io::ErrorKind::NotFound => String::new(),
        Err(error) => return Err(error),
    };
    text.push_str("run\n");
    std::fs::write(&path, &text)?;
    Ok(text.lines().count())
}

/// Answer one AWS credential process run, counting it first.
fn aws_process(operands: &[OsString]) -> io::Result<()> {
    let answer = Answer::parse(operands)?;
    let (output, status) = outcome(answer, count_run()?);
    write_stdout(output.as_bytes())?;
    leave(status)
}

/// Answer the installed alias's single protocol.
pub(super) fn run(record: &Record, operands: &[OsString]) -> io::Result<()> {
    match record.scenario {
        Scenario::AwsProcess => aws_process(operands),
        Scenario::GoogleExecutable => {
            if !operands.is_empty() {
                return Err(io::Error::other("an executable source takes no operand"));
            }
            write_stdout(GOOGLE_SOURCE.as_bytes())?;
            leave(0)
        }
    }
}
