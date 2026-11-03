//! Digest-checked report assets and request confinement.

use super::page::{Page, string};
use base64::{Engine, engine::general_purpose::STANDARD};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{collections::HashMap, io, path::Path};

/// One exact cached report library.
struct Asset {
    /// URL authored by the report.
    url: &'static str,
    /// Exact source acquired by the explicit qualification command.
    source: &'static str,
    /// Cache leaf name.
    file: &'static str,
    /// Expected SHA-256 digest.
    digest: &'static str,
}

/// The report's two pinned external assets.
const ASSETS: [Asset; 2] = [
    Asset {
        url: "https://cdn.jsdelivr.net/npm/@tailwindcss/browser@4",
        source: "https://cdn.jsdelivr.net/npm/@tailwindcss/browser@4.3.3",
        file: "tailwind-browser-4.3.3.js",
        digest: "6d8c473ef2f8ad63feafc0bd76502dda31501a6c135dc4c6173f6268cde595be",
    },
    Asset {
        url: "https://cdn.jsdelivr.net/npm/chart.js@4.4.9/dist/chart.umd.min.js",
        source: "https://cdn.jsdelivr.net/npm/chart.js@4.4.9/dist/chart.umd.min.js",
        file: "chart-4.4.9.umd.min.js",
        digest: "bce154080959c574be0bb6b1a924ff32f08ebc6ff460c159171f51c53802c844",
    },
];

/// Verified asset bodies and the one admitted loopback origin.
pub(super) struct Cache {
    /// Bytes indexed by the exact authored URL.
    bodies: HashMap<String, Vec<u8>>,
    /// Navigation origin; the slash boundary is required when matching a request.
    origin: String,
    /// Every attempted request outside the admitted origin and cache.
    pub escaped: Vec<String>,
}

/// Load verified assets before any page request is admitted.
pub(super) async fn load(directory: &Path, origin: &str) -> io::Result<Cache> {
    tokio::fs::create_dir_all(directory).await?;
    let client = reqwest::Client::new();
    let mut bodies = HashMap::new();
    for asset in &ASSETS {
        let file = directory.join(asset.file);
        if !tokio::fs::try_exists(&file).await? {
            let bytes = download(asset, &client).await?;
            tokio::fs::write(&file, bytes).await?;
        }
        let bytes = tokio::fs::read(file).await?;
        verify(&bytes, asset)?;
        bodies.insert(asset.url.to_owned(), bytes);
    }
    Ok(Cache {
        bodies,
        origin: format!("{origin}/"),
        escaped: Vec::new(),
    })
}

/// Refuse stale or corrupt cache entries without executing them.
fn verify(bytes: &[u8], asset: &Asset) -> io::Result<()> {
    let actual = maestro_tooling::hex(&Sha256::digest(bytes));
    if actual != asset.digest {
        return Err(io::Error::other(format!(
            "{} digest {actual} does not match its pin",
            asset.file
        )));
    }
    Ok(())
}

/// Retain the existing three-attempt acquisition budget without timer-based waits.
async fn download(asset: &Asset, client: &reqwest::Client) -> io::Result<Vec<u8>> {
    let mut last = io::Error::other("asset acquisition failed");
    for _ in 0..3 {
        let result = async {
            let response = client
                .get(asset.source)
                .send()
                .await
                .map_err(io::Error::other)?
                .error_for_status()
                .map_err(io::Error::other)?;
            let bytes = response.bytes().await.map_err(io::Error::other)?.to_vec();
            verify(&bytes, asset)?;
            Ok(bytes)
        }
        .await;
        match result {
            Ok(bytes) => return Ok(bytes),
            Err(error) => last = error,
        }
    }
    Err(io::Error::other(format!(
        "cannot fetch {}: {last}",
        asset.source
    )))
}

#[cfg(test)]
mod tests;

impl Page<'_> {
    /// Answer exact cached URLs and abort every request outside the admitted loopback origin.
    pub(super) async fn route(&mut self, params: &Value) -> io::Result<()> {
        let cache = self
            .cache
            .as_mut()
            .ok_or_else(|| io::Error::other("request interception without verified assets"))?;
        let url = string(&params["request"], "url")?;
        let id = string(params, "requestId")?;
        let (method, params) = if let Some(bytes) = cache.bodies.get(url) {
            (
                "Fetch.fulfillRequest",
                json!({"requestId":id,"responseCode":200,"responseHeaders":[{"name":"Content-Type","value":"application/javascript; charset=utf-8"}],"body":STANDARD.encode(bytes)}),
            )
        } else if url.starts_with(&cache.origin) {
            ("Fetch.continueRequest", json!({"requestId":id}))
        } else {
            cache.escaped.push(url.to_owned());
            (
                "Fetch.failRequest",
                json!({"requestId":id,"errorReason":"Aborted"}),
            )
        };
        let session = self.session.clone();
        self.command(Some(&session), method, params).await?;
        Ok(())
    }
}
