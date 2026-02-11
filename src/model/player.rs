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

/// Complete player profile data from a player overview page.
#[derive(Debug, Clone, Serialize)]
pub struct Player {
    pub info: PlayerInfo,
    pub current_teams: Vec<PlayerTeam>,
    pub past_teams: Vec<PlayerTeam>,
    pub agent_stats: Vec<PlayerAgentStats>,
    pub news: Vec<PlayerNewsItem>,
    pub event_placements: Vec<PlayerEventPlacement>,
    pub total_winnings: Option<String>,
}

/// Basic profile information for a player.
#[derive(Debug, Clone, Serialize)]
pub struct PlayerInfo {
    pub id: u32,
    pub name: String,
    pub real_name: Option<String>,
    pub avatar_url: Option<String>,
    pub country: Option<String>,
    pub country_code: Option<String>,
    pub socials: Vec<PlayerSocial>,
}

/// A social media link from a player's profile.
#[derive(Debug, Clone, Serialize)]
pub struct PlayerSocial {
    pub platform: String,
    pub url: String,
    pub display_text: String,
}

/// A team associated with a player (current or past).
#[derive(Debug, Clone, Serialize)]
pub struct PlayerTeam {
    pub id: u32,
    pub slug: String,
    pub href: String,
    pub name: String,
    pub logo_url: String,
    pub info: Option<String>,
}

/// Agent usage and performance statistics for a player.
#[derive(Debug, Clone, Serialize)]
pub struct PlayerAgentStats {
    pub agent: String,
    pub usage_count: u32,
    pub usage_pct: f32,
    pub rounds: u32,
    pub rating: f32,
    pub acs: f32,
    pub kd: f32,
    pub adr: f32,
    pub kast: f32,
    pub kpr: f32,
    pub apr: f32,
    pub fkpr: f32,
    pub fdpr: f32,
    pub kills: u32,
    pub deaths: u32,
    pub assists: u32,
    pub first_kills: u32,
    pub first_deaths: u32,
}

/// Time window for agent statistics.
#[derive(
    Default,
    Debug,
    Clone,
    Copy,
    Hash,
    Eq,
    PartialEq,
    Serialize,
    strum_macros::Display,
    strum_macros::EnumString,
)]
pub enum AgentStatsTimespan {
    #[strum(serialize = "30d")]
    Days30,
    #[strum(serialize = "60d")]
    Days60,
    #[default]
    #[strum(serialize = "90d")]
    Days90,
    #[strum(serialize = "all")]
    All,
}

/// A news article mentioning the player.
#[derive(Debug, Clone, Serialize)]
pub struct PlayerNewsItem {
    pub href: String,
    pub date: String,
    pub title: String,
}

/// A player's placement history at a single event.
#[derive(Debug, Clone, Serialize)]
pub struct PlayerEventPlacement {
    pub event_id: u32,
    pub event_slug: String,
    pub event_href: String,
    pub event_name: String,
    pub placements: Vec<PlayerPlacementEntry>,
    pub year: String,
}

/// A single placement entry within an event (stage + result).
#[derive(Debug, Clone, Serialize)]
pub struct PlayerPlacementEntry {
    pub stage: String,
    pub placement: String,
    pub prize: Option<String>,
    pub team_name: String,
}
