//! WebAssembly signature witnesses, not executed browser observations.
#![cfg(target_arch = "wasm32")]
use maestro_app::browser::components::sandbox::providers::{
    SandboxRespond, SandboxRuntimeProvider,
};
use maestro_app::browser::components::sandboxed_iframe::SandboxValue;
use maestro_app::{BoxFuture, DiagnosticErrorInfo, SharedHandle};
use maestro_web::browser::values::BrowserSandboxValue;
use maestro_web::components::sandbox::routing::{
    MessageType, RuntimeMessageBridgeOptions, RuntimeMessageContext,
};
use maestro_web::components::sandbox::runtime_message_router::{
    MessageConsumer, RuntimeMessageRouter,
};
use maestro_web::{RUNTIME_MESSAGE_ROUTER, RuntimeMessageBridge};
use std::{cell::RefCell, rc::Rc, sync::Arc};
use wasm_bindgen::JsValue;

/// Provider preserving raw native callbacks and values.
struct Provider;
impl SandboxRuntimeProvider<BrowserSandboxValue> for Provider {
    fn get_data(
        &self,
    ) -> Result<indexmap::IndexMap<String, BrowserSandboxValue>, DiagnosticErrorInfo> {
        Ok(indexmap::IndexMap::new())
    }
    fn get_runtime(&self) -> Result<String, DiagnosticErrorInfo> {
        Ok(String::new())
    }
    fn get_description(&self) -> Result<String, DiagnosticErrorInfo> {
        Ok(String::new())
    }
    fn handle_message(
        &self,
        message: BrowserSandboxValue,
        respond: SandboxRespond<BrowserSandboxValue>,
    ) -> BoxFuture<Result<(), DiagnosticErrorInfo>> {
        Box::pin(async move { respond(message) })
    }
}
/// Consumer accepting the same raw native handle.
struct Consumer;
impl MessageConsumer<BrowserSandboxValue> for Consumer {
    fn handle_message(
        &self,
        _message: BrowserSandboxValue,
    ) -> BoxFuture<Result<(), DiagnosticErrorInfo>> {
        Box::pin(async { Ok(()) })
    }
}

#[test]
fn sandbox_sdk_exports_retain_shared_router() {
    let _: &std::thread::LocalKey<RuntimeMessageRouter> = &RUNTIME_MESSAGE_ROUTER;
    let _ = [MessageType::RequestResponse, MessageType::FireAndForget];
    let _ = RuntimeMessageBridge::generate_bridge_code(RuntimeMessageBridgeOptions {
        context: RuntimeMessageContext::UserScript,
        sandbox_id: "sdk",
    });
    let router = RuntimeMessageRouter::default();
    let provider: SharedHandle<dyn SandboxRuntimeProvider<BrowserSandboxValue>> =
        SharedHandle::new(Provider);
    let consumer: SharedHandle<dyn MessageConsumer<BrowserSandboxValue>> =
        SharedHandle::new(Consumer);
    let _ = router.register_sandbox(
        "sdk".into(),
        Rc::new(RefCell::new(vec![provider])),
        vec![consumer.clone()],
    );
    router.add_consumer("sdk", consumer.clone());
    router.remove_consumer("sdk", &consumer);
    let _: fn(&RuntimeMessageRouter, &str, web_sys::HtmlIFrameElement) =
        RuntimeMessageRouter::set_sandbox_iframe;
    let _ = router.unregister_sandbox("sdk");
}

#[test]
fn raw_sandbox_values_reach_provider_and_consumer_contracts() {
    let raw = BrowserSandboxValue(JsValue::UNDEFINED);
    let cloned = raw.clone();
    let _: &JsValue = &cloned.0;
    let _ = raw.format_repl_value();
    let reply: SandboxRespond<BrowserSandboxValue> = SharedHandle::new(|_value| Ok(()));
    let _: BoxFuture<Result<(), DiagnosticErrorInfo>> = Provider.handle_message(raw.clone(), reply);
    let _: BoxFuture<Result<(), DiagnosticErrorInfo>> = Consumer.handle_message(raw);
}

#[test]
fn concrete_providers_keep_native_browser_handles() {
    use maestro_web::{
        AttachmentsRuntimeProvider, ConsoleLog, ConsoleRuntimeProvider, DownloadableFile,
        FileDownloadRuntimeProvider,
    };
    let console = SharedHandle::new(ConsoleRuntimeProvider::<BrowserSandboxValue>::new());
    let files = SharedHandle::new(FileDownloadRuntimeProvider::<BrowserSandboxValue>::new());
    let attachments = SharedHandle::new(AttachmentsRuntimeProvider::<BrowserSandboxValue>::new(
        Arc::default(),
    ));
    let raw = BrowserSandboxValue(js_sys::Uint8Array::new_with_length(3).into());
    let _: ConsoleLog<BrowserSandboxValue> = ConsoleLog {
        r#type:
            maestro_app::browser::components::sandbox::console_runtime_provider::ConsoleLogType::Log,
        text: raw.clone(),
        args: Some(raw.clone()),
    };
    let _: DownloadableFile<BrowserSandboxValue> = DownloadableFile {
        file_name: BrowserSandboxValue(JsValue::UNDEFINED),
        content: raw,
        mime_type: BrowserSandboxValue(JsValue::NULL),
    };
    let router = RuntimeMessageRouter::default();
    let _ = router.register_sandbox(
        "concrete".into(),
        Rc::new(RefCell::new(vec![console, attachments, files])),
        vec![],
    );
    let _ = router.unregister_sandbox("concrete");
}

#[test]
fn artifact_storage_provider_serves_local_panels() {
    use maestro_app::browser::components::sandbox::artifacts_runtime_provider::AgentLike;
    use maestro_app::browser::tools::documents::{
        ArtifactsPanelState, LocalArtifactPresentation, LocalArtifactsPanel,
    };
    use maestro_web::ArtifactsRuntimeProvider;
    /// Read-write storage over the browser's local panel and conversation.
    fn serve(
        panel: &LocalArtifactsPanel,
        agent: Option<SharedHandle<dyn AgentLike>>,
    ) -> SharedHandle<dyn SandboxRuntimeProvider<BrowserSandboxValue>> {
        SharedHandle::new(ArtifactsRuntimeProvider::<LocalArtifactPresentation>::new(
            panel, agent, true,
        ))
    }
    /// Read-only HTML providers from the live panel state.
    fn html(
        state: &ArtifactsPanelState,
    ) -> Vec<SharedHandle<dyn SandboxRuntimeProvider<BrowserSandboxValue>>> {
        state.get_html_artifact_runtime_providers()
    }
    let _: fn(_, _) -> _ = serve;
    let _: fn(_) -> _ = html;
}
