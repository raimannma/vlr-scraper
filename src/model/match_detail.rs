use chrono::NaiveDateTime;
use serde::Serialize;

/// Full details of a single match, including all games played.
#[derive(Debug, Clone, Serialize)]
pub struct Match {
    pub id: u32,
    pub header: MatchHeader,
    pub streams: Vec<MatchStream>,
    pub vods: Vec<MatchStream>,
    pub games: Vec<MatchGame>,
}

/// Header metadata for a match (event info, date, teams).
#[derive(Debug, Clone, Serialize)]
pub struct MatchHeader {
    pub event_icon: String,
    pub event_title: String,
    pub event_series_name: String,
    pub date: NaiveDateTime,
    pub note: String,
    pub teams: Vec<MatchHeaderTeam>,
}

/// A team as shown in the match header.
#[derive(Debug, Clone, Serialize)]
pub struct MatchHeaderTeam {
    pub id: u32,
    pub slug: String,
    pub href: String,
    pub name: String,
    pub score: Option<u8>,
    pub icon: String,
}

/// A stream or VOD link associated with a match.
#[derive(Debug, Clone, Serialize)]
pub struct MatchStream {
    pub name: String,
    pub link: String,
}

/// Stats for a single game (map) within a match.
#[derive(Debug, Clone, Serialize)]
pub struct MatchGame {
    pub map: String,
    pub teams: Vec<MatchGameTeam>,
    pub rounds: Vec<MatchGameRound>,
}

/// Per-team stats for a single game.
#[derive(Debug, Clone, Serialize)]
pub struct MatchGameTeam {
    pub name: String,
    pub score: Option<u8>,
    pub score_t: Option<u8>,
    pub score_ct: Option<u8>,
    pub is_winner: bool,
    pub players: Vec<MatchGamePlayer>,
}

/// The outcome of a single round within a game.
#[derive(Debug, Clone, Serialize)]
pub struct MatchGameRound {
    pub round: u8,
    pub winning_team: u32,
    pub winning_site: String,
}

/// A player's participation in a single game.
#[derive(Debug, Clone, Serialize)]
pub struct MatchGamePlayer {
    pub nation: String,
    pub id: u32,
    pub name: String,
    pub slug: String,
    pub agent: String,
}
