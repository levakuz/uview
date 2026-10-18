//! Package indexes, read over HTTP for what uv cannot list: a package's versions. The only code
//! in uview that makes a request (ADR-0012). It reads; it never uploads or writes.

pub mod parse;

use std::fmt;
use std::time::Duration;

use crate::domain::normalize_name;

/// Why an index could not say.
#[derive(Debug)]
pub enum IndexError {
    /// No answer: no connection, a timeout, or TLS refused.
    Network(String),
    /// The index has no such project.
    NotFound,
    /// The index answered with another failure, such as 401 for a private one.
    Status(u16),
    /// The index answered, but not in a form uview reads.
    Parse(String),
}

impl fmt::Display for IndexError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            IndexError::Network(e) => write!(f, "no answer: {e}"),
            IndexError::NotFound => write!(f, "no such package"),
            IndexError::Status(401 | 403) => {
                write!(
                    f,
                    "the index wants credentials, which uview does not send yet"
                )
            }
            IndexError::Status(code) => write!(f, "the index answered {code}"),
            IndexError::Parse(e) => write!(f, "could not read the index's answer: {e}"),
        }
    }
}

impl std::error::Error for IndexError {}

/// A project's page on an index, as served: JSON (PEP 691) or HTML (PEP 503).
#[derive(Clone, Debug)]
pub struct Page {
    pub json: bool,
    pub body: String,
}

/// Reads indexes. The seam that lets everything above it be tested without a network.
pub trait IndexClient {
    /// The page of the project `name` on the index at `index`, its Simple API URL.
    fn project_page(
        &self,
        index: &str,
        name: &str,
    ) -> impl Future<Output = Result<Page, IndexError>> + Send;
}

/// Asks for JSON, and takes HTML from an index that has only that, as PEP 691 recommends.
const ACCEPT: &str = "application/vnd.pypi.simple.v1+json, \
     application/vnd.pypi.simple.v1+html;q=0.2, text/html;q=0.01";

/// Reads indexes over HTTP, trusting the certificates the platform trusts and the proxy the
/// environment names, as uv does.
pub struct RealIndexClient {
    /// Why there is no client, when TLS could not be set up.
    http: Result<reqwest::Client, String>,
}

impl RealIndexClient {
    pub fn new() -> Self {
        let http = reqwest::Client::builder()
            .user_agent(concat!("uview/", env!("CARGO_PKG_VERSION")))
            .connect_timeout(Duration::from_secs(5))
            .timeout(Duration::from_secs(15))
            .build()
            .map_err(|e| e.to_string());
        Self { http }
    }
}

impl Default for RealIndexClient {
    fn default() -> Self {
        Self::new()
    }
}

impl IndexClient for RealIndexClient {
    async fn project_page(&self, index: &str, name: &str) -> Result<Page, IndexError> {
        let http = self
            .http
            .as_ref()
            .map_err(|e| IndexError::Network(e.clone()))?;
        let url = project_url(index, name);
        let response = http
            .get(&url)
            .header(reqwest::header::ACCEPT, ACCEPT)
            .send()
            .await
            .map_err(|e| IndexError::Network(e.to_string()))?;
        match response.status().as_u16() {
            200..=299 => {}
            404 => return Err(IndexError::NotFound),
            code => return Err(IndexError::Status(code)),
        }
        let json = response
            .headers()
            .get(reqwest::header::CONTENT_TYPE)
            .and_then(|value| value.to_str().ok())
            .is_some_and(|value| value.contains("json"));
        let body = response
            .text()
            .await
            .map_err(|e| IndexError::Network(e.to_string()))?;
        Ok(Page { json, body })
    }
}

/// The page of the project `name` under the index at `index`: its normalized name, with the
/// trailing slash the Simple API asks for.
pub fn project_url(index: &str, name: &str) -> String {
    format!("{}/{}/", index.trim_end_matches('/'), normalize_name(name))
}
