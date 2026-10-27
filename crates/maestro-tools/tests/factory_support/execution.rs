//! Selected execution against existing supplied-operation probes and native files.
#![cfg(test)]
use super::{factories, individual, select};
use crate::read_support::Fixture;
use crate::runtime;
use crate::{
    agent_shell_support, edit_support, find_support, grep_support, ls_support, read_support,
    write_support,
};
use maestro_models::{Cancellation, DiagnosticErrorInfo};
use maestro_tools::*;
use serde_json::{Value, json};
use std::sync::{Arc, Mutex, atomic::AtomicUsize};

/// Existing operation probes, kept alive to observe constructor silence and routed operands.
pub struct Probes {
    /// Text/image access probe.
    pub read: Arc<read_support::ScriptedOps>,
    /// Replacement file probe.
    pub edit: Arc<edit_support::Operations>,
    /// Whole-file write probe.
    pub write: Arc<write_support::ScriptedWrite>,
    /// File-pattern probe.
    pub find: Arc<find_support::Scripted>,
    /// Directory probe.
    pub ls: Arc<ls_support::Scripted>,
    /// Search filesystem probe.
    pub grep: Arc<grep_support::Operations>,
    /// Supplied shell/hook transcript.
    pub bash: Arc<Mutex<Vec<Value>>>,
    /// Cancellation shared with these operations.
    pub signal: Cancellation,
    /// Whether the supplied shell operation rejects its command.
    bash_fail: bool,
}

impl Probes {
    /// Fresh independent handles with controlled success or operation failures.
    pub fn new(fail: bool) -> Self {
        let signal = Cancellation::new();
        let read = read_support::ScriptedOps::new(b"read-operations");
        let read = if fail {
            read.at(
                read_support::Stage::Access,
                read_support::Behavior::Fail,
                None,
            )
        } else {
            read
        };
        let edit = edit_support::Operations::new(b"a c");
        let edit = if fail {
            edit.at(
                "access",
                write_support::Mode::Fail(write_support::failure("access denied")),
                None,
            )
        } else {
            edit
        };
        let write = write_support::ScriptedWrite::new(&write_support::Journal::default());
        let write = if fail {
            write.mkdir_mode(write_support::Mode::Fail(write_support::failure(
                "mkdir denied",
            )))
        } else {
            write
        };
        let find_input = if fail {
            json!({"fail":"exists"})
        } else {
            json!({"results":["find-operations"]})
        };
        let ls_input = if fail {
            json!({"fail":"exists"})
        } else {
            json!({"entries":["ls-operations"]})
        };
        Self {
            read: Arc::new(read),
            edit: Arc::new(edit),
            write: Arc::new(write),
            find: Arc::new(find_support::Scripted::new(
                serde_json::from_value(find_input).unwrap(),
                &signal,
            )),
            ls: Arc::new(ls_support::Scripted::new(
                &serde_json::from_value(ls_input).unwrap(),
                &signal,
            )),
            grep: grep_operations(fail, &signal),
            bash: Arc::default(),
            signal,
            bash_fail: fail,
        }
    }

    /// Bind each opaque fixture identity to its existing typed operation owner.
    pub fn options(&self, state: &Value) -> ToolsOptions {
        let mut options = ToolsOptions::default();
        options.read.auto_resize_images = state["read"]["autoResizeImages"].as_bool();
        options.bash = self.shell_options(&state["bash"]);
        if state["read"].get("operations").is_some() {
            assert_eq!(state["read"]["operations"], "read-operations");
            options.read.operations = Some(self.read.clone());
        }
        if state["edit"].get("operations").is_some() {
            assert_eq!(state["edit"]["operations"], "edit-operations");
            options.edit.operations = Some(self.edit.clone());
        }
        if state["write"].get("operations").is_some() {
            assert_eq!(state["write"]["operations"], "write-operations");
            options.write.operations = Some(self.write.clone());
        }
        if state["find"].get("operations").is_some() {
            assert_eq!(state["find"]["operations"], "find-operations");
            options.find.operations = Some(self.find.clone());
        }
        if state["ls"].get("operations").is_some() {
            assert_eq!(state["ls"]["operations"], "ls-operations");
            options.ls.operations = Some(self.ls.clone());
        }
        if state["grep"].get("operations").is_some() {
            assert_eq!(state["grep"]["operations"], "grep-operations");
            options.grep.operations = Some(self.grep.clone());
        }
        options
    }

    /// Bind shell options separately from filesystem operation owners.
    fn shell_options(&self, state: &Value) -> BashToolOptions {
        let mut options = BashToolOptions {
            shell_path: state["shellPath"].as_str().map(str::to_owned),
            command_prefix: state["commandPrefix"].as_str().map(str::to_owned),
            ..BashToolOptions::default()
        };
        if state.get("operations").is_some() {
            assert_eq!(state["operations"], "bash-operations");
            let log = Arc::clone(&self.bash);
            let fail = self.bash_fail;
            options.operations = Some(Arc::new(agent_shell_support::Immediate(Arc::new(
                move |command, cwd, options| shell_operation(&log, fail, command, cwd, options),
            ))));
        }
        if state.get("spawnHook").is_some() {
            assert_eq!(state["spawnHook"], "hook");
            let log = Arc::clone(&self.bash);
            options.spawn_hook = Some(Arc::new(move |context| {
                log.lock()
                    .unwrap()
                    .push(json!(["hook", context.command, context.cwd]));
                Ok(context)
            }));
        }
        options
    }

    /// Every supplied effect in source order per owner, with its actual operand identity.
    pub fn logs(&self) -> Value {
        json!({"read": self.read.log(), "edit":self.edit.events.entries(), "editCalls":self.edit.script.calls(), "write":self.write.calls(), "find":self.find.log(), "ls":self.ls.log(), "grep":*self.grep.calls.lock().unwrap(), "bash":*self.bash.lock().unwrap()})
    }
}

/// The existing immediate shell probe observes operands before producing controlled bytes.
fn shell_operation(
    log: &Mutex<Vec<Value>>,
    fail: bool,
    command: &str,
    cwd: &str,
    options: process::bash::BashExecOptions<'_>,
) -> Result<process::bash::BashExecResult, process::bash::BashError> {
    let process::bash::BashExecOptions {
        on_data, signal, ..
    } = options;
    if signal.is_some_and(Cancellation::is_aborted) {
        return Err(process::bash::BashError::Aborted);
    }
    log.lock().unwrap().push(json!(["exec", command, cwd]));
    if fail {
        return Err(std::io::Error::other("shell denied").into());
    }
    on_data(b"bash-operations");
    Ok(process::bash::BashExecResult {
        exit_code: Some(0.0),
    })
}

/// Construct the existing content-search probe with a caller-owned signal.
fn grep_operations(fail: bool, signal: &Cancellation) -> Arc<grep_support::Operations> {
    Arc::new(grep_support::Operations {
        input: grep_support::Input {
            fail: fail.then(|| "stat".into()),
            ..grep_support::Input::default()
        },
        calls: Mutex::default(),
        signal: signal.clone(),
    })
}

/// Grouped configuration with all distinct operation handles.
pub fn all_options() -> Value {
    json!({"read":{"operations":"read-operations"}, "bash":{"operations":"bash-operations"}, "edit":{"operations":"edit-operations"}, "write":{"operations":"write-operations"}, "grep":{"operations":"grep-operations"}, "find":{"operations":"find-operations"}, "ls":{"operations":"ls-operations"}})
}

/// Arguments whose actual consumers discriminate the selected family.
pub fn arguments(name: &str) -> Value {
    match name {
        "read" => json!({"path":"f"}),
        "bash" => json!({"command":"printf native"}),
        "edit" => json!({"path":"f","edits":[{"oldText":"a","newText":"b"}]}),
        "write" => json!({"path":"out","content":"write-operations"}),
        "grep" => json!({"pattern":"hit"}),
        "find" => json!({"pattern":"*"}),
        "ls" => json!({}),
        _ => panic!("unknown family"),
    }
}

/// Execute the public wrapper, keeping complete results and original errors.
pub fn execute(
    tool: &Tool,
    args: Value,
    signal: Option<Cancellation>,
) -> Result<Value, DiagnosticErrorInfo> {
    runtime::run((tool.execute)("call".into(), args, signal, None)).map(|result| {
        assert!(result.terminate.is_none());
        serde_json::to_value(result).unwrap()
    })
}

/// A native search manager that cannot reach a network service.
pub fn manager(root: &read_support::Scratch) -> Arc<ToolsManager> {
    find_support::manager(root.text(), &Arc::new(AtomicUsize::new(0)))
}

/// Compare real routed operations, including exact operand order, with the producing owner.
pub fn paired(factory: &str, name: &str, cwd: &str, state: &Value) {
    let root = read_support::Scratch::new().unwrap();
    #[cfg(unix)]
    grep_support::producer(&root, &grep_support::Input::default());
    let a = Probes::new(false);
    let b = Probes::new(false);
    let silent = a.logs();
    let tools = select(factory, name, cwd.into(), a.options(state), manager(&root)).unwrap();
    assert_eq!(a.logs(), silent, "construction performed supplied effects");
    for tool in tools
        .into_iter()
        .filter(|t| name.is_empty() || t.definition.name == name)
    {
        let name = tool.definition.name.as_str();
        let individual = individual(name, cwd.into(), b.options(state), manager(&root));
        let mut args = arguments(name);
        if name != "bash" && state[name].get("operations").is_none() {
            root.file("f", b"a c\nhit\n");
            if name == "find" {
                // One match keeps factory equivalence independent of parallel traversal order.
                root.file("match.txt", b"hit");
                args["pattern"] = json!("*.txt");
            }
            args["path"] = json!(if ["read", "edit"].contains(&name) {
                root.path().join("f")
            } else if name == "write" {
                root.path().join("out")
            } else {
                root.path().to_path_buf()
            });
        }
        let actual = execute(&tool, args.clone(), None);
        if state[name].get("operations").is_none() {
            assert_native_bytes(&root, name);
        }
        if name == "edit" && state[name].get("operations").is_none() {
            root.file("f", b"a c\nhit\n");
        }
        assert_eq!(actual, execute(&individual, args, None));
        assert_eq!(a.logs(), b.logs(), "routed effects differ for {name}");
    }
}

/// Exercise every applicable public constructor through native and supplied callers.
pub fn check_operations() {
    for name in ["read", "bash", "edit", "write", "grep", "find", "ls"] {
        for factory in factories(name) {
            paired(factory, name, "/work", &all_options());
            controlled_failure(factory, name);
            native_call(factory, name);
            if name == "read" {
                native_image(factory);
            }
        }
    }
}

/// Errors and pre-abort remain owned by the individual factory.
fn controlled_failure(factory: &str, name: &str) {
    let root = read_support::Scratch::new().unwrap();
    #[cfg(unix)]
    grep_support::producer(&root, &grep_support::Input::default());
    let probes = Probes::new(true);
    let tool = select(
        factory,
        name,
        root.text(),
        probes.options(&all_options()),
        manager(&root),
    )
    .unwrap()
    .into_iter()
    .find(|t| t.definition.name == name)
    .unwrap();
    assert!(
        execute(&tool, arguments(name), None).is_err(),
        "failure not propagated for {name}"
    );
    let before = probes.logs();
    probes.signal.abort();
    assert!(execute(&tool, arguments(name), Some(probes.signal.clone())).is_err());
    assert_eq!(
        probes.logs(),
        before,
        "pre-abort performed supplied effects"
    );
}

/// Native execution is compared on a fresh real file with its producing owner.
fn native_call(factory: &str, name: &str) {
    let root = read_support::Scratch::new().unwrap();
    root.file("f", b"a c\nhit\n");
    let tool = select(
        factory,
        name,
        root.text(),
        ToolsOptions::default(),
        manager(&root),
    )
    .unwrap()
    .into_iter()
    .find(|t| t.definition.name == name)
    .unwrap();
    let result = execute(&tool, arguments(name), None);
    assert_native_bytes(&root, name);
    root.file("f", b"a c\nhit\n");
    if name == "write" {
        let _ = std::fs::remove_file(root.path().join("out"));
    }
    let original = individual(name, root.text(), ToolsOptions::default(), manager(&root));
    assert_eq!(result, execute(&original, arguments(name), None));
    assert!(result.is_ok(), "native {name} failed");
    assert_native_bytes(&root, name);
}

/// Observe selected native file effects before a comparison can reset them.
fn assert_native_bytes(root: &read_support::Scratch, name: &str) {
    if name == "edit" {
        assert_eq!(std::fs::read(root.path().join("f")).unwrap(), b"b c\nhit\n");
    }
    if name == "write" {
        assert_eq!(
            std::fs::read(root.path().join("out")).unwrap(),
            b"write-operations"
        );
    }
}

/// Native image output uses the same selected read caller and image-fitting owner.
fn native_image(factory: &str) {
    let root = read_support::Scratch::new().unwrap();
    root.file(
        "image.png",
        &read_support::blank_image(2, 2, image::ImageOutputFormat::Png),
    );
    let tool = select(
        factory,
        "read",
        root.text(),
        ToolsOptions::default(),
        manager(&root),
    )
    .unwrap()
    .into_iter()
    .find(|t| t.definition.name == "read")
    .unwrap();
    let args = json!({"path":"image.png"});
    let result = execute(&tool, args.clone(), None).unwrap();
    let original = individual("read", root.text(), ToolsOptions::default(), manager(&root));
    assert_eq!(result, execute(&original, args, None).unwrap());
    assert_eq!(result["content"][1]["type"], "image");
    assert_eq!(result["content"][1]["mimeType"], "image/png");
}

/// Corrected UTF-8 write counts and unlimited native find remain visible through selection.
pub fn check_corrections() {
    let root = read_support::Scratch::new().unwrap();
    for factory in factories("write") {
        let tool = select(
            factory,
            "write",
            root.text(),
            ToolsOptions::default(),
            manager(&root),
        )
        .unwrap()
        .into_iter()
        .find(|t| t.definition.name == "write")
        .unwrap();
        for (text, bytes) in [("", 0), ("ascii", 5), ("é", 2), ("😀", 4), ("é😀", 6)] {
            let result = execute(&tool, json!({"path":"out", "content":text}), None).unwrap();
            assert_eq!(
                result["content"][0]["text"],
                format!("Successfully wrote {bytes} bytes to out")
            );
            assert_eq!(
                std::fs::read(root.path().join("out")).unwrap(),
                text.as_bytes()
            );
        }
    }
    unlimited_find(&root);
    #[cfg(unix)]
    ignored_search();
}

/// Zero removes the result-count budget both below and above the default.
fn unlimited_find(root: &read_support::Scratch) {
    root.file("one.txt", b"hit");
    for factory in factories("find") {
        let tool = select(
            factory,
            "find",
            root.text(),
            ToolsOptions::default(),
            manager(root),
        )
        .unwrap()
        .into_iter()
        .find(|t| t.definition.name == "find")
        .unwrap();
        let result = execute(&tool, json!({"pattern":"*.txt", "limit":0}), None).unwrap();
        assert_eq!(result["content"][0]["text"], "one.txt");
        assert!(result.get("details").is_none());
    }
    for n in 0..1001 {
        root.file(&format!("many-{n:04}.txt"), b"hit");
    }
    let tool = create_tool(
        "find",
        root.text(),
        ToolsOptions::default(),
        "/unused/bash".into(),
        manager(root),
    )
    .unwrap();
    let result = execute(&tool, json!({"pattern":"*.txt", "limit":0}), None).unwrap();
    assert_eq!(
        result["content"][0]["text"]
            .as_str()
            .unwrap()
            .lines()
            .count(),
        1002
    );
    assert!(result.get("details").is_none());
}

/// Native traversal keeps user hidden/dependency files while respecting scoped ignore rules.
#[cfg(unix)]
fn ignored_search() {
    for repository in [false, true] {
        let Some(root) = grep_support::tree() else {
            return;
        };
        if repository {
            assert!(
                std::process::Command::new("git")
                    .args(["init", "-q"])
                    .current_dir(root.path())
                    .status()
                    .unwrap()
                    .success()
            );
        }
        for name in ["find", "grep"] {
            for factory in factories(name) {
                let tool = select(
                    factory,
                    name,
                    root.text(),
                    ToolsOptions::default(),
                    manager(&root),
                )
                .unwrap()
                .into_iter()
                .find(|t| t.definition.name == name)
                .unwrap();
                let params = json!({"pattern": if name == "find" { "*" } else { "needle" }});
                let result = execute(&tool, params, None).unwrap();
                let text = result["content"][0]["text"].as_str().unwrap();
                assert!(!text.contains(".git/"));
                assert!(!text.contains("ignored.txt"));
                assert!(text.contains(".secret/hidden.txt"));
                assert!(text.contains("b/local.txt"));
                assert!(!text.contains("a/local.txt"));
                assert!(!text.contains("a/deep/secret.txt"));
                assert!(text.contains("node_modules"));
            }
        }
    }
}
