use scraper::{ElementRef, Html, Selector};

use crate::enums::VlrScraperError;

pub(crate) async fn get_document(
    client: &reqwest::Client,
    url: String,
) -> Result<Html, VlrScraperError> {
    client
        .get(&url)
        .send()
        .await
        .map_err(VlrScraperError::ReqwestError)?
        .text()
        .await
        .map(|d| Html::parse_document(&d))
        .map_err(VlrScraperError::ReqwestError)
}

pub(crate) fn get_element_selector_value(element: &ElementRef, selector: &Selector) -> String {
    element
        .select(selector)
        .next()
        .and_then(|d| d.text().map(|t| t.trim()).find(|t| !t.is_empty()))
        .unwrap_or_default()
        .trim()
        .replace("\n", "")
        .replace("\t", "")
        .to_string()
}

pub fn parse_img_link(s: &str) -> String {
    if s.starts_with("//") {
        format!("https:{}", s)
    } else if s.starts_with("/") {
        format!("https://www.vlr.gg{}", s)
    } else {
        s.to_string()
    }
}
