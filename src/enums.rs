use scraper::error::SelectorErrorKind;
use std::num::ParseIntError;

#[derive(thiserror::Error, Debug)]
pub enum VlrScraperError {
    #[error("Reqwest error: {0}")]
    ReqwestError(#[from] reqwest::Error),
    #[error("Selector error: {0}")]
    SelectorError(#[from] SelectorErrorKind<'static>),
    #[error("Integer Parse error: {0}")]
    IntegerParseError(#[from] ParseIntError),
    #[error("Date Parse error: {0}")]
    DateParseError(#[from] chrono::ParseError),
    #[error("Wrapper not found")]
    ElementNotFound,
}

#[derive(Debug, Clone, strum_macros::Display)]
#[strum(serialize_all = "kebab-case")]
pub enum Region {
    All,
    NorthAmerica,
    Europe,
    Brazil,
    AsiaPacific,
    Korea,
    Japan,
    LatinAmerica,
    Oceania,
    MiddleEastNorthAfrica,
    GameChangers,
    Collegiate,
}
