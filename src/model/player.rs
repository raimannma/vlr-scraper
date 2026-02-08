use chrono::NaiveDateTime;
use serde::Serialize;

/// A list of matches a player has participated in.
pub type PlayerMatchList = Vec<PlayerMatchListItem>;

/// A single match entry in a player's match history.
#[derive(Debug, Clone, Serialize)]
pub struct PlayerMatchListItem {
    pub id: u32,
    pub slug: String,
    pub league_icon: String,
    pub league_name: String,
    pub league_series_name: String,
    pub teams: Vec<PlayerMatchListTeam>,
    pub vods: Vec<String>,
    pub match_start: Option<NaiveDateTime>,
}

/// Team information as shown in a player's match history.
#[derive(Debug, Clone, Serialize)]
pub struct PlayerMatchListTeam {
    pub name: String,
    pub tag: String,
    pub logo_url: String,
    pub score: Option<u8>,
}
