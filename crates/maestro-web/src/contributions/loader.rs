//! Separately built browser modules initialized and registered at runtime.
use super::{
    BrowserContribution, BrowserModule, ContributionFailure, ContributionRegistration,
    adapters::FrameView,
    host::{Callables, Invocation, interface, release},
};
use crate::dom::DomView;
use js_sys::{Function, Object, Promise, Reflect};
use maestro_app::{CustomAgentMessages, NoCustomMessages, SandboxUrlProvider};
use std::{cell::RefCell, marker::PhantomData, rc::Rc};
use wasm_bindgen::{JsCast as _, JsValue};
use wasm_bindgen_futures::JsFuture;

/// Page host for built-in elements and runtime module contributions.
pub struct ContributionHost<C: CustomAgentMessages + 'static = NoCustomMessages> {
    /// Isolated-frame document URL selection; absence selects inline documents.
    sandbox_url: Option<SandboxUrlProvider>,
    /// The embedding page's dynamic module import.
    import_module: Function,
    /// Message representation of the host application.
    messages: PhantomData<C>,
}
/// Registrations acquired by one load; dropping it releases those still owned.
pub struct ModuleRegistrations<C: CustomAgentMessages + 'static = NoCustomMessages> {
    /// Isolated-frame contributions registered by the load itself.
    frames: Vec<ContributionRegistration>,
    /// Initialized module invocations, which may still register from their callbacks.
    modules: Vec<(Rc<RefCell<Invocation<C>>>, Callables)>,
}
impl<C: CustomAgentMessages + 'static> Drop for ModuleRegistrations<C> {
    fn drop(&mut self) {
        for frame in std::mem::take(&mut self.frames) {
            // A later replacement owns the entry when this returns false.
            let _ = frame.unregister();
        }
        for (invocation, callables) in std::mem::take(&mut self.modules) {
            // Releasing runs module callbacks; with the interface gone they cannot register again.
            drop(callables);
            let owned = std::mem::take(&mut invocation.borrow_mut().owned);
            release(owned);
        }
    }
}
/// Modules in first-declaration order with the contributions each declares.
type Modules<'a> = Vec<(&'a BrowserModule, Vec<BrowserContribution>)>;
impl<C: CustomAgentMessages + serde::Serialize + 'static> ContributionHost<C> {
    /// Define the built-in elements and install their stylesheet once for the page.
    ///
    /// `import_module` is the embedding page's `(url) => import(url)`. The page that
    /// supplies it is trusted: the host calls it only with module URLs below the
    /// package base it is given, and the imported modules run with page authority.
    /// # Errors
    /// Returns a browser registration failure.
    pub fn new(
        view: &Rc<DomView>,
        sandbox_url: Option<SandboxUrlProvider>,
        import_module: Function,
    ) -> Result<Self, JsValue> {
        super::register_builtin_elements(view)?;
        Ok(Self {
            sandbox_url,
            import_module,
            messages: PhantomData,
        })
    }
    /// Load enabled contributions whose assets are served below `base_url`.
    ///
    /// `base_url` names the package directory and ends with `/`. Each module
    /// initializes once and registers the contributions it declares; a document
    /// contribution mounts in an isolated frame. Contributions with neither are
    /// left to built-in templates.
    pub async fn load(
        &self,
        contributions: &[BrowserContribution],
        base_url: &str,
    ) -> (ModuleRegistrations<C>, Vec<ContributionFailure>) {
        let mut registrations = ModuleRegistrations {
            frames: Vec::new(),
            modules: Vec::new(),
        };
        let mut failures = Vec::new();
        let mut modules: Modules<'_> = Vec::new();
        for contribution in contributions {
            let descriptor = &contribution.descriptor;
            match (&descriptor.module, &descriptor.document) {
                (Some(module), document) => {
                    failures.extend(document.iter().map(|document| ignored(document)));
                    match modules.iter_mut().find(|(known, _)| *known == module) {
                        Some((_, declared)) => declared.push(contribution.clone()),
                        None => modules.push((module, vec![contribution.clone()])),
                    }
                }
                (None, Some(document)) => {
                    match self.frame(contribution, document, base_url).await {
                        Ok(owned) => registrations.frames.push(owned),
                        Err(failure) => failures.push(failure),
                    }
                }
                (None, None) => {}
            }
        }
        for (module, declared) in modules {
            match self.initialize(module, declared, base_url).await {
                Ok(initialized) => registrations.modules.push(initialized),
                Err(failure) => failures.push(failure),
            }
        }
        (registrations, failures)
    }
    /// Register a document contribution shown in an isolated frame.
    async fn frame(
        &self,
        contribution: &BrowserContribution,
        document: &str,
        base_url: &str,
    ) -> Result<ContributionRegistration, ContributionFailure> {
        let failed = |cause| ContributionFailure::Initialization {
            entry: document.to_owned(),
            cause,
        };
        let text = fetch_text(&asset(base_url, document).map_err(failed)?)
            .await
            .map_err(failed)?;
        let view = FrameView {
            id: contribution.descriptor.id.clone(),
            document: text,
            url: self.sandbox_url.clone(),
        };
        Ok(super::register_contribution(
            contribution.clone(),
            Rc::new(view),
        ))
    }
    /// Initialize one module, releasing what it acquired when initialization fails.
    async fn initialize(
        &self,
        module: &BrowserModule,
        declared: Vec<BrowserContribution>,
        base_url: &str,
    ) -> Result<(Rc<RefCell<Invocation<C>>>, Callables), ContributionFailure> {
        let invocation = Rc::new(RefCell::new(Invocation {
            declared,
            owned: Vec::new(),
        }));
        match self.invoke(module, base_url, &invocation).await {
            Ok(callables) => Ok((invocation, callables)),
            Err(cause) => {
                let acquired = std::mem::take(&mut invocation.borrow_mut().owned);
                release(acquired);
                Err(ContributionFailure::Initialization {
                    entry: module.entry.clone(),
                    cause,
                })
            }
        }
    }
    /// Import one module, start its WebAssembly and call its registration export.
    async fn invoke(
        &self,
        module: &BrowserModule,
        base_url: &str,
        invocation: &Rc<RefCell<Invocation<C>>>,
    ) -> Result<Callables, String> {
        let entry = asset(base_url, &module.entry)?;
        let wasm = asset(base_url, &module.wasm)?;
        let imported = self
            .import_module
            .call1(&JsValue::NULL, &entry.into())
            .map_err(|error| describe(&error))?;
        let exports = settle(&imported).await?;
        let options = Object::new();
        Reflect::set(&options, &"module_or_path".into(), &wasm.into())
            .map_err(|error| describe(&error))?;
        settle(
            &export(&exports, "default")?
                .call1(&JsValue::NULL, &options)
                .map_err(|error| describe(&error))?,
        )
        .await?;
        let (object, callables) = interface(invocation).map_err(|error| describe(&error))?;
        let returned = export(&exports, &module.register)?
            .call1(&JsValue::NULL, &object)
            .map_err(|error| describe(&error))?;
        settle(&returned).await?;
        Ok(callables)
    }
}
/// The failure reported for a document a module contribution also declares.
fn ignored(document: &str) -> ContributionFailure {
    ContributionFailure::Initialization {
        entry: document.to_owned(),
        cause: "ignored, the module supplies this contribution".into(),
    }
}
/// Resolve a package-relative asset, refusing any URL outside the `base_url` directory.
fn asset(base_url: &str, path: &str) -> Result<String, String> {
    let base = url::Url::parse(base_url).map_err(|error| error.to_string())?;
    // Without its trailing slash a base would admit siblings sharing its prefix.
    if !base.path().ends_with('/') {
        return Err(format!("{base} does not name a package directory"));
    }
    let resolved = base.join(path).map_err(|error| error.to_string())?;
    if resolved.as_str().starts_with(base.as_str()) {
        Ok(resolved.into())
    } else {
        Err(format!("{resolved} is outside {base}"))
    }
}
/// A named function export of an imported module.
fn export(exports: &JsValue, name: &str) -> Result<Function, String> {
    Reflect::get(exports, &name.into())
        .map_err(|error| describe(&error))?
        .dyn_into()
        .map_err(|_| format!("missing export {name}"))
}
/// Await a returned promise, or take a plain value as already settled.
async fn settle(value: &JsValue) -> Result<JsValue, String> {
    JsFuture::from(Promise::resolve(value))
        .await
        .map_err(|error| describe(&error))
}
/// The browser's description of a failure: an error's name and message, or a thrown string.
fn describe(error: &JsValue) -> String {
    error
        .dyn_ref::<js_sys::Error>()
        .map(|error| String::from(error.to_string()))
        .or_else(|| error.as_string())
        .unwrap_or_else(|| format!("{error:?}"))
}
/// Fetch a same-origin text asset.
async fn fetch_text(url: &str) -> Result<String, String> {
    let window = web_sys::window().ok_or("No window")?;
    let response: web_sys::Response = JsFuture::from(window.fetch_with_str(url))
        .await
        .map_err(|error| describe(&error))?
        .dyn_into()
        .map_err(|error| describe(&error))?;
    if !response.ok() {
        return Err(format!("HTTP {}", response.status()));
    }
    let text = JsFuture::from(response.text().map_err(|error| describe(&error))?)
        .await
        .map_err(|error| describe(&error))?;
    text.as_string().ok_or_else(|| "Not text".into())
}
