#![allow(dead_code)]
use maestro_credentials::*;
use maestro_models::*;
use std::{
    future::Future,
    sync::Arc,
    task::{Context as TaskContext, Poll, Wake, Waker},
};
struct Notify(std::thread::Thread);
impl Wake for Notify {
    fn wake(self: Arc<Self>) {
        self.0.unpark();
    }
    fn wake_by_ref(self: &Arc<Self>) {
        self.0.unpark();
    }
}
pub fn block_on<F: Future>(future: F) -> F::Output {
    let waker = Waker::from(Arc::new(Notify(std::thread::current())));
    let mut cx = TaskContext::from_waker(&waker);
    let mut future = std::pin::pin!(future);
    loop {
        match future.as_mut().poll(&mut cx) {
            Poll::Ready(value) => return value,
            Poll::Pending => std::thread::park(),
        }
    }
}
pub fn secret(value: &str) -> SecretString {
    SecretString::new(value.into())
}
pub fn auth(value: &str) -> RequestAuth {
    RequestAuth::Secret {
        secret: secret(value),
        source: None,
    }
}
pub struct Literal;
impl SecretResolver for Literal {
    fn environment(&self, _: &str) -> Option<SecretString> {
        None
    }
    fn resolve(
        &self,
        value: SecretString,
        _: Cancellation,
    ) -> std::pin::Pin<
        Box<dyn Future<Output = Result<Option<SecretString>, CredentialError>> + Send + '_>,
    > {
        Box::pin(async move { Ok(Some(value)) })
    }
}
pub fn options() -> CredentialOptions {
    CredentialOptions {
        environment_names: Default::default(),
        fallback: None,
        secrets: Arc::new(Literal),
        now: Arc::new(|| 100),
    }
}
pub fn owner(storage: Arc<dyn CredentialStorage>) -> Arc<Credentials> {
    Arc::new(Credentials::new(storage, options()).unwrap())
}
pub fn memory() -> Arc<MemoryCredentialStorage> {
    Arc::new(MemoryCredentialStorage::new(None))
}
pub fn model(provider: &str) -> Model {
    Model {
        identity: ModelIdentity {
            provider: provider.into(),
            model: "synthetic".into(),
            operation: "chat".into(),
        },
        protocol: "script".into(),
        headers: Default::default(),
    }
}
pub fn context() -> Context {
    Context {
        system_prompt: None,
        messages: vec![UserMessage {
            content: "hello".into(),
            timestamp: 1,
        }],
    }
}
pub fn scripted(count: usize) -> Arc<ScriptedProvider> {
    Arc::new(ScriptedProvider::new(
        (0..count)
            .map(|_| {
                Script::Steps(vec![ScriptStep::Update(ProviderUpdate::Done {
                    reason: StopReason::Stop,
                })])
            })
            .collect(),
    ))
}
pub fn request(
    credentials: Arc<dyn AuthResolver>,
    provider: &str,
) -> (AssistantMessage, Arc<ScriptedProvider>) {
    let fake = scripted(1);
    let mut models = Models::new(Arc::new(|| 123));
    models.register(model(provider), fake.clone()).unwrap();
    let result = block_on(models.complete(
        model(provider),
        context(),
        StreamOptions {
            auth_resolver: Some(credentials),
            ..Default::default()
        },
    ));
    (result, fake)
}
pub fn assert_secret(auth: &RequestAuth, value: &str, source: &str) {
    match auth {
        RequestAuth::Secret {
            secret,
            source: label,
        } => {
            assert_eq!(secret.expose(), value);
            assert_eq!(label.as_deref(), Some(source));
        }
        _ => panic!("expected secret"),
    }
}

pub struct CountingStorage {
    pub inner: Arc<dyn CredentialStorage>,
    pub reads: std::sync::atomic::AtomicUsize,
    pub fail: std::sync::atomic::AtomicBool,
}
impl CountingStorage {
    pub fn new(inner: Arc<dyn CredentialStorage>) -> Arc<Self> {
        Arc::new(Self {
            inner,
            reads: 0.into(),
            fail: false.into(),
        })
    }
}
impl CredentialStorage for CountingStorage {
    fn transact(
        &self,
        cancellation: &Cancellation,
        edit: &mut CredentialTransaction<'_>,
    ) -> Result<(), CredentialError> {
        self.reads.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        if self.fail.load(std::sync::atomic::Ordering::SeqCst) {
            return Err(CredentialError::Storage);
        }
        self.inner.transact(cancellation, edit)
    }
}
pub fn bytes(storage: &dyn CredentialStorage) -> String {
    let mut result = String::new();
    storage
        .transact(&Cancellation::new(), &mut |bytes| {
            result = bytes.map(|s| s.expose().to_owned()).unwrap_or_default();
            Ok(None)
        })
        .unwrap();
    result
}

pub struct Secrets {
    pub environment: std::sync::Mutex<std::collections::BTreeMap<String, String>>,
    pub names: std::sync::Mutex<Vec<String>>,
    pub resolves: std::sync::atomic::AtomicUsize,
    pub unresolved: bool,
}
impl Secrets {
    pub fn new(values: &[(&str, &str)], unresolved: bool) -> Arc<Self> {
        Arc::new(Self {
            environment: std::sync::Mutex::new(
                values
                    .iter()
                    .map(|(k, v)| (k.to_string(), v.to_string()))
                    .collect(),
            ),
            names: Default::default(),
            resolves: 0.into(),
            unresolved,
        })
    }
}
impl SecretResolver for Secrets {
    fn environment(&self, name: &str) -> Option<SecretString> {
        self.names.lock().unwrap().push(name.into());
        self.environment
            .lock()
            .unwrap()
            .get(name)
            .filter(|s| !s.is_empty())
            .map(|s| secret(s))
    }
    fn resolve(
        &self,
        value: SecretString,
        _: Cancellation,
    ) -> std::pin::Pin<
        Box<dyn Future<Output = Result<Option<SecretString>, CredentialError>> + Send + '_>,
    > {
        Box::pin(async move {
            self.resolves
                .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            Ok(if self.unresolved { None } else { Some(value) })
        })
    }
}
pub struct Fallback {
    pub calls: std::sync::Mutex<Vec<(String, Cancellation)>>,
    pub statuses: std::sync::atomic::AtomicUsize,
    pub result: Result<RequestAuth, Failure>,
}
impl Fallback {
    pub fn new(result: Result<RequestAuth, Failure>) -> Arc<Self> {
        Arc::new(Self {
            calls: Default::default(),
            statuses: 0.into(),
            result,
        })
    }
}
impl AuthResolver for Fallback {
    fn status(&self, _: &str) -> AuthStatus {
        self.statuses
            .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        AuthStatus {
            configured: true,
            source: Some("ignored".into()),
        }
    }
    fn resolve(
        &self,
        provider: String,
        cancellation: Cancellation,
    ) -> std::pin::Pin<Box<dyn Future<Output = Result<RequestAuth, Failure>> + Send + '_>> {
        Box::pin(async move {
            self.calls.lock().unwrap().push((provider, cancellation));
            self.result.clone()
        })
    }
}

pub struct Gate {
    pub entered: std::sync::mpsc::Sender<()>,
    pub release: Cancellation,
    pub count: std::sync::atomic::AtomicUsize,
}
impl SecretResolver for Gate {
    fn environment(&self, _: &str) -> Option<SecretString> {
        panic!("unexpected environment lookup")
    }
    fn resolve(
        &self,
        _: SecretString,
        _: Cancellation,
    ) -> std::pin::Pin<
        Box<dyn Future<Output = Result<Option<SecretString>, CredentialError>> + Send + '_>,
    > {
        Box::pin(async move {
            self.count.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            self.entered.send(()).unwrap();
            self.release.cancelled().await;
            Ok(Some(secret("ready")))
        })
    }
}
impl AuthResolver for Gate {
    fn status(&self, _: &str) -> AuthStatus {
        AuthStatus {
            configured: true,
            source: None,
        }
    }
    fn resolve(
        &self,
        _: String,
        _: Cancellation,
    ) -> std::pin::Pin<Box<dyn Future<Output = Result<RequestAuth, Failure>> + Send + '_>> {
        Box::pin(async move {
            self.count.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            self.entered.send(()).unwrap();
            self.release.cancelled().await;
            Ok(auth("ready"))
        })
    }
}
pub fn gate() -> (Arc<Gate>, std::sync::mpsc::Receiver<()>) {
    let (tx, rx) = std::sync::mpsc::channel();
    (
        Arc::new(Gate {
            entered: tx,
            release: Cancellation::new(),
            count: 0.into(),
        }),
        rx,
    )
}

pub struct Scratch(pub std::path::PathBuf);
impl Scratch {
    pub fn new() -> Self {
        static NEXT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
        loop {
            let path = std::env::temp_dir().join(format!(
                "maestro-credentials-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, std::sync::atomic::Ordering::SeqCst)
            ));
            match std::fs::create_dir(&path) {
                Ok(()) => return Self(path),
                Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(e) => panic!("scratch creation failed: {e}"),
            }
        }
    }
}
impl Drop for Scratch {
    fn drop(&mut self) {
        std::fs::remove_dir_all(&self.0).unwrap();
    }
}
pub fn replace(storage: &dyn CredentialStorage, value: &str) {
    storage
        .transact(&Cancellation::new(), &mut |_| Ok(Some(secret(value))))
        .unwrap();
}
pub type AdapterCase = (Arc<dyn CredentialStorage>, Arc<dyn CredentialStorage>, bool);
pub fn adapters(root: &std::path::Path, initial: &str) -> Vec<AdapterCase> {
    let memory: Arc<dyn CredentialStorage> =
        Arc::new(MemoryCredentialStorage::new(Some(secret(initial))));
    let file: Arc<dyn CredentialStorage> =
        Arc::new(FileCredentialStorage::new(root.join("credentials.json")).unwrap());
    replace(file.as_ref(), initial);
    vec![
        (memory.clone(), memory.clone(), true),
        (file.clone(), file.clone(), true),
        (
            Arc::new(ReadOnlyCredentialStorage::new(memory.clone())),
            memory,
            false,
        ),
        (
            Arc::new(ReadOnlyCredentialStorage::new(file.clone())),
            file,
            false,
        ),
    ]
}

pub fn poll_once<F: Future>(future: std::pin::Pin<&mut F>) -> Poll<F::Output> {
    let waker = Waker::from(Arc::new(Notify(std::thread::current())));
    future.poll(&mut TaskContext::from_waker(&waker))
}
