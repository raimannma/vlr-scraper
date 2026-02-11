use chrono::NaiveDate;
use serde::Serialize;

use super::common::{EventPlacement, Social};

/// Complete team profile data from a team overview page.
#[derive(Debug, Clone, Serialize)]
pub struct Team {
    pub info: TeamInfo,
    pub roster: Vec<TeamRosterMember>,
    pub event_placements: Vec<EventPlacement>,
    pub total_winnings: Option<String>,
}

/// Basic profile information for a team.
#[derive(Debug, Clone, Serialize)]
pub struct TeamInfo {
    pub id: u32,
    pub name: String,
    pub tag: Option<String>,
    pub logo_url: Option<String>,
    pub country: Option<String>,
    pub country_code: Option<String>,
    pub socials: Vec<Social>,
}

/// A member of a team's roster (player or staff).
#[derive(Debug, Clone, Serialize)]
pub struct TeamRosterMember {
    pub id: u32,
    pub slug: String,
    pub href: String,
    pub alias: String,
    pub real_name: Option<String>,
    pub country_code: Option<String>,
    pub avatar_url: Option<String>,
    pub role: String,
    pub is_captain: bool,
}

/// A single roster transaction (join, leave, or inactive change).
#[derive(Debug, Clone, Serialize)]
pub struct TeamTransaction {
    pub date: Option<NaiveDate>,
    pub action: String,
    pub player_id: u32,
    pub player_slug: String,
    pub player_alias: String,
    pub player_real_name: Option<String>,
    pub player_country_code: Option<String>,
    pub position: String,
    pub reference_url: Option<String>,
}

/// A list of team roster transactions.
pub type TeamTransactions = Vec<TeamTransaction>;
