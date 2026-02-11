use chrono::NaiveDateTime;
use serde::Serialize;

/// A list of match items (used by both player and team match histories).
pub type MatchItemList = Vec<MatchItem>;

/// A single match entry in a match history.
#[derive(Debug, Clone, Serialize)]
pub struct MatchItem {
    pub id: u32,
    pub slug: String,
    pub league_icon: String,
    pub league_name: String,
    pub league_series_name: String,
    pub teams: Vec<MatchItemTeam>,
    pub vods: Vec<String>,
    pub match_start: Option<NaiveDateTime>,
}

/// Team information as shown in a match history item.
#[derive(Debug, Clone, Serialize)]
pub struct MatchItemTeam {
    pub name: String,
    pub tag: String,
    pub logo_url: String,
    pub score: Option<u8>,
}
