pub(crate) mod events;
pub(crate) mod match_detail;
pub(crate) mod matchlist;
pub(crate) mod player;

pub(crate) use ::scraper::Html;
use ::scraper::{ElementRef, Selector};
use tracing::debug;

use crate::error::{Result, VlrError};

const BASE_URL: &str = "https://www.vlr.gg";

/// Fetch a URL and parse the response body as an HTML document.
pub(crate) async fn get_document(client: &reqwest::Client, url: &str) -> Result<Html> {
    debug!(url, "fetching page");

    let response = client.get(url).send().await.map_err(|e| VlrError::Http {
        url: url.to_owned(),
        source: e,
    })?;

    let status = response.status();
    if !status.is_success() {
        return Err(VlrError::UnexpectedStatus {
            url: url.to_owned(),
            status,
        });
    }

    let body = response.text().await.map_err(|e| VlrError::ResponseBody {
        url: url.to_owned(),
        source: e,
    })?;

    Ok(Html::parse_document(&body))
}

/// Extract trimmed text content from the first element matching `selector`
/// inside `element`. Returns an empty string if nothing matches.
pub(crate) fn select_text(element: &ElementRef, selector: &Selector) -> String {
    element
        .select(selector)
        .next()
        .and_then(|d| d.text().map(|t| t.trim()).find(|t| !t.is_empty()))
        .unwrap_or_default()
        .trim()
        .replace(['\n', '\t'], "")
        .to_string()
}

/// Normalize a potentially relative image URL to an absolute vlr.gg URL.
pub(crate) fn normalize_img_url(src: &str) -> String {
    if src.starts_with("//") {
        format!("https:{src}")
    } else if src.starts_with('/') {
        format!("{BASE_URL}{src}")
    } else {
        src.to_string()
    }
}
