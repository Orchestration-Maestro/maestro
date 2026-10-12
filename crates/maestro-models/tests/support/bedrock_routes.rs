//! Requests sent by the default client to loopback origins and proxies, from a child process.

use std::net::TcpListener;

use maestro_models::StreamOptions;
use maestro_models::providers::cloud::amazon_bedrock::{BedrockOptions, stream_bedrock};

use crate::bedrock;
use crate::chat::{TestResult, block_on};
use crate::child_process;
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

/// Send one conversation to the origin the environment names.
fn converse() -> TestResult {
    block_on(false, async {
        let origin = std::env::var("MAESTRO_ORIGIN")?;
        let options = BedrockOptions {
            common: StreamOptions::default(),
            bearer_token: Some("fixture-token".into()),
            ..BedrockOptions::default()
        };
        let stream = stream_bedrock(
            bedrock::model(&origin)?,
            bedrock::context("question")?,
            Some(options),
        );
        let (labels, message) = transport::drain(&stream).await?;
        assert_eq!(message.error_message, None);
        assert_eq!(labels.last().map(String::as_str), Some("done"));
        Ok(())
    })
}

/// Run each route in a child and check which loopback peer it reached.
pub fn run_routes(test: &str, routes: &[Route]) -> TestResult {
    if child_process::child_case().is_some() {
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
        child_process::rerun(test, route.id, &vars)?;
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
