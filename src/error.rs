use ::scraper::error::SelectorErrorKind;
use std::num::ParseIntError;

/// All errors that can occur during VLR scraping operations.
#[derive(thiserror::Error, Debug)]
pub enum VlrError {
    /// HTTP request failed (network, DNS, TLS, timeout, etc.).
    #[error("http request failed for {url}: {source}")]
    Http {
        url: String,
        source: reqwest::Error,
    },

    /// Server returned a non-success HTTP status code.
    #[error("unexpected status {status} for {url}")]
    UnexpectedStatus {
        url: String,
        status: reqwest::StatusCode,
    },

    /// Failed to read the response body as text.
    #[error("failed to read response body from {url}: {source}")]
    ResponseBody {
        url: String,
        source: reqwest::Error,
    },

    /// A CSS selector string could not be parsed.
    #[error("invalid CSS selector: {0}")]
    Selector(String),

    /// Failed to parse an integer from scraped text.
    #[error("failed to parse integer: {0}")]
    IntParse(#[from] ParseIntError),

    /// Failed to parse a date/time from scraped text.
    #[error("failed to parse date: {0}")]
    DateParse(#[from] chrono::ParseError),

    /// An expected HTML element was not found on the page.
    #[error("expected element not found: {context}")]
    ElementNotFound { context: &'static str },
}

impl<'a> From<SelectorErrorKind<'a>> for VlrError {
    fn from(err: SelectorErrorKind<'a>) -> Self {
        VlrError::Selector(err.to_string())
    }
}

pub type Result<T> = std::result::Result<T, VlrError>;
