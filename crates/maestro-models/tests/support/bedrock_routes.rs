//! Requests sent by the default client to loopback origins and proxies, from a child process.

use std::net::TcpListener;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use maestro_models::StreamOptions;
use maestro_models::providers::cloud::amazon_bedrock::{BedrockOptions, stream_bedrock};

use crate::bedrock;
use crate::chat::{TestResult, block_on};
use crate::loopback::{Loopback, Step, serve};
use crate::transport;

const HEAD: &str = "HTTP/1.1 200 OK\r\ncontent-type: application/vnd.amazon.eventstream\r\nconnection: close\r\n\r\n";
const PATH: &str = "/model/meta.llama-controlled/converse-stream";

/// A loopback address that either answers one request or must stay untouched.
enum Peer {
    Serving(Loopback),
    Idle(TcpListener),
}

impl Peer {
    fn serving() -> TestResult<Self> {
        let frames = bedrock::answer("answer")?.concat();
        Ok(Self::Serving(serve(HEAD, Step::writes(vec![frames]))?))
    }

    fn redirecting(target: &str) -> TestResult<Self> {
        let head = format!(
            "HTTP/1.1 302 Found\r\nlocation: {target}{PATH}\r\ncontent-length: 0\r\nconnection: close\r\n\r\n"
        );
        Ok(Self::Serving(serve(&head, Vec::new())?))
    }

    fn url(&self) -> TestResult<String> {
        Ok(match self {
            Self::Serving(server) => server.url.clone(),
            Self::Idle(listener) => format!("http://{}", listener.local_addr()?),
        })
    }

    /// The request line the peer received; an idle peer must have received no connection.
    fn request_line(self) -> TestResult<Option<String>> {
        match self {
            Self::Serving(server) => Ok(Some(server.finish()?.request_line)),
            Self::Idle(listener) => {
                listener.set_nonblocking(true)?;
                match listener.accept() {
                    Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => Ok(None),
                    _ => Err("an idle peer was contacted".into()),
                }
            }
        }
    }
}

/// How a case chooses its route.
pub struct Route {
    pub id: &'static str,
    /// Environment variables; `PROXY` in a value stands for the proxy URL.
    pub env: &'static [(&'static str, &'static str)],
    /// Whether the proxy, rather than the origin, must receive the request.
    pub proxied: bool,
}

/// Send one conversation to the origin the environment names; with `MAESTRO_REFUSED` set the
/// conversation must end in an error without the response hook having run.
fn converse() -> TestResult {
    block_on(false, async {
        let origin = std::env::var("MAESTRO_ORIGIN")?;
        let refused = std::env::var_os("MAESTRO_REFUSED").is_some();
        let hooked = Arc::new(AtomicBool::new(false));
        let seen = Arc::clone(&hooked);
        let options = BedrockOptions {
            common: StreamOptions::<maestro_models::Threaded> {
                on_response: Some(Arc::new(move |_, _| {
                    seen.store(true, Ordering::SeqCst);
                    Box::pin(async { Ok(()) })
                })),
                ..StreamOptions::default()
            },
            bearer_token: Some("fixture-token".into()),
            ..BedrockOptions::default()
        };
        let stream = stream_bedrock(
            bedrock::model(&origin)?,
            bedrock::context("question")?,
            Some(options),
        );
        let (labels, message) = transport::drain(&stream).await?;
        if refused {
            assert!(message.error_message.is_some());
            assert_eq!(labels.last().map(String::as_str), Some("error"));
            assert!(!hooked.load(Ordering::SeqCst), "the response hook ran");
        } else {
            assert_eq!(message.error_message, None);
            assert_eq!(labels.last().map(String::as_str), Some("done"));
        }
        Ok(())
    })
}

/// Run each route in a child and check which loopback peer it reached.
pub fn run_routes(test: &str, routes: &[Route]) -> TestResult {
    if std::env::var("MAESTRO_CHILD_CASE").ok().is_some() {
        return converse();
    }
    for route in routes {
        let (origin, proxy) = if route.proxied {
            (
                Peer::Idle(TcpListener::bind("127.0.0.1:0")?),
                Peer::serving()?,
            )
        } else {
            (
                Peer::serving()?,
                Peer::Idle(TcpListener::bind("127.0.0.1:0")?),
            )
        };
        let (origin_url, proxy_url) = (origin.url()?, proxy.url()?);
        let vars: Vec<(String, String)> = route
            .env
            .iter()
            .map(|(name, value)| ((*name).to_owned(), value.replace("PROXY", &proxy_url)))
            .chain([("MAESTRO_ORIGIN".to_owned(), origin_url.clone())])
            .collect();
        let vars: Vec<(&str, &str)> = vars.iter().map(|(k, v)| (k.as_str(), v.as_str())).collect();
        {
            let output = maestro_test_process::TestProcess::current(test)?
                .nocapture()
                .env_clear()
                .envs(vars.iter().copied())
                .env("MAESTRO_CHILD_CASE", route.id)
                .output()?;
            assert!(output.status.success(), "{output:?}");
        };
        let (reached, untouched) = if route.proxied {
            (proxy, origin)
        } else {
            (origin, proxy)
        };
        let line = reached
            .request_line()?
            .ok_or("the expected peer was not contacted")?;
        let expected = if route.proxied {
            format!("POST {origin_url}{PATH} HTTP/1.1")
        } else {
            format!("POST {PATH} HTTP/1.1")
        };
        assert_eq!(line, expected, "{}", route.id);
        assert_eq!(untouched.request_line()?, None, "{}", route.id);
    }
    Ok(())
}

/// A request that must fail before anything beyond the first peer is contacted.
pub struct Refusal {
    pub id: &'static str,
    /// Environment variables; `BAIT` in a value stands for the URL of the second peer.
    pub env: &'static [(&'static str, &'static str)],
    /// Whether the origin answers with a redirect to the second peer rather than staying idle.
    pub redirects: bool,
}

/// Run each refusal in a child; the origin sees a request only when it redirects, and the
/// second peer is never contacted.
pub fn run_refusals(test: &str, refusals: &[Refusal]) -> TestResult {
    if std::env::var("MAESTRO_CHILD_CASE").ok().is_some() {
        return converse();
    }
    for refusal in refusals {
        let bait = Peer::Idle(TcpListener::bind("127.0.0.1:0")?);
        let origin = if refusal.redirects {
            Peer::redirecting(&bait.url()?)?
        } else {
            Peer::Idle(TcpListener::bind("127.0.0.1:0")?)
        };
        let (origin_url, bait_url) = (origin.url()?, bait.url()?);
        let vars: Vec<(String, String)> = refusal
            .env
            .iter()
            .map(|(name, value)| ((*name).to_owned(), value.replace("BAIT", &bait_url)))
            .chain([
                ("MAESTRO_ORIGIN".to_owned(), origin_url),
                ("MAESTRO_REFUSED".to_owned(), "1".to_owned()),
            ])
            .collect();
        let vars: Vec<(&str, &str)> = vars.iter().map(|(k, v)| (k.as_str(), v.as_str())).collect();
        {
            let output = maestro_test_process::TestProcess::current(test)?
                .nocapture()
                .env_clear()
                .envs(vars.iter().copied())
                .env("MAESTRO_CHILD_CASE", refusal.id)
                .output()?;
            assert!(output.status.success(), "{output:?}");
        };
        let line = origin.request_line()?;
        let expected = refusal.redirects.then(|| format!("POST {PATH} HTTP/1.1"));
        assert_eq!(line, expected, "{}", refusal.id);
        assert_eq!(bait.request_line()?, None, "{}", refusal.id);
    }
    Ok(())
}
