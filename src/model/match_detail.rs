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
    pub head_to_head: Vec<HeadToHeadMatch>,
    pub past_matches: Vec<TeamPastMatches>,
    pub performance: Option<MatchPerformance>,
    pub economy: Option<MatchEconomy>,
}

/// Header metadata for a match (event info, date, teams).
#[derive(Debug, Clone, Serialize)]
pub struct MatchHeader {
    pub event_icon: String,
    pub event_title: String,
    pub event_series_name: String,
    pub event_id: u32,
    pub event_slug: String,
    pub date: NaiveDateTime,
    pub patch: String,
    pub format: String,
    pub status: String,
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
    pub picked_by: Option<u32>,
    pub duration: Option<String>,
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

/// A previous head-to-head encounter between the two teams.
#[derive(Debug, Clone, Serialize)]
pub struct HeadToHeadMatch {
    pub match_id: u32,
    pub match_slug: String,
    pub event_name: String,
    pub event_series: String,
    pub event_icon: String,
    pub team1_score: u8,
    pub team2_score: u8,
    pub winner_index: u8,
    pub date: String,
}

/// A team's recent past matches.
#[derive(Debug, Clone, Serialize)]
pub struct TeamPastMatches {
    pub team_id: u32,
    pub matches: Vec<PastMatch>,
}

/// A single past match from a team's recent history.
#[derive(Debug, Clone, Serialize)]
pub struct PastMatch {
    pub match_id: u32,
    pub match_slug: String,
    pub score_for: u8,
    pub score_against: u8,
    pub is_win: bool,
    pub opponent_name: String,
    pub opponent_logo: String,
    pub date: String,
}

/// Overall performance data from the performance tab.
#[derive(Debug, Clone, Serialize)]
pub struct MatchPerformance {
    pub kill_matrix: Vec<KillMatrixEntry>,
    pub player_performances: Vec<PlayerPerformance>,
}

/// A single cell in the kill matrix (killer vs victim).
#[derive(Debug, Clone, Serialize)]
pub struct KillMatrixEntry {
    pub killer_id: u32,
    pub victim_id: u32,
    pub kills: u16,
    pub deaths: u16,
}

/// Detailed performance stats for a single player.
#[derive(Debug, Clone, Serialize)]
pub struct PlayerPerformance {
    pub player_id: u32,
    pub player_name: String,
    pub multi_kills_2k: u8,
    pub multi_kills_3k: u8,
    pub multi_kills_4k: u8,
    pub multi_kills_5k: u8,
    pub clutch_1v1: u8,
    pub clutch_1v2: u8,
    pub clutch_1v3: u8,
    pub clutch_1v4: u8,
    pub clutch_1v5: u8,
    pub econ_rating: u16,
    pub plants: u8,
    pub defuses: u8,
}

/// Economy data from the economy tab.
#[derive(Debug, Clone, Serialize)]
pub struct MatchEconomy {
    pub teams: Vec<TeamEconomy>,
}

/// Economy breakdown for a single team.
#[derive(Debug, Clone, Serialize)]
pub struct TeamEconomy {
    pub team_name: String,
    pub pistol_won: u8,
    pub eco_rounds: u8,
    pub eco_won: u8,
    pub semi_eco_rounds: u8,
    pub semi_eco_won: u8,
    pub semi_buy_rounds: u8,
    pub semi_buy_won: u8,
    pub full_buy_rounds: u8,
    pub full_buy_won: u8,
}

/// A player's participation in a single game.
#[derive(Debug, Clone, Serialize)]
pub struct MatchGamePlayer {
    pub nation: String,
    pub id: u32,
    pub name: String,
    pub slug: String,
    pub agent: String,
    pub rating: Option<f32>,
    pub acs: Option<u16>,
    pub kills: Option<u16>,
    pub deaths: Option<u16>,
    pub assists: Option<u16>,
    pub kd_diff: Option<i16>,
    pub kast: Option<f32>,
    pub adr: Option<f32>,
    pub hs_pct: Option<f32>,
    pub first_kills: Option<u16>,
    pub first_deaths: Option<u16>,
    pub fk_diff: Option<i16>,
}
