//! A supplied adapter failure follows a real native pipe prefix through the production host.
#![cfg(test)]
use crate::support::Root;
use maestro_app::ReadonlySessionManager;
use maestro_extensions_wasmtime::native_io::{
    ChildProcess, ExitStatus, IoFuture, NativeIo, Readable, SpawnOptions, Writable,
};
use serde_json::{Value, json};
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
    sync::Arc,
};

/// A real application adapter whose output consumer supplies one selected failure.
struct FailedIo(Rc<dyn NativeIo>);
impl NativeIo for FailedIo {
    fn watch(
        &self,
        path: String,
        listener: Rc<dyn Fn()>,
    ) -> Result<Rc<dyn maestro_extensions_wasmtime::native_io::FsWatcher>, String> {
        self.0.watch(path, listener)
    }
    fn fetch(
        &self,
        request: maestro_extensions_wasmtime::native_io::Request,
    ) -> IoFuture<'_, Rc<dyn maestro_extensions_wasmtime::native_io::Response>> {
        self.0.fetch(request)
    }
    fn spawn(
        &self,
        command: String,
        args: Vec<String>,
        options: SpawnOptions,
    ) -> IoFuture<'_, Rc<dyn ChildProcess>> {
        Box::pin(async move {
            let child = self.0.spawn(command, args, options).await?;
            let output = child.stdout().ok_or("expected native output")?;
            Ok(Rc::new(FailedChild {
                child,
                output: Rc::new(FailedRead {
                    output,
                    delivered: Cell::new(0),
                }),
            }) as Rc<dyn ChildProcess>)
        })
    }
    fn kill(&self, pid: i32, signal: Option<String>) -> Result<(), String> {
        self.0.kill(pid, signal)
    }
}
/// Controls and other descriptors still belong to the actual application child adapter.
struct FailedChild {
    /// The actual child, including its original waiter.
    child: Rc<dyn ChildProcess>,
    /// The deliberately replaced output consumer.
    output: Rc<dyn Readable>,
}
impl ChildProcess for FailedChild {
    fn pid(&self) -> Option<u32> {
        self.child.pid()
    }
    fn stdin(&self) -> Option<Rc<dyn Writable>> {
        self.child.stdin()
    }
    fn stdout(&self) -> Option<Rc<dyn Readable>> {
        Some(Rc::clone(&self.output))
    }
    fn stderr(&self) -> Option<Rc<dyn Readable>> {
        self.child.stderr()
    }
    fn wait(&self) -> IoFuture<'_, ExitStatus> {
        self.child.wait()
    }
    fn kill(&self, signal: Option<String>) -> Result<bool, String> {
        self.child.kill(signal)
    }
}
/// An adapter swap, not a claim that the kernel pipe failed.
struct FailedRead {
    /// The real pipe provides the prefix unchanged.
    output: Rc<dyn Readable>,
    /// Prefix bytes already delivered, regardless of chunk boundaries.
    delivered: Cell<usize>,
}
impl Readable for FailedRead {
    fn read(&self) -> IoFuture<'_, Option<Vec<u8>>> {
        Box::pin(async move {
            if self.delivered.get() >= 3 {
                return Err("native read failed".into());
            }
            let chunk = self.output.read().await?;
            if let Some(bytes) = &chunk {
                self.delivered.set(self.delivered.get() + bytes.len());
            }
            Ok(chunk)
        })
    }
    fn close(&self) -> Result<(), String> {
        self.output.close()
    }
}
/// The controlled catalog never asks to execute a credential command.
struct NoCommands;
impl maestro_app::ConfigValueOperations for NoCommands {
    fn execute(&self, _: &str) -> Option<Vec<u8>> {
        panic!("no configured command requested")
    }
}

/// Loads each component with the supplied native capability.
fn loader(io: Rc<dyn NativeIo>) -> maestro_app::ModuleLoader<maestro_app::Local> {
    Rc::new(move |path: &str| {
        Box::pin(std::future::ready(Ok(Some(
            maestro_extensions_wasmtime::artifact_factory_with_io(
                path.into(),
                crate::support::widgets(),
                maestro_extensions_wasmtime::Installation {
                    version: maestro_app::configuration::config::VERSION.into(),
                    agent_dir: "/controlled-agent".into(),
                },
                Rc::clone(&io),
            ),
        ))))
    })
}

/// Creates the normal embedding session with an explicitly swapped native capability.
pub(super) async fn session(
    root: &Root,
    local: &Rc<tokio::task::LocalSet>,
    io: Rc<dyn NativeIo>,
) -> Rc<maestro_app::AgentSession> {
    root.models("http://127.0.0.1:1/v1");
    let auth = Arc::new(maestro_app::AuthStorage::in_memory(
        maestro_app::AuthStorageData::default(),
    ));
    let registry = Arc::new(
        maestro_app::ModelRegistry::create(
            auth.clone(),
            root.path("agent/models.json").display().to_string(),
            maestro_app::SharedHandle::new(NoCommands),
        )
        .unwrap(),
    );
    let model = registry.find("startup", "controlled").unwrap();
    let mut resources =
        maestro_app::ResourceLoaderOptions::<maestro_app::Local>::new(Rc::clone(local), loader(io));
    resources
        .additional_extension_paths
        .push(std::env::var("MAESTRO_NATIVE_IO_COMPONENT").unwrap());
    resources.offline = Some(true);
    let cwd = root.path("project").display().to_string();
    let options = maestro_app::CreateAgentSessionOptions {
        cwd: Some(cwd.clone()),
        agent_dir: Some(root.path("agent").display().to_string()),
        auth_storage: Some(auth),
        model_registry: Some(registry),
        model: Some(model),
        thinking_level: None,
        scoped_models: Vec::new(),
        no_tools: Some(maestro_app::NoTools::All),
        tools: None,
        custom_tools: Vec::new(),
        resources: maestro_app::SessionResources::Default(Box::new(resources)),
        session_manager: Some(Rc::new(RefCell::new(
            maestro_app::SessionManager::in_memory(Some(&cwd)).unwrap(),
        ))),
        settings_manager: Some(Rc::new(RefCell::new(
            maestro_app::SettingsManager::in_memory(maestro_app::Settings::default()),
        ))),
        session_start_event: None,
        theme: maestro_theme::LiveTheme::default(),
        offline: Some(true),
    };
    maestro_app::create_agent_session(options)
        .await
        .unwrap()
        .session
}

/// Reads the guest's own recorded reply without installing another user-interface host.
pub(super) async fn answer(session: &Rc<maestro_app::AgentSession>, data: Value) -> Value {
    session
        .prompt(
            &format!("/native-child {data}"),
            maestro_app::PromptOptions::default(),
        )
        .await
        .unwrap();
    let entries = serde_json::to_value(session.session_manager().borrow().get_entries()).unwrap();
    entries
        .as_array()
        .unwrap()
        .iter()
        .rev()
        .find(|entry| entry["customType"] == "native-io-result")
        .unwrap()["data"]
        .clone()
}

#[test]
fn native_child_read_failure_follows_the_real_prefix() {
    let root = Root::new();
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    let local = Rc::new(tokio::task::LocalSet::new());
    local.block_on(&runtime,async {
        let io = Rc::new(FailedIo(maestro_app::extension_native_io::extension_native_io(&local)));
        let session = session(&root,&local,io).await;
        let reply = answer(&session,json!({"op":"spawn","command":"python3","args":["-c","import os; os.write(1,b'\\x00\\xff\\x01')"],"env":[],"stdio":["ignore","pipe","ignore"]})).await;
        assert!(reply["value"].as_u64().unwrap() > 0);
        let mut prefix = Vec::new();
        while prefix.len() < 3 {
            prefix.extend(serde_json::from_value::<Vec<u8>>(answer(&session,json!({"op":"read","stderr":false})).await["value"].clone()).unwrap());
        }
        assert_eq!(prefix,[0,255,1]);
        assert_eq!(answer(&session,json!({"op":"read","stderr":false})).await,json!({"error":"native read failed"}));
        assert_eq!(answer(&session,json!({"op":"wait"})).await,json!({"value":{"code":0,"signal":null}}));
    });
}
