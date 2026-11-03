//! Shared pinned-browser runtime and native fixture ownership.

use chromiumoxide::cdp::browser_protocol::{browser::BrowserContextId, target::SessionId};
use maestro_test_browser::{
    browser_driver::{Browser, Page, Server, cleanup},
    test_support,
};
use maestro_test_directory::Scratch;
use serde_json::json;
use std::{fs, future::Future, io, path::Path};

/// Run one browser test without enabling test-specific runtime features.
pub(super) fn run<T>(future: impl Future<Output = T>) -> T {
    test_support::runtime().unwrap().block_on(future)
}

/// Own a fresh packaged endpoint and browser for the entire supplied assertion.
pub(super) async fn fixture<T>(
    args: &[String],
    action: impl AsyncFnOnce(&mut Browser, &str) -> io::Result<T>,
) -> io::Result<T> {
    let executable = std::env::var_os("CHROMIUM_PATH").expect("pinned job requires CHROMIUM_PATH");
    let root = Scratch::new().unwrap();
    fs::write(
        root.path().join("index.html"),
        "<!doctype html><body></body>",
    )
    .unwrap();
    fs::write(
        root.path().join("helper.js"),
        "export const answer='helper-ok'",
    )
    .unwrap();
    fs::write(
        root.path().join("sandbox.js"),
        "export default async function(){};export function ready(){throw Error('must not run')};export const not_callable=9;",
    ).unwrap();
    fs::write(
        root.path().join("programs.js"),
        include_str!("../../../maestro-web/tests/sandbox_loading_support/programs.js"),
    )
    .unwrap();
    fs::write(
        root.path().join("native.js"),
        include_str!("../../../maestro-web/tests/sandbox_loading_support/native.js"),
    )
    .unwrap();
    let profile = Scratch::new().unwrap();
    let server = Server::start(root.path()).await.unwrap();
    let browser = Browser::launch(Path::new(&executable), profile.path(), args).await;
    let result = match browser {
        Ok(mut browser) => {
            let result = action(&mut browser, &server.origin).await;
            let closed = browser.close().await;
            cleanup(result, closed)
        }
        Err(error) => {
            let error = cleanup::<()>(Err(error), server.close().await).unwrap_err();
            panic!("browser fixture startup: {error}");
        }
    };
    cleanup(result, server.close().await)
}

/// Create an independent page through the same browser connection.
pub(super) async fn foreign_page(page: &mut Page<'_>) -> io::Result<(BrowserContextId, SessionId)> {
    let created = page
        .send(
            None,
            "Target.createBrowserContext",
            json!({"disposeOnDetach":true}),
        )
        .await?;
    let context = BrowserContextId::from(created["browserContextId"].as_str().unwrap().to_owned());
    let target = page
        .send(
            None,
            "Target.createTarget",
            json!({"url":"about:blank","browserContextId":context}),
        )
        .await?;
    let attached = page
        .send(
            None,
            "Target.attachToTarget",
            json!({"targetId":target["targetId"],"flatten":true}),
        )
        .await?;
    let session = SessionId::from(attached["sessionId"].as_str().unwrap().to_owned());
    Ok((context, session))
}

/// Mount the exact pointer-reporting document with nested, SVG and decoy variations.
pub(super) const MOUNT: &str = r#"async (nested=false, count=0, svg=false) => {
  const {program_input}=await import('/programs.js');
  for(let i=0;i<=count;i++) {
    const frame=document.createElement('iframe');if(i===0) frame.id='obsolete';
    const loaded=new Promise(resolve=>frame.onload=resolve);document.body.append(frame);await loaded;
  }
  let doc=document;
  if(nested){const host=document.createElement('iframe');const loaded=new Promise(resolve=>host.onload=resolve);document.body.append(host);await loaded;doc=host.contentDocument;}
  const decoy=doc.createElement('div');decoy.id='probe:"雪]-decoy';doc.body.append(decoy);
  const wrong=doc.createElement('iframe');wrong.srcdoc='<script>window.S={value:"wrong"}</script><button id="target" onclick="window.S.clicked=true">decoy</button>';
  const wrongLoaded=new Promise(resolve=>wrong.onload=resolve);decoy.append(wrong);await wrongLoaded;window.decoyWitness=wrong;
  const box=doc.createElement('div');box.id='probe:"雪]';doc.body.append(box);
  const target=doc.createElement('iframe');target.sandbox='allow-scripts allow-modals';
  target.srcdoc=program_input('click-document').replace('<head>','<head><meta http-equiv="x-test" content="retained">')
    .replace('</script>','window.S={value:"right",started:true};window.done=Promise.resolve();</script>')
    +(svg?'<body><svg width="200" height="40"><rect id="target" width="150" height="30"/></svg>':'<body><button id="target" onclick="window.S.clicked=true">click</button>')+'</body>';
  const loaded=new Promise(resolve=>target.onload=resolve);box.append(target);await loaded;
  window.selected=target;return true;
}"#;

/// The selected sandbox state, independent of containing-document access.
pub(super) fn state() -> serde_json::Value {
    serde_json::json!({"value":"right","started":true,"head":"meta,script","body":"button","scaffold":0,"metaParent":"head","origin":"null","href":"about:srcdoc"})
}

/// Raise an actual unhandled srcdoc exception and await the document's load signal.
pub(super) const POLICY: &str = r"(async () => {
  const frame=document.createElement('iframe');frame.srcdoc='<script>window.complete()</script>';
  const loaded=new Promise(resolve=>frame.onload=resolve);document.body.append(frame);await loaded;
})()";
