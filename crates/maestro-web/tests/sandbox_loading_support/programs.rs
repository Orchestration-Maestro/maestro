//! Public preparation and production-mounted frame witnesses.
use super::native::{equal, equal_json, program_sources};
use super::{Provider, adapter};
use crate::browser_fixture_support::{get, global, invoke, object, post_from};
use crate::contributions_support::settle;
use crate::sandbox_bootstrap_support::OBSERVER;
use maestro_app::SharedHandle;
use maestro_web::components::sandbox::runtime_bundle::BUNDLE;
use maestro_web::components::sandboxed_iframe::PrepareHtmlOptions;
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};
use wasm_bindgen::{JsCast as _, closure::Closure, prelude::*};
use web_sys::{Element, HtmlIFrameElement};

/// Prepare through the public interface and take frame attributes from actual loading.
pub(crate) async fn program_witness() {
    let loading = super::Fixture::default();
    let sandbox = adapter();
    let providers = vec![SharedHandle::new(Provider {
        fixture: loading.clone(),
        data: js_sys::JSON::parse(&serde_json::json!({"obsData": Fixture::program_input("raw")}).to_string())
            .unwrap(),
        runtime: Fixture::program_input("provider"),
        failure: None,
    }) as SharedHandle<dyn maestro_app::browser::components::sandbox::sandbox_runtime_provider::SandboxRuntimeProvider<super::BrowserSandboxValue>>];
    let html = sandbox
        .prepare_html_document("program", &Fixture::program_input("user"), &providers, None)
        .unwrap();
    let tt_provider = super::provider(
        &loading,
        &Fixture::program_input("provider").replace("if(location.hash==='#tt')", "if(true)"),
    );
    let tt_html = sandbox
        .prepare_html_document(
            "policy",
            &Fixture::program_input("user"),
            &[tt_provider],
            None,
        )
        .unwrap();
    for url in [false, true] {
        sandbox
            .sandbox_url_provider
            .replace(url.then(|| SharedHandle::new(|| Ok(Fixture::program_url())) as _));
        sandbox
            .load_content("prototype", "<head></head>", vec![], vec![])
            .unwrap();
        Fixture::program_checks(&html, &tt_html, url).await;
    }
    sandbox.remove().unwrap();
    super::Fixture::finish();
}

/// The prepared navigation runtime runs in a parsed opaque document.
pub(crate) async fn link_witness() {
    let sandbox = adapter();
    let html = sandbox
        .prepare_html_document(
            "links",
            &Fixture::program_input("click-document"),
            &[],
            Some(PrepareHtmlOptions {
                is_html_artifact: true,
                is_standalone: false,
            }),
        )
        .unwrap();
    sandbox
        .load_content("prototype", "<head></head>", vec![], vec![])
        .unwrap();
    Fixture::link_checks(&html).await;
    sandbox.remove().unwrap();
    super::Fixture::finish();
}

/// All authored insertion points retain head policies on both public loading paths.
pub(crate) async fn artifact_witness() {
    let loading = super::Fixture::default();
    let sandbox = adapter();
    for url in [false, true] {
        sandbox
            .sandbox_url_provider
            .replace(url.then(|| SharedHandle::new(|| Ok(Fixture::program_url())) as _));
        for position in ["head", "html", "fragment"] {
            let mut code = Fixture::artifact();
            if position != "head" {
                code = code.replace("<head>", "").replace("</head>", "");
            }
            if position == "fragment" {
                code = code.replace("<html>", "").replace("</html>", "");
            }
            sandbox
                .load_content(
                    "artifact",
                    &code,
                    vec![super::provider(
                        &loading,
                        &Fixture::program_input("provider"),
                    )],
                    vec![],
                )
                .unwrap();
            Fixture::loaded_artifact(url).await;
        }
    }
    sandbox.remove().unwrap();
    super::Fixture::finish();
}

/// Marker-looking identifiers, provider records and programs remain literal data.
pub(crate) async fn marker_witness() {
    let loading = super::Fixture::default();
    let sandbox = adapter();
    let marker = "__PROGRAM__ __RUNTIME__ __SCRIPT__ __USER_CODE__";
    let providers = vec![SharedHandle::new(Provider {
        fixture: loading.clone(),
        data: js_sys::JSON::parse(&serde_json::json!({marker: marker, "attachment": {"text": marker}}).to_string()).unwrap(),
        runtime: Fixture::program_input("marker-provider"),
        failure: None,
    }) as SharedHandle<dyn maestro_app::browser::components::sandbox::sandbox_runtime_provider::SandboxRuntimeProvider<super::BrowserSandboxValue>>];
    let html = sandbox
        .prepare_html_document(
            marker,
            &Fixture::program_input("marker-user"),
            &providers,
            None,
        )
        .unwrap();
    sandbox
        .load_content("prototype", "<head></head>", vec![], vec![])
        .unwrap();
    Fixture::marker_checks(&html).await;
    sandbox.remove().unwrap();
    super::Fixture::finish();
}

/// A provider runtime defining one marker-looking literal.
const MARKER_PROVIDER: &str = include_str!("program_inputs/marker_provider.js");
/// User code returning every marker-looking value it can read.
const MARKER_USER: &str = include_str!("program_inputs/marker_user.js");
/// A provider runtime observing its own text, data, loading order and an optional policy.
const PROVIDER: &str = include_str!("program_inputs/provider.js");
/// User code observing loading order, module resolution and a preserved carriage return.
const USER: &str = include_str!("program_inputs/user.js");
/// An authored head reporting each click to its parent.
const CLICK_DOCUMENT: &str = include_str!("program_inputs/click_document.html");
/// An authored document with a policy and a parser-time observation.
const ARTIFACT: &str = include_str!("program_inputs/artifact.html");
/// Raw provider data that looks like markup.
const RAW: &str = r"</ScRiPt> <!--<script> </script> 雪";
/// The raw value the user code returns.
const USER_RAW: &str = r"</ScRiPt> <!--<script> </script> <![CDATA[ ]]> &amp; &lt;x> 雪";
/// Prepared programs mounted in frames copied from the production frame, observed in place.
#[derive(Clone, Default)]
pub struct Fixture;

/// One copied production frame inside its own same-origin host frame.
struct Mounted {
    /// The copied frame.
    frame: HtmlIFrameElement,
    /// The host frame holding it.
    host: HtmlIFrameElement,
    /// The copied frame's parent, which the driver inspects by id.
    probe: Element,
}

impl Mounted {
    /// The driver's observation of the frame's retained document and program settlement.
    async fn inspect(&self) -> JsValue {
        inspect(&self.probe.id(), "state").await
    }
}

/// Ask the driver to observe or click the sandbox mounted below the element `id`.
async fn inspect(id: &str, kind: &str) -> JsValue {
    let window = global("window");
    settle(invoke(&window, "inspectSandbox", &[&id.into(), &kind.into()]).unwrap())
        .await
        .unwrap()
}

/// Attribute names and values of an element, in order.
fn attributes(element: &Element) -> js_sys::Array {
    element
        .get_attribute_names()
        .iter()
        .map(|name| -> JsValue {
            let value = element.get_attribute(&name.as_string().unwrap());
            js_sys::Array::of2(&name, &value.into()).into()
        })
        .collect()
}

/// Settle when the copied `frame` reports `kind` to the window `host`, then stop listening.
fn reported(host: &JsValue, frame: &HtmlIFrameElement, kind: &str) -> js_sys::Promise {
    let (host, frame, kind) = (host.clone(), frame.clone(), kind.to_owned());
    js_sys::Promise::new(&mut |resolve, _| {
        let listener: Rc<RefCell<JsValue>> = Rc::default();
        let (registered, window) = (Rc::clone(&listener), host.clone());
        let (frame, kind) = (frame.clone(), kind.clone());
        let callback =
            Closure::<dyn Fn(web_sys::MessageEvent)>::new(move |event: web_sys::MessageEvent| {
                let source = event.source().map(JsValue::from);
                let own = source == frame.content_window().map(JsValue::from);
                let data = event.data();
                if own && data.is_object() && get(&data, "type") == kind.as_str() {
                    invoke(
                        &window,
                        "removeEventListener",
                        &[&"message".into(), &registered.borrow()],
                    )
                    .unwrap();
                    resolve.call1(&JsValue::UNDEFINED, &data).unwrap();
                }
            })
            .into_js_value();
        invoke(&host, "addEventListener", &[&"message".into(), &callback]).unwrap();
        *listener.borrow_mut() = callback;
    })
}

/// The host document a copied frame is mounted in.
#[derive(Clone, Copy)]
enum Host<'a> {
    /// A host with no policy of its own.
    Plain,
    /// A host under a content security `policy`.
    Policy(&'a str),
    /// A host under a `policy` that also installs a TrustedHTML-only default policy.
    TrustedTypes(&'a str),
}

/// Mount `html` the way the production frame loads it, inside a `host` document.
async fn mount(html: &str, url: bool, host_document: Host<'_>) -> Mounted {
    thread_local!(static SERIAL: Cell<u32> = const { Cell::new(0) });
    let page = crate::contributions_support::document();
    let prototype = page.query_selector("[data-sandbox-test] iframe").unwrap();
    let prototype = prototype.expect("production-created frame");
    equal(
        &prototype.has_attribute("src").into(),
        &url.into(),
        "production loading path",
    );
    let host = host(host_document).await;
    let (window, document) = (
        JsValue::from(host.content_window().unwrap()),
        host.content_document().unwrap(),
    );
    let probe = document.create_element("div").unwrap();
    let serial = SERIAL.with(|serial| {
        serial.set(serial.get() + 1);
        serial.get()
    });
    probe.set_id(&format!("program-probe-{serial}"));
    document.body().unwrap().append_child(&probe).unwrap();
    let frame = copy(&document, &prototype);
    let loaded = load(&window, &frame, html, url);
    probe.append_child(&frame).unwrap();
    loaded.await;
    Mounted { frame, host, probe }
}

/// A loaded same-origin host frame with the selected policies.
async fn host(host_document: Host<'_>) -> HtmlIFrameElement {
    let (policy, trusted) = match host_document {
        Host::Plain => ("", false),
        Host::Policy(policy) => (policy, false),
        Host::TrustedTypes(policy) => (policy, true),
    };
    let page = crate::contributions_support::document();
    let host: HtmlIFrameElement = page.create_element("iframe").unwrap().unchecked_into();
    let host_loaded = super::native::loaded(&host);
    page.body().unwrap().append_child(&host).unwrap();
    host_loaded.await;
    if trusted {
        let identity = Closure::<dyn Fn(JsValue) -> JsValue>::new(|text| text).into_js_value();
        let options = object(&[("createHTML", &identity)]);
        let types = get(&host.content_window().unwrap(), "trustedTypes");
        invoke(&types, "createPolicy", &[&"default".into(), &options]).unwrap();
    }
    if !policy.is_empty() {
        let document = host.content_document().unwrap();
        let meta = document.create_element("meta").unwrap();
        meta.set_attribute("http-equiv", "Content-Security-Policy")
            .unwrap();
        meta.set_attribute("content", policy).unwrap();
        let head = document.query_selector("head").unwrap().unwrap();
        head.append_child(&meta).unwrap();
    }
    host
}

/// Load `html` into the copied `frame` the way production does: by URL handshake through
/// the host `window`, or as its source document.
fn load(
    window: &JsValue,
    frame: &HtmlIFrameElement,
    html: &str,
    url: bool,
) -> impl std::future::Future<Output = ()> + use<> {
    let loaded = if url {
        let installed = reported(window, frame, "sandbox-installed");
        let ready = reported(window, frame, "sandbox-ready");
        let (host, frame, html) = (window.clone(), frame.clone(), html.to_owned());
        let send = Closure::once_into_js(move |_: JsValue| {
            let message = object(&[("type", &"sandbox-load".into()), ("code", &html.into())]);
            // Production sends from the frame's parent, which here is the host.
            post_from(&host, &frame.content_window().unwrap(), &message);
        });
        invoke(&ready, "then", &[&send]).unwrap();
        installed
    } else {
        let loaded = js_sys::Promise::new(&mut |resolve, _| {
            let once = object(&[("once", &JsValue::TRUE)]);
            let arguments = [&"load".into(), &resolve.into(), &once.into()];
            invoke(frame, "addEventListener", &arguments).unwrap();
        });
        frame.set_srcdoc(html);
        loaded
    };
    async move {
        settle(loaded.into()).await.unwrap();
    }
}

/// A frame in `document` carrying exactly the production `prototype`'s attributes.
fn copy(document: &web_sys::Document, prototype: &Element) -> HtmlIFrameElement {
    let frame: HtmlIFrameElement = document.create_element("iframe").unwrap().unchecked_into();
    for pair in attributes(prototype).iter() {
        let pair = js_sys::Array::from(&pair);
        frame
            .set_attribute(
                &pair.get(0).as_string().unwrap(),
                &pair.get(1).as_string().unwrap(),
            )
            .unwrap();
    }
    equal(
        &attributes(&frame),
        &attributes(prototype),
        "complete production attribute set copied",
    );
    equal(
        &frame.get_attribute("sandbox").into(),
        &prototype.get_attribute("sandbox").into(),
        "production sandbox flags",
    );
    frame
}

/// The page's content-security-policy source for `text`.
async fn digest(text: &str) -> String {
    use base64::Engine as _;
    let bytes = crate::contributions_support::page::digest(text).await;
    format!(
        "'sha256-{}'",
        base64::engine::general_purpose::STANDARD.encode(bytes)
    )
}

impl Fixture {
    /// Run the exact-text, ordering and policy observations on a production frame's attributes.
    pub(crate) async fn program_checks(html: &str, tt_html: &str, url: bool) {
        assert!(
            html.contains("<template data-maestro-sandbox-provider"),
            "providers use inert template carriers, not a resource"
        );
        let sources = program_sources(html);
        let (runtime, module) = (&sources[0], &sources[1]);
        assert!(runtime.contains(PROVIDER), "provider input is unchanged");
        let (statements, returned) = USER.split_once("return ").unwrap();
        assert!(
            module.contains(statements) && module.contains(returned.trim_end_matches(';')),
            "user statements and returned expression are unchanged including CR"
        );
        let mounted = mount(html, url, Host::Plain).await;
        exact(&mounted.inspect().await, runtime, url);
        mounted.host.remove();
        if !url {
            policies(html, tt_html, runtime, module).await;
        }
    }

    /// Observe literal payload preservation and mounted completion.
    pub(crate) async fn marker_checks(html: &str) {
        let marker = "__PROGRAM__ __RUNTIME__ __SCRIPT__ __USER_CODE__";
        let parser = web_sys::DomParser::new().unwrap();
        let parsed = parser
            .parse_from_string(html, web_sys::SupportedType::TextHtml)
            .unwrap();
        let count =
            |root: &Element, selector: &str| root.query_selector_all(selector).unwrap().length();
        let head = parsed.query_selector("head").unwrap().unwrap();
        equal(
            &count(&head, "template").into(),
            &1.into(),
            "one head carrier",
        );
        equal(
            &count(&parsed.body().unwrap(), "svg > script[type=\"module\"]").into(),
            &1.into(),
            "one body carrier",
        );
        let sources = program_sources(html);
        assert!(
            sources[0].contains(MARKER_PROVIDER),
            "provider source byte-exact"
        );
        assert!(
            sources[1].contains(&format!("'{marker}'")),
            "user literal byte-exact"
        );
        let mounted = mount(html, false, Host::Plain).await;
        let s = mounted.inspect().await;
        equal(
            &get(&s, "error"),
            &JsValue::NULL,
            "successful marker completion",
        );
        let markers: js_sys::Array = std::iter::repeat_n(JsValue::from(marker), 5).collect();
        equal(
            &get(&s, "value"),
            &markers,
            "id, key/value, attachment, provider and user literals",
        );
        mounted.host.remove();
    }

    /// Click parsed anchors through the browser driver and observe native routing.
    pub(crate) async fn link_checks(html: &str) {
        let svg = |attributes: &str| {
            format!(
                r#"<svg width="200" height="40"><a {attributes}><rect id="target" width="150" height="30"/></a></svg>"#
            )
        };
        let animated = svg(r#"href="./initial""#).replace(
            "<rect",
            r#"<set attributeName="href" to="./animated" begin="0s" fill="freeze"/><rect"#,
        );
        let cases = [
            (
                "html",
                r#"<a href="./html"><span id="target">link</span></a>"#.to_owned(),
                Some("html"),
            ),
            ("href", svg(r#"href="./vector""#), Some("vector")),
            (
                "xlink",
                svg(r#"xmlns:xlink="http://www.w3.org/1999/xlink" xlink:href="./xlink""#),
                Some("xlink"),
            ),
            (
                "both",
                svg(r#"href="./preferred" xlink:href="./ignored""#),
                Some("preferred"),
            ),
            ("empty", svg(r#"href="" xlink:href="./ignored""#), Some("")),
            ("missing", svg(""), None),
            ("animated", animated, Some("animated")),
        ];
        for (name, body, target) in cases {
            let document = format!(
                "{}<body>{body}</body>",
                html.replace(
                    "</head>",
                    r#"<base href="https://example.org/links/"></head>"#
                )
            );
            link_case(&mount(&document, false, Host::Plain).await, name, target).await;
        }
    }

    /// Controlled HTTP bootstrap implementing the normal readiness handshake.
    #[must_use]
    pub(crate) fn program_url() -> String {
        format!("{}/sandbox.html", crate::contributions_support::origin())
    }

    /// Authored document with a policy and a parser-time observation.
    #[must_use]
    pub(crate) fn artifact() -> String {
        ARTIFACT.to_owned()
    }

    /// Observe the authored policy in the frame loaded by the public adapter.
    pub(crate) async fn loaded_artifact(url: bool) {
        let frame: HtmlIFrameElement = crate::contributions_support::document()
            .query_selector("[data-sandbox-test] iframe")
            .unwrap()
            .unwrap()
            .unchecked_into();
        if url {
            settle(reported(&global("window"), &frame, "sandbox-installed").into())
                .await
                .unwrap();
        } else {
            super::native::loaded(&frame).await;
        }
        let parent = frame.parent_element().unwrap();
        parent.set_id("artifact-probe");
        let s = inspect(&parent.id(), "state").await;
        let read = |key: &str| get(&s, key);
        let policy = js_sys::Array::of4(
            &read("before"),
            &read("metaParent"),
            &read("blocked"),
            &read("scaffold"),
        );
        equal_json(
            &policy,
            r#"[true,"head",true,0]"#,
            "authored head policy and order",
        );
        equal(
            &read("head"),
            &"style,meta,title,script".into(),
            "head element order at public loading interface",
        );
    }

    /// Provider and module input used in the mounted exactness observations.
    #[must_use]
    pub(crate) fn program_input(kind: &str) -> String {
        match kind {
            "marker-provider" => MARKER_PROVIDER,
            "marker-user" => MARKER_USER,
            "raw" => RAW,
            "provider" => PROVIDER,
            "user" => USER,
            _ => CLICK_DOCUMENT,
        }
        .to_owned()
    }
}

/// The mounted program kept its exact texts, values, ordering, resolution and origin.
fn exact(s: &JsValue, runtime: &str, url: bool) {
    let read = |key: &str| get(s, key);
    equal(
        &read("runtime"),
        &runtime.into(),
        "entire assembled runtime text",
    );
    let values = js_sys::Array::of3(&read("raw"), &read("data"), &read("value"));
    equal(
        &values,
        &js_sys::Array::of3(&RAW.into(), &RAW.into(), &USER_RAW.into()),
        "raw runtime, provider data and user values",
    );
    let ready = if url { "complete" } else { "interactive" };
    let order = js_sys::Array::of4(
        &read("error"),
        &read("cr"),
        &read("ready"),
        &read("beforeDcl"),
    );
    equal(
        &order,
        &js_sys::Array::of4(
            &JsValue::NULL,
            &JsValue::TRUE,
            &ready.into(),
            &JsValue::TRUE,
        ),
        "CR and module ordering",
    );
    let resolution = js_sys::Array::of2(&read("metaUrl"), &read("helper"));
    equal(
        &resolution,
        &js_sys::Array::of2(&read("base"), &"helper-ok".into()),
        "relative module resolution without base",
    );
    let scaffolds = js_sys::Array::of3(&read("head"), &read("body"), &read("scaffold"));
    equal_json(
        &scaffolds,
        r#"["style","",0]"#,
        "no carrier scaffolds remain",
    );
    equal(&read("origin"), &"null".into(), "opaque sandbox origin");
}

/// Under the host `policy`, the supplied provider runs and the module completes exactly as
/// `expected` says.
async fn admission(html: &str, label: &str, policy: &str, expected: (bool, bool)) {
    let (runs, completes) = expected;
    let mounted = mount(html, false, Host::Policy(policy)).await;
    let s = mounted.inspect().await;
    let (runtime, started) = (
        get(&s, "runtime").is_truthy(),
        get(&s, "started").is_truthy(),
    );
    equal(
        &runtime.into(),
        &runs.into(),
        &format!("{label} provider admission"),
    );
    equal(
        &started.into(),
        &completes.into(),
        &format!("{label} module admission"),
    );
    if completes {
        equal(
            &get(&s, "value"),
            &USER_RAW.into(),
            &format!("{label} completion"),
        );
    }
    web_sys::console::log_1(&format!("POLICY {label}: provider={runtime} module={started}").into());
    mounted.host.remove();
}

/// Only hash-admitted scripts run under a host policy, and Trusted Types keep completion.
async fn policies(html: &str, tt_html: &str, provider: &str, module: &str) {
    let mut hashes = Vec::new();
    for text in [BUNDLE, provider, module] {
        hashes.push(digest(text).await);
    }
    let encoded = |text: &str| {
        text.replace('&', "&amp;")
            .replace('<', "&lt;")
            .replace('\r', "&#13;")
    };
    let markup = vec![
        hashes[0].clone(),
        digest(&encoded(provider)).await,
        digest(&encoded(module)).await,
    ];
    let wasm = "'wasm-unsafe-eval'";
    let scenarios = [
        ("all", wasm, hashes.clone(), true, true),
        ("missing module", wasm, hashes[..2].to_vec(), true, false),
        // The admitted module reads the runtime's outcome observer: the one designated page error.
        (
            "missing runtime",
            wasm,
            vec![hashes[1].clone(), hashes[2].clone()],
            false,
            false,
        ),
        ("encoded markup", wasm, markup, false, false),
        (
            "unmatched",
            wasm,
            vec![digest("unrelated").await],
            false,
            false,
        ),
    ];
    for (label, permission, allowed, runs, completes) in scenarios {
        let policy = format!("script-src 'self' {permission} {}", allowed.join(" "));
        admission(html, label, &policy, (runs, completes)).await;
    }
    refused_compilation(html, &hashes).await;
    trusted_types(html, tt_html).await;
}

/// Without the WebAssembly permission the bundle cannot compile its module, so nothing installs.
/// An admitted observer records the refused compilation and the module's failure.
async fn refused_compilation(html: &str, hashes: &[String]) {
    let observed = html.replacen("<head>", &format!("<head><script>{OBSERVER}</script>"), 1);
    let mut allowed = hashes.to_vec();
    allowed.push(digest(OBSERVER).await);
    let policy = format!("script-src 'self' {}", allowed.join(" "));
    admission(
        &observed,
        "missing wasm permission",
        &policy,
        (false, false),
    )
    .await;
}

/// Trusted Types keep completion, inherited with an HTML-only default and set inside the frame.
async fn trusted_types(html: &str, tt_html: &str) {
    let trusted =
        "script-src 'self' 'unsafe-inline' 'wasm-unsafe-eval'; require-trusted-types-for 'script'";
    let mounted = mount(html, false, Host::TrustedTypes(trusted)).await;
    equal(
        &get(&mounted.inspect().await, "value"),
        &USER_RAW.into(),
        "inherited Trusted Types with TrustedHTML-only default",
    );
    mounted.host.remove();
    // A provider installs this policy synchronously, before the parser reaches the module.
    let mounted = mount(tt_html, false, Host::Plain).await;
    equal(
        &get(&mounted.inspect().await, "value"),
        &USER_RAW.into(),
        "in-frame Trusted Types none",
    );
    mounted.host.remove();
}

/// Record the copied frame's external routes into `routes`, settling with its click report.
fn link_routes(mounted: &Mounted, routes: &js_sys::Array) -> js_sys::Promise {
    let window = JsValue::from(mounted.host.content_window().unwrap());
    let (frame, opened) = (mounted.frame.clone(), routes.clone());
    js_sys::Promise::new(&mut |resolve, _| {
        let (frame, opened) = (frame.clone(), opened.clone());
        let listener =
            Closure::<dyn Fn(web_sys::MessageEvent)>::new(move |event: web_sys::MessageEvent| {
                let data = event.data();
                let own =
                    event.source().map(JsValue::from) == frame.content_window().map(JsValue::from);
                let kind = if own && data.is_object() {
                    get(&data, "type").as_string()
                } else {
                    None
                };
                match kind.as_deref() {
                    Some("open-external-url") => {
                        opened.push(&get(&data, "url"));
                    }
                    Some("pointer-done") => {
                        resolve.call1(&JsValue::UNDEFINED, &data).unwrap();
                    }
                    _ => {}
                }
            })
            .into_js_value();
        invoke(&window, "addEventListener", &[&"message".into(), &listener]).unwrap();
    })
}

/// One link case: the driver's click routes once to `target`, or nowhere, without navigating.
async fn link_case(mounted: &Mounted, name: &str, target: Option<&str>) {
    let routes = js_sys::Array::new();
    let clicked = link_routes(mounted, &routes);
    inspect(&mounted.probe.id(), "click").await;
    let clicked = settle(clicked.into()).await.unwrap();
    let expected: js_sys::Array = target
        .map(|target| JsValue::from(format!("https://example.org/links/{target}")))
        .into_iter()
        .collect();
    equal(&routes, &expected, &format!("{name} routes once"));
    equal(
        &get(&clicked, "prevented"),
        &target.is_some().into(),
        &format!("{name} native prevention"),
    );
    equal(
        &get(&mounted.inspect().await, "href"),
        &"about:srcdoc".into(),
        &format!("{name} frame does not navigate"),
    );
    mounted.host.remove();
}
