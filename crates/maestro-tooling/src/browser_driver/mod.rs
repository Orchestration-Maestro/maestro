//! Browser export qualification with the supplied pinned Chromium.
//!
//! The development command owns preparation and cleanup. Browser operations use one
//! low-level library connection, with default-world bindings routed by renderer session.

use connection::Dispatcher;
use launch::Process;
use package::{Work, command};
use serde_json::{Value, json};
use std::{
    ffi::OsString,
    fs, io,
    path::{Path, PathBuf},
    process::{Command, ExitCode},
};

/// Verified report assets and request confinement.
mod assets;
/// Native assertion bindings.
mod bindings;
/// Browser message correlation.
mod connection;
/// Browser export inventory and compiler artifacts.
mod inputs;
/// Library-owned browser launch and cleanup.
mod launch;
/// Locked native preparation.
mod package;
/// Native page and frame operations.
mod page;
/// Committed report painting observations.
mod report;
/// Browser-native painting and completion expressions.
mod report_scripts;
/// Canonical packaged resource routing.
mod resources;
/// Browser-native observation expressions.
mod scripts;
/// Ephemeral fixture HTTP transport.
mod server;

pub use page::Page;
pub use resources::resource_path;
pub use server::Server;

/// One browser invocation, retaining its sole CDP connection until cleanup.
pub struct Browser {
    /// Browser process and diagnostics.
    process: Process,
    /// Session-aware transport shared by successive fresh pages.
    dispatcher: Dispatcher,
    /// Ordered output from assertions and completion phases.
    lines: Vec<String>,
}

impl Browser {
    /// Launch the explicit executable with an invocation-owned profile.
    ///
    /// Extra arguments retain the browser input's native renderer configurations.
    ///
    /// # Errors
    /// Returns launch, readiness or CDP connection failures.
    pub async fn launch(executable: &Path, profile: &Path, args: &[String]) -> io::Result<Self> {
        let (process, endpoint) = Process::launch(executable, profile, args).await?;
        match Dispatcher::connect(&endpoint).await {
            Ok(dispatcher) => Ok(Self {
                process,
                dispatcher,
                lines: Vec::new(),
            }),
            Err(error) => cleanup(Err(error), process.close().await),
        }
    }

    /// Open a fresh page whose explicit close settles renderer cleanup.
    ///
    /// # Errors
    /// Returns target creation or renderer initialization failures.
    pub async fn page(&mut self, name: &str) -> io::Result<Page<'_>> {
        Page::open(&mut self.dispatcher, name, &mut self.lines).await
    }

    /// Execute one assertion with fresh page/module state and exact policy-error accounting.
    ///
    /// # Errors
    /// Returns navigation, assertion, page/crash/disconnect or cleanup failures.
    pub async fn check(&mut self, url: &str, name: &str, expression: &str) -> io::Result<Value> {
        let mut page = self.page(name).await?;
        let result = async {
            page.initialize().await?;
            page.navigate(url).await?;
            let value = page.evaluate(expression).await?;
            page.phase("export resolved");
            Ok(value)
        }
        .await;
        let closed = page.close().await;
        cleanup(result, closed)
    }

    /// Require every selected export to be callable before invoking any assertion.
    ///
    /// # Errors
    /// Returns initialization failures or the complete missing/non-callable selection.
    pub async fn preflight(&mut self, origin: &str, names: &[String]) -> io::Result<()> {
        let expression = format!(
            "(async () => {{ const module = await import('/sandbox.js'); await module.default(); return {}.filter(name => typeof module[name] !== 'function'); }})()",
            json!(names)
        );
        let missing = self
            .check(&format!("{origin}/"), "inventory", &expression)
            .await?;
        if missing != json!([]) {
            return Err(io::Error::other(format!(
                "missing/non-callable exports: {missing}"
            )));
        }
        Ok(())
    }

    /// Ordered observations emitted by completed and failed assertions.
    #[must_use]
    pub fn lines(&self) -> &[String] {
        &self.lines
    }

    /// Close and reap the library-launched child, including failed-page outcomes.
    ///
    /// # Errors
    /// Returns process termination, reaping or diagnostic-drain failures.
    pub async fn close(mut self) -> io::Result<()> {
        match self.execute("Browser.close", json!({})).await {
            Ok(_) => self.process.wait().await,
            Err(error) => cleanup(Err(error), self.process.close().await),
        }
    }

    /// Execute a browser-scoped protocol operation through the retained connection.
    ///
    /// # Errors
    /// Returns browser protocol or disconnection failures.
    pub async fn execute(&mut self, method: &'static str, params: Value) -> io::Result<Value> {
        page::raw(&mut self.dispatcher, None, method, params).await
    }

    /// Emit family accounting alongside the page's diagnostic phases.
    fn log(&mut self, text: &str) {
        println!("{text}");
        self.lines.push(text.to_owned());
    }
}

/// Dispatch the full browser command on the binary's runtime.
///
/// # Errors
/// Returns tool-contract, preparation, browser assertion and cleanup failures.
pub async fn run(args: &[OsString], cargo: &Path, checkout: &Path) -> io::Result<ExitCode> {
    if args.len() > 1 {
        return Err(io::Error::other(
            "browser-test accepts at most one report HTML path",
        ));
    }
    let fixture = args.first().map(PathBuf::from);
    let executable = std::env::var_os("CHROMIUM_PATH")
        .map(PathBuf::from)
        .ok_or_else(|| io::Error::other("CHROMIUM_PATH must be an absolute executable file"))?;
    let checkout = checkout.to_owned();
    let cargo = cargo.to_owned();
    let build_executable = executable.clone();
    let build_checkout = checkout.clone();
    let (work, root, inventory) =
        tokio::task::spawn_blocking(move || prepare(&build_checkout, &cargo, &build_executable))
            .await
            .map_err(io::Error::other)??;
    let profile = work.path.join("chromium");
    let result = async {
        exports(&root, &executable, &profile, &inventory).await?;
        let target = work
            .path
            .parent()
            .ok_or_else(|| io::Error::other("browser target directory missing"))?;
        report::run(
            &checkout,
            &executable,
            target,
            &work.path,
            fixture.as_deref(),
        )
        .await
    }
    .await;
    cleanup(result, tokio::fs::remove_dir_all(&work.path).await)?;
    Ok(ExitCode::SUCCESS)
}

/// Validate and build before starting a listener or a browser process.
fn prepare(checkout: &Path, cargo: &Path, executable: &Path) -> io::Result<(Work, PathBuf, Value)> {
    package::tools(executable, checkout)?;
    println!(
        "CHECKOUT {}",
        command(
            Command::new("git")
                .current_dir(checkout)
                .args(["rev-parse", "HEAD"])
        )?
        .trim()
    );
    let inventory = inputs::inventory(&fs::read_to_string(
        checkout.join("crates/maestro-web/tests/sandbox_loading_support/exports.json"),
    )?)?;
    let target = crate::locked_metadata(cargo, checkout)?
        .target_directory
        .into_std_path_buf();
    let work = Work::new(&target)?;
    let root = package::build(checkout, cargo, &target, &work.path)?;
    Ok((work, root, inventory))
}

/// Run the complete inventory in fresh pages and prove host-byte stability.
async fn exports(
    root: &Path,
    executable: &Path,
    profile: &Path,
    inventory: &Value,
) -> io::Result<()> {
    let server = server::Server::start(root).await?;
    let result = async {
        let mut browser = Browser::launch(executable, profile, &[]).await?;
        let result = run_inventory(&mut browser, root, &server.origin, inventory).await;
        cleanup(result, browser.close().await)
    }
    .await;
    cleanup(result, server.close().await)
}

/// Preflight every callable export before invoking the first assertion.
async fn run_inventory(
    browser: &mut Browser,
    root: &Path,
    origin: &str,
    inventory: &Value,
) -> io::Result<()> {
    let names: Vec<_> = ["execution", "loading", "contributions"]
        .into_iter()
        .flat_map(|family| inventory[family].as_array().into_iter().flatten())
        .collect();
    let names: Vec<String> = serde_json::from_value(json!(names)).map_err(io::Error::other)?;
    let before = host_digest(root).await?;
    browser.preflight(origin, &names).await?;
    for family in ["execution", "loading", "contributions"] {
        let names = inventory[family]
            .as_array()
            .ok_or_else(|| io::Error::other("invalid export family"))?;
        for name in names {
            let name = name
                .as_str()
                .ok_or_else(|| io::Error::other("invalid export name"))?;
            browser.log(&format!("START {family} {name}"));
            let expression = format!(
                "(async () => {{ const module = await import('/sandbox.js'); await module.default(); await module[{}](url => import(url)); }})()",
                json!(name)
            );
            let entry = if family == "contributions" {
                "/contributions.html"
            } else {
                "/"
            };
            browser
                .check(&format!("{origin}{entry}"), name, &expression)
                .await
                .map_err(|error| io::Error::other(format!("FAIL {name}: {error}")))?;
            browser.log(&format!("PASS {family} {name}"));
        }
        browser.log(&format!("FAMILY {family} {0}/{0}", names.len()));
    }
    if host_digest(root).await? != before {
        return Err(io::Error::other("host bundle changed while modules loaded"));
    }
    browser.log(&format!("HOST {before} unchanged"));
    browser.log(&format!("COMPLETE {0}/{0}", names.len()));
    Ok(())
}

/// Keep filesystem hashing off the browser event loop.
async fn host_digest(root: &Path) -> io::Result<String> {
    let root = root.to_owned();
    tokio::task::spawn_blocking(move || resources::digest(&root))
        .await
        .map_err(io::Error::other)?
}

/// Preserve the original failure while still reporting an independent cleanup failure.
fn cleanup<T>(result: io::Result<T>, closed: io::Result<()>) -> io::Result<T> {
    match result {
        Ok(value) => closed.map(|()| value),
        Err(error) => {
            if let Err(closed) = closed {
                eprintln!("browser cleanup: {closed}");
            }
            Err(error)
        }
    }
}

#[cfg(test)]
mod tests;
