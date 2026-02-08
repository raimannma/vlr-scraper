use chrono::NaiveDateTime;
use serde::Serialize;

/// A list of matches belonging to a particular event.
pub type MatchList = Vec<MatchListItem>;

/// Summary information for a single match within an event.
#[derive(Debug, Clone, Serialize)]
pub struct MatchListItem {
    pub id: u32,
    pub slug: String,
    pub href: String,
    pub date_time: Option<NaiveDateTime>,
    pub teams: Vec<MatchListTeam>,
    pub tags: Vec<String>,
    pub event_text: String,
    pub event_series_text: String,
}

/// Team info as shown in a match list entry.
#[derive(Debug, Clone, Serialize)]
pub struct MatchListTeam {
    pub name: String,
    pub is_winner: bool,
    pub score: Option<u8>,
}
