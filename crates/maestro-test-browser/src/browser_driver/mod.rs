//! Browser export qualification with the supplied pinned Chromium.
//!
//! A nextest setup prepares the export package once; each export and the report then run
//! as their own test. Browser operations use one low-level library connection per driver,
//! with default-world bindings routed by renderer session.

use connection::Dispatcher;
use inputs::ExpectedError;
use launch::Process;
use package::command;
use serde_json::{Value, json};
use std::{
    ffi::{OsStr, OsString},
    io,
    path::{Path, PathBuf},
    process::Command,
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
/// Real screenshot observations for authored reading paint scenes.
mod paint;
/// Browser-native screenshot decoding and pixel predicates.
mod paint_scripts;
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
pub use report::run as tool_report;
pub use resources::resource_path;
pub use server::Server;

/// The export inventory, the same file the build script generates one test per export from.
const INVENTORY: &str =
    include_str!("../../../maestro-web/tests/sandbox_loading_support/exports.json");

/// The setup variable holding the digest of the prepared bundle, hashed once for the whole run.
const DIGEST_VARIABLE: &str = "MAESTRO_BROWSER_EXPORTS_DIGEST";

/// One CDP connection, owning the fresh pages it creates and its ordered diagnostics.
struct Driver {
    /// Session-aware transport shared by successive fresh pages.
    dispatcher: Dispatcher,
    /// Ordered output from assertions and completion phases.
    lines: Vec<String>,
    /// Parsed policy-error identity used by every page.
    expected_error: ExpectedError,
}

impl Driver {
    /// Connect one transport to a launched browser's advertised endpoint.
    async fn connect(endpoint: &str) -> io::Result<Self> {
        let expected_error = inputs::inventory(INVENTORY)?.expected_error;
        Ok(Self {
            dispatcher: Dispatcher::connect(endpoint).await?,
            lines: Vec::new(),
            expected_error,
        })
    }

    /// Open a fresh page whose explicit close settles renderer cleanup.
    ///
    /// # Errors
    /// Returns target creation or renderer initialization failures.
    async fn page(&mut self, name: &str) -> io::Result<Page<'_>> {
        Page::open(
            &mut self.dispatcher,
            name,
            &mut self.lines,
            &self.expected_error,
        )
        .await
    }

    /// Execute one assertion with fresh page/module state and exact policy-error accounting.
    ///
    /// # Errors
    /// Returns navigation, assertion, page/crash/disconnect or cleanup failures.
    async fn check(&mut self, url: &str, name: &str, expression: &str) -> io::Result<Value> {
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

    /// Require every function the served module exports to be one of `names`, without calling any.
    ///
    /// The bindings' initializers (`default`, `initSync`) and generated classes are not exports.
    ///
    /// # Errors
    /// Returns initialization failures or every unlisted export.
    async fn listed(&mut self, origin: &str, names: &[String]) -> io::Result<()> {
        let expression = format!(
            "(async () => {{ const module = await import('/sandbox.js'); const listed = new Set({}); return Object.keys(module).filter(name => typeof module[name] === 'function' && !Function.prototype.toString.call(module[name]).startsWith('class') && !['default', 'initSync'].includes(name) && !listed.has(name)); }})()",
            json!(names)
        );
        let unlisted = self
            .check(&format!("{origin}/"), "inventory", &expression)
            .await?;
        if unlisted != json!([]) {
            return Err(io::Error::other(format!(
                "exports missing from the inventory: {unlisted}"
            )));
        }
        Ok(())
    }

    /// Ordered observations emitted by completed and failed assertions.
    fn lines(&self) -> &[String] {
        &self.lines
    }

    /// Execute a browser-scoped protocol operation through the retained connection.
    ///
    /// # Errors
    /// Returns browser protocol or disconnection failures.
    async fn execute(&mut self, method: &'static str, params: Value) -> io::Result<Value> {
        page::raw(&mut self.dispatcher, None, method, params).await
    }
}

/// One launched browser process with its connected driver.
pub struct Browser {
    /// Browser process and diagnostics.
    process: Process,
    /// Driver serving every page of this browser.
    driver: Driver,
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
        match Driver::connect(&endpoint).await {
            Ok(driver) => Ok(Self { process, driver }),
            Err(error) => cleanup(Err(error), process.close().await),
        }
    }

    /// Open a fresh page whose explicit close settles renderer cleanup.
    ///
    /// # Errors
    /// Returns target creation or renderer initialization failures.
    pub async fn page(&mut self, name: &str) -> io::Result<Page<'_>> {
        self.driver.page(name).await
    }

    /// Execute one assertion with fresh page/module state and exact policy-error accounting.
    ///
    /// # Errors
    /// Returns navigation, assertion, page/crash/disconnect or cleanup failures.
    pub async fn check(&mut self, url: &str, name: &str, expression: &str) -> io::Result<Value> {
        self.driver.check(url, name, expression).await
    }

    /// Require every function the served module exports to be one of `names`, without calling any.
    ///
    /// # Errors
    /// Returns initialization failures or every unlisted export.
    pub async fn listed(&mut self, origin: &str, names: &[String]) -> io::Result<()> {
        self.driver.listed(origin, names).await
    }

    /// Ordered observations emitted by completed and failed assertions.
    #[must_use]
    pub fn lines(&self) -> &[String] {
        self.driver.lines()
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
        self.driver.execute(method, params).await
    }
}

/// Prepare the export package once for a nextest run and tell its browser tests where it is.
///
/// The package replaces the previous run's in Cargo's target directory, where the tests read
/// it until the next preparation.
///
/// # Errors
/// Returns argument, tool-contract, inventory, build or setup-export failures.
pub fn prepare(args: &[OsString], cargo: &Path, checkout: &Path) -> io::Result<()> {
    if !args.is_empty() {
        return Err(io::Error::other(
            "the browser export setup takes no arguments",
        ));
    }
    let executable = std::env::var_os("CHROMIUM_PATH")
        .map(PathBuf::from)
        .ok_or_else(|| io::Error::other("CHROMIUM_PATH must be an absolute executable file"))?;
    package::tools(&executable, checkout)?;
    println!(
        "CHECKOUT {}",
        command(
            Command::new("git")
                .current_dir(checkout)
                .args(["rev-parse", "HEAD"])
        )?
        .trim()
    );
    inputs::inventory(INVENTORY)?;
    let target = maestro_tooling::locked_metadata(cargo, checkout)?
        .target_directory
        .into_std_path_buf();
    let root = package::build(checkout, cargo, &target, &package::directory(&target)?)?;
    setup_variable("MAESTRO_BROWSER_EXPORTS", &root)?;
    setup_variable(DIGEST_VARIABLE, resources::digest(&root)?)?;
    setup_variable(
        "MAESTRO_BROWSER_ASSETS",
        target.join("browser-analysis-assets"),
    )
}

/// Hand one prepared location or digest to the tests the setup serves.
fn setup_variable(variable: &str, value: impl AsRef<OsStr>) -> io::Result<()> {
    let mut entry = OsString::from(format!("{variable}="));
    entry.push(value);
    maestro_tooling::nextest_setup::export(&entry)
}

/// Run one inventory export in its own browser on the prepared package, which it must leave
/// as the setup recorded it.
///
/// # Errors
/// Returns a missing recorded digest, or server, launch, assertion, cleanup or host-change
/// failures.
pub async fn run_export(
    executable: &Path,
    profile: &Path,
    root: &Path,
    family: &str,
    name: &str,
) -> io::Result<()> {
    let recorded = std::env::var(DIGEST_VARIABLE).map_err(|error| {
        io::Error::other(format!(
            "the browser export setup exports {DIGEST_VARIABLE}: {error}"
        ))
    })?;
    session(executable, profile, root, async |browser, origin| {
        run_case(&mut browser.driver, origin, family, name).await
    })
    .await?;
    require_unchanged(root, &recorded).await
}

/// Require the bundle on disk to hash to the digest the setup recorded.
async fn require_unchanged(root: &Path, recorded: &str) -> io::Result<()> {
    if host_digest(root).await? == recorded {
        Ok(())
    } else {
        Err(io::Error::other("host bundle changed while modules loaded"))
    }
}

/// Require every export the prepared package's module offers to be in the inventory.
///
/// # Errors
/// Returns server, launch, inventory or cleanup failures, or every unlisted export.
pub async fn check_inventory(executable: &Path, profile: &Path, root: &Path) -> io::Result<()> {
    let names: Vec<String> = inputs::inventory(INVENTORY)?
        .families
        .into_iter()
        .flat_map(|(_, names)| names)
        .collect();
    session(executable, profile, root, async |browser, origin| {
        browser.listed(origin, &names).await
    })
    .await
}

/// Serve a package to one fresh browser for the action, closing both on every outcome.
async fn session<T>(
    executable: &Path,
    profile: &Path,
    root: &Path,
    action: impl AsyncFnOnce(&mut Browser, &str) -> io::Result<T>,
) -> io::Result<T> {
    let server = Server::start(root).await?;
    let result = async {
        let mut browser = Browser::launch(executable, profile, &[]).await?;
        let result = action(&mut browser, &server.origin).await;
        cleanup(result, browser.close().await)
    }
    .await;
    cleanup(result, server.close().await)
}

/// Run one export in a fresh page of the driver that owns it.
async fn run_case(driver: &mut Driver, origin: &str, family: &str, name: &str) -> io::Result<()> {
    let expression = format!(
        "(async () => {{ const module = await import('/sandbox.js'); await module.default(); await module[{}](url => import(url)); }})()",
        json!(name)
    );
    let entry = match family {
        "contributions" => "/contributions.html",
        "canvas" => "/canvas.html",
        _ => "/",
    };
    driver
        .check(&format!("{origin}{entry}"), name, &expression)
        .await
        .map(|_| ())
}

/// Keep filesystem hashing off the browser event loop.
async fn host_digest(root: &Path) -> io::Result<String> {
    let root = root.to_owned();
    tokio::task::spawn_blocking(move || resources::digest(&root))
        .await
        .map_err(io::Error::other)?
}

/// Preserve the original failure while still reporting an independent cleanup failure.
///
/// # Errors
/// Returns the original failure, or the cleanup failure when the operation succeeded.
pub fn cleanup<T>(result: io::Result<T>, closed: io::Result<()>) -> io::Result<T> {
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
