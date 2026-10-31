//! Shared live-registration witnesses over real native owners.
#![cfg(test)]
use super::*;
use maestro_extensions::ExtensionRunner;
use std::{
    future::Future,
    pin::Pin,
    sync::{Arc, Mutex},
};

#[derive(Clone, Copy)]
pub enum Probe {
    Live,
    Errors,
}
macro_rules! family {
    ($module:ident, $mode:ty, $ptr:ident) => {
        pub mod $module {
            use super::*;
            type Mode = $mode;
            type Operation<'a> = Pin<Box<dyn Future<Output=Result<(),EventError>>+'a>>;
            pub async fn exercise<V:'static,R:'static>(
                name: &'static str, mut event: V,
                invoke: for<'a> fn(&'a ExtensionRunner<Mode>, &'a mut V)->Operation<'a>,
                duplicate: bool,
            ) {
                let bus = create_event_bus_for::<Mode>($ptr::new(|_|{}),$ptr::new(|_|{}));
                let first = support::loaded_for::<Mode>("first",bus.for_extension());
                let second = support::loaded_for::<Mode>("second",bus.for_extension());
                first.owners.write().unwrap().push(second.owner.clone());
                let runner = support::runner_for::<Mode>(&first.owners,$ptr::new(|_|{}));
                let trace = Arc::new(Mutex::new(Vec::new()));
                let (release,waiting) = tokio::sync::oneshot::channel();
                let waiting = Mutex::new(Some(waiting));
                let observed = trace.clone();
                let handler: ExtensionHandler<V,R,Mode> = $ptr::new(move |_,ctx| {
                    assert_eq!(ctx.cwd().unwrap(),"/work");
                    observed.lock().unwrap().push("first");
                    let waiting = waiting.lock().unwrap().take();
                    Box::pin(async move {if let Some(waiting) = waiting {waiting.await.unwrap();} Ok(None)})
                });
                first.api.on(name.into(),handler.clone()).unwrap();
                if duplicate {first.api.on(name.into(),handler).unwrap();}
                let observed = trace.clone();
                second.api.on::<V,R>(name.into(),$ptr::new(move |_,_| {
                    observed.lock().unwrap().push("second");Box::pin(async {Ok(None)})
                })).unwrap();
                let mut operation = std::pin::pin!(invoke(&runner,&mut event));
                let mut context = std::task::Context::from_waker(std::task::Waker::noop());
                assert!(std::future::Future::poll(operation.as_mut(),&mut context).is_pending(),"{name}");
                assert_eq!(*trace.lock().unwrap(),["first"]);
                let observed = trace.clone();
                first.api.on::<V,R>(name.into(),$ptr::new(move |_,_| {
                    observed.lock().unwrap().push("appended");Box::pin(async {Ok(None)})
                })).unwrap();
                release.send(()).unwrap();
                operation.await.unwrap();
                let expected = if duplicate {vec!["first","first","appended","second"]} else {vec!["first","appended","second"]};
                assert_eq!(*trace.lock().unwrap(),expected,"{name}");
            }
            async fn errors<V:'static,R:'static>(name:&'static str, mut event:V, invoke:for<'a> fn(&'a ExtensionRunner<Mode>,&'a mut V)->Operation<'a>) {
                let bus = create_event_bus_for::<Mode>($ptr::new(|_|{}),$ptr::new(|_|{}));
                let loaded = support::loaded_for::<Mode>("initial",bus.for_extension());
                let runner = support::runner_for::<Mode>(&loaded.owners,$ptr::new(|_|{}));
                let errors = Rc::new(RefCell::new(Vec::new()));
                let observed = errors.clone();
                let owner = $ptr::downgrade(&loaded.owner);
                let _remove = runner.on_error(Rc::new(move |error| {
                    owner.upgrade().unwrap().write().unwrap().resolved_path = "listener-can-write".into();
                    observed.borrow_mut().push((error.extension_path.clone(),error.event.clone(),error.error.clone(),error.stack.clone()));Ok(())
                }));
                let owner = $ptr::downgrade(&loaded.owner);
                loaded.api.on::<V,R>(name.into(),$ptr::new(move |_,_| {
                    owner.upgrade().unwrap().write().unwrap().path = "native-path".into();
                    Box::pin(async {Err(std::io::Error::other("native failure").into())})
                })).unwrap();
                let owner = $ptr::downgrade(&loaded.owner);
                loaded.api.on::<V,R>(name.into(),$ptr::new(move |_,_| {
                    owner.upgrade().unwrap().write().unwrap().path = "authored-path".into();
                    Box::pin(async {Err(Box::new(maestro_models::DiagnosticErrorInfo {message:"authored failure".into(),name:None,stack:Some("owned stack".into()),code:None}) as EventError)})
                })).unwrap();
                let healthy = Arc::new(Mutex::new(false));
                let reached = healthy.clone();
                loaded.api.on::<V,R>(name.into(),$ptr::new(move |_,_| { *reached.lock().unwrap()=true;Box::pin(async {Ok(None)})})).unwrap();
                invoke(&runner,&mut event).await.unwrap();
                assert_eq!(*errors.borrow(),[("native-path".into(),name.into(),"native failure".into(),None),("authored-path".into(),name.into(),"authored failure".into(),Some("owned stack".into()))],"{name}");
                assert!(*healthy.lock().unwrap(),"{name}");
            }
            async fn visit<V:'static,R:'static>(name:&'static str,event:V,invoke:for<'a> fn(&'a ExtensionRunner<Mode>,&'a mut V)->Operation<'a>,probe:Probe) {
                match probe {
                    Probe::Live => exercise::<V,R>(name,event,invoke,false).await,
                    Probe::Errors if name != "tool_call" => errors::<V,R>(name,event,invoke).await,
                    Probe::Errors => {}
                }
            }
            async fn notification<V:RunnerEmitEvent>(event:V,probe:Probe) {
                visit::<V,V::Result>(V::EVENT_TYPE,event,|runner,event|Box::pin(async move {runner.emit(event).await.map(|_|())}),probe).await;
            }
            pub async fn all_routes(probe:Probe) {
                notification(from_json::<SessionStartEvent>(json!({"reason":"startup"})),probe).await;
                notification(from_json::<SessionCompactEvent>(json!({"compactionEntry":{"type":"compaction","id":"compact","parentId":null,"timestamp":"2026-10-17T00:00:00Z","summary":"summary","firstKeptEntryId":"kept","tokensBefore":2.5},"fromExtension":false})),probe).await;
                notification(from_json::<SessionShutdownEvent>(json!({"reason":"quit"})),probe).await;
                notification(from_json::<SessionTreeEvent>(json!({"newLeafId":"new","oldLeafId":null})),probe).await;
                notification(AfterProviderResponseEvent {status:200.0,headers:std::collections::BTreeMap::new()},probe).await;
                notification(AgentStartEvent{},probe).await;
                notification(AgentEndEvent::<Value>{messages:vec![]},probe).await;
                notification(TurnStartEvent{turn_index:1.0,timestamp:2.0},probe).await;
                notification(from_json::<TurnEndEvent<Value>>(json!({"turnIndex":1,"message":false,"toolResults":[]})),probe).await;
                notification(MessageStartEvent{message:json!(false)},probe).await;
                notification(message_update_event(),probe).await;
                notification(ToolExecutionStartEvent{tool_call_id:"id".into(),tool_name:"tool".into(),args:json!({})},probe).await;
                notification(ToolExecutionUpdateEvent{tool_call_id:"id".into(),tool_name:"tool".into(),args:json!({}),partial_result:json!(false)},probe).await;
                notification(ToolExecutionEndEvent{tool_call_id:"id".into(),tool_name:"tool".into(),result:json!(false),is_error:false},probe).await;
                notification(ModelSelectEvent{model:model_support::model("live"),previous_model:Presence::Missing,source:ModelSelectSource::Set},probe).await;
                notification(from_json::<ThinkingLevelSelectEvent>(json!({"level":"high","previousLevel":"off"})),probe).await;
                notification(from_json::<SessionBeforeSwitchEvent>(json!({"reason":"new"})),probe).await;
                notification(from_json::<SessionBeforeForkEvent>(json!({"entryId":"entry","position":"at"})),probe).await;
                notification(compact_event(),probe).await;
                notification(tree_event(),probe).await;
                dedicated_routes(probe).await;
            }
            async fn dedicated_routes(probe:Probe) {
                visit::<ContextEvent,ContextEventResult>("context",ContextEvent{messages:vec![]},|runner,event|Box::pin(async move {runner.emit_context(&event.messages).await.map(|_|())}),probe).await;
                visit::<BeforeProviderRequestEvent,Value>("before_provider_request",BeforeProviderRequestEvent{payload:json!({})},|runner,event|Box::pin(async move {runner.emit_before_provider_request(event.payload.clone()).await.map(|_|())}),probe).await;
                visit::<BeforeAgentStartEvent,BeforeAgentStartEventResult>("before_agent_start",from_json(json!({"prompt":"user","systemPrompt":"system","systemPromptOptions":{"cwd":"/work"}})),|runner,event|Box::pin(async move {runner.emit_before_agent_start(event.prompt.clone(),&mut event.images,event.system_prompt.clone(),event.system_prompt_options.clone()).await.map(|_|())}),probe).await;
                let message: maestro_agent::AgentMessage = from_json(json!({"role":"user","content":"user","timestamp":1}));
                visit::<MessageEndEvent<maestro_agent::AgentMessage>,MessageEndEventResult<maestro_agent::AgentMessage>>("message_end",MessageEndEvent{message},|runner,event|Box::pin(async move {runner.emit_message_end(event).await.map(|_|())}),probe).await;
                visit::<ResourcesDiscoverEvent,ResourcesDiscoverResult>("resources_discover",ResourcesDiscoverEvent{cwd:"/work".into(),reason:ResourcesDiscoverReason::Startup},|runner,event|Box::pin(async move {runner.emit_resources_discover(event.cwd.clone(),event.reason).await.map(|_|())}),probe).await;
                visit::<InputEvent,InputEventResult>("input",InputEvent{text:"input".into(),images:Presence::Missing,source:InputSource::Interactive},|runner,event|Box::pin(async move {runner.emit_input(&event.text,&mut event.images,event.source).await.map(|_|())}),probe).await;
                visit::<ToolCallEvent,ToolCallEventResult>("tool_call",ToolCallEvent{tool_call_id:"id".into(),tool_name:"tool".into(),input:json!({})},|runner,event|Box::pin(async move {runner.emit_tool_call(event).await.map(|_|())}),probe).await;
                visit::<ToolResultEvent,ToolResultEventResult>("tool_result",ToolResultEvent{tool_call_id:"id".into(),tool_name:"tool".into(),input:json!({}),content:vec![],details:Presence::Missing,is_error:false},|runner,event|Box::pin(async move {runner.emit_tool_result(event).await.map(|_|())}),probe).await;
                visit::<UserBashEvent,UserBashEventResult<(),()>>("user_bash",UserBashEvent{command:"literal".into(),cwd:"/work".into(),exclude_from_context:false},|runner,event|Box::pin(async move {runner.emit_user_bash::<(),()>(event).await.map(|_|())}),probe).await;
            }
        }
    };
}
family!(local, maestro_models::Local, Rc);
family!(threaded, maestro_models::Threaded, Arc);
