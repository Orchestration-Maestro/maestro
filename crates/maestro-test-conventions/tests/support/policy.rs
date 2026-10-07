#![cfg(test)]

use crate::contribution_policy::Process;
use crate::support::Workspace;
use serde_json::{Value, json};
use std::collections::VecDeque;
use std::path::{Path, PathBuf};
use std::process::{ExitStatus, Output};

#[derive(Debug)]
pub struct Request {
    pub cwd: PathBuf,
    pub program: String,
    pub args: Vec<String>,
    pub input: Option<Value>,
}

pub struct RecordingProcess {
    pub diagnostics: Vec<String>,
    pub git_head: String,
    pub requests: Vec<Request>,
    pub replies: VecDeque<Result<Value, String>>,
}

impl Default for RecordingProcess {
    fn default() -> Self {
        Self {
            diagnostics: vec![],
            git_head: "trusted-head".into(),
            requests: vec![],
            replies: VecDeque::new(),
        }
    }
}

impl RecordingProcess {
    pub fn reply(&mut self, value: Value) {
        self.replies.push_back(Ok(value));
    }
    pub fn fail(&mut self) {
        self.replies.push_back(Err("controlled failure".into()));
    }
}

impl Process for RecordingProcess {
    fn diagnostic(&mut self, message: &str) {
        self.diagnostics.push(message.into());
    }
    fn output(
        &mut self,
        cwd: &Path,
        program: &str,
        args: &[String],
        input: Option<&[u8]>,
    ) -> Result<Output, String> {
        self.requests.push(Request {
            cwd: cwd.into(),
            program: program.into(),
            args: args.into(),
            input: input.map(|b| serde_json::from_slice(b).unwrap()),
        });
        if program == "git" {
            return Ok(Output {
                status: ExitStatus::default(),
                stdout: format!("{}\n", self.git_head).into_bytes(),
                stderr: vec![],
            });
        }
        let response = self
            .replies
            .pop_front()
            .expect("unexpected process operation");
        let response = match response {
            Ok(value) => value,
            Err(error) if error == "gh stderr" => {
                #[cfg(unix)]
                {
                    use std::os::unix::process::ExitStatusExt;
                    return Ok(Output {
                        status: ExitStatus::from_raw(256),
                        stdout: vec![],
                        stderr: b"  controlled gh failure\n".to_vec(),
                    });
                }
                #[cfg(not(unix))]
                return Err("controlled gh failure".into());
            }
            Err(error) => return Err(error),
        };
        Ok(Output {
            status: ExitStatus::default(),
            stdout: serde_json::to_vec(&response).unwrap(),
            stderr: vec![],
        })
    }
}

pub fn fixture() -> Workspace {
    let w = Workspace::new();
    std::fs::create_dir(w.root.join(".github")).unwrap();
    std::fs::write(w.root.join(".github/APPROVED_CONTRIBUTORS"), "").unwrap();
    std::fs::write(w.root.join(".github/repository-policy.json"), serde_json::to_vec(&json!({
        "issue_gate": {"message_mode":"normal", "weekend_days":[], "weekend_message":"", "weekend_labels":[], "refactor_until":"", "refactor_branch":"", "refactor_reason":"", "refactor_labels":[]},
        "help_url":"", "activity_gate":{"repositories":[],"label":""}
    })).unwrap()).unwrap();
    w
}

pub fn event(action: &str) -> Value {
    json!({"action":action,"repository":{"full_name":"fixture/project","name":"project","owner":{"login":"fixture"},"default_branch":"trunk"},
      "issue":{"number":7,"user":{"login":"Alice"},"created_at":"2026-10-09T00:00:00Z"},
      "comment":{"id":42,"body":"lgtm","user":{"login":"Reviewer"}}})
}

pub fn update_replies(p: &mut RecordingProcess) {
    p.reply(json!({"permission":"write"}));
    p.reply(json!({"object":{"sha":"trusted-head"}}));
    p.reply(json!({}));
    p.reply(json!({"data":{"createCommitOnBranch":{"commit":{"oid":"signed-head"}}}}));
    p.reply(json!({"number":81,"html_url":"https://github.com/fixture/project/pull/81"}));
    p.reply(json!({}));
}

pub fn changed_content(p: &RecordingProcess) -> String {
    use base64::Engine;
    let b64 = p
        .requests
        .iter()
        .find_map(|r| {
            r.input
                .as_ref()?
                .pointer("/variables/input/fileChanges/additions/0/contents")
                .and_then(Value::as_str)
        })
        .unwrap();
    String::from_utf8(
        base64::engine::general_purpose::STANDARD
            .decode(b64)
            .unwrap(),
    )
    .unwrap()
}

pub fn content(p: &mut RecordingProcess, text: &str) {
    use base64::Engine;
    p.reply(json!({"content":base64::engine::general_purpose::STANDARD.encode(text)}));
}

pub fn gate_event(pr: bool) -> Value {
    let mut e = event("opened");
    if pr {
        e["pull_request"] = e["issue"].clone();
    }
    e
}

pub fn configure(w: &Workspace, edit: impl FnOnce(&mut Value)) {
    let path = w.root.join(".github/repository-policy.json");
    let mut v: Value = serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
    edit(&mut v);
    std::fs::write(path, serde_json::to_vec(&v).unwrap()).unwrap();
}

pub fn completion_event() -> Value {
    let mut e = event("closed");
    e["pull_request"] = json!({"number":81,"merged":true,"user":{"login":"policy-app[bot]"},"base":{"ref":"trunk","repo":{"full_name":"fixture/project"}},"head":{"ref":"chore/approve-contributor-7-42","repo":{"full_name":"fixture/project"}},"changed_files":1,"body":"<!-- maestro-approval:{\"issue\":7,\"comment\":42,\"author\":\"Alice\",\"capability\":\"pr\"} -->"});
    e
}

pub fn completion_replies(p: &mut RecordingProcess, e: &Value, effective: &str) {
    p.reply(e["pull_request"].clone());
    p.reply(json!([{"filename":".github/APPROVED_CONTRIBUTORS","status":"modified"}]));
    p.reply(e["issue"].clone());
    let mut comment = e["comment"].clone();
    comment["issue_url"] = json!("https://api.github.com/repos/fixture/project/issues/7");
    p.reply(comment);
    p.reply(json!({"permission":"write"}));
    content(p, effective);
    p.reply(json!({}));
}
