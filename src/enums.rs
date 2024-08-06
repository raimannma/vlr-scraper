use std::fmt::{Display, Formatter};

use scraper::error::SelectorErrorKind;

#[derive(Debug)]
pub enum VlrScraperError {
    ReqwestError(reqwest::Error),
    SelectorError(SelectorErrorKind<'static>),
    ParseError(String),
    WrapperNotFound,
}

#[derive(Debug, Clone)]
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

impl Display for Region {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Region::All => write!(f, ""),
            Region::NorthAmerica => write!(f, "north-america"),
            Region::Europe => write!(f, "europe"),
            Region::Brazil => write!(f, "brazil"),
            Region::AsiaPacific => write!(f, "asia-pacific"),
            Region::Korea => write!(f, "korea"),
            Region::Japan => write!(f, "japan"),
            Region::LatinAmerica => write!(f, "latin-america"),
            Region::Oceania => write!(f, "oceania"),
            Region::MiddleEastNorthAfrica => write!(f, "mena"),
            Region::GameChangers => write!(f, "game-changers"),
            Region::Collegiate => write!(f, "collegiate"),
        }
    }
}
