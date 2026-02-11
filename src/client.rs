use tracing::instrument;

use crate::error::Result;
use crate::model::*;
use crate::vlr_scraper;

/// The main entry point for interacting with VLR.gg.
///
/// `VlrClient` wraps a [`reqwest::Client`] and exposes methods
/// to fetch events, match lists, match details, and player histories.
///
/// # Examples
///
/// ```no_run
/// # async fn example() -> vlr_scraper::Result<()> {
/// use vlr_scraper::{EventType, Region, VlrClient};
///
/// let client = VlrClient::new();
/// let events = client
///     .get_events(EventType::Upcoming, Region::All, 1)
///     .await?;
/// println!("Found {} events", events.events.len());
/// # Ok(())
/// # }
/// ```
pub struct VlrClient {
    http: reqwest::Client,
}

impl VlrClient {
    /// Create a new client with default settings.
    pub fn new() -> Self {
        Self {
            http: reqwest::Client::new(),
        }
    }

    /// Create a new client using the provided [`reqwest::Client`].
    ///
    /// Use this when you need to configure timeouts, proxies, headers, etc.
    pub fn with_client(client: reqwest::Client) -> Self {
        Self { http: client }
    }

    /// Fetch a paginated list of events, filtered by type and region.
    #[instrument(skip(self))]
    pub async fn get_events(
        &self,
        event_type: EventType,
        region: Region,
        page: u8,
    ) -> Result<EventsData> {
        vlr_scraper::events::get_events(&self.http, event_type, region, page).await
    }

    /// Fetch all matches belonging to an event.
    #[instrument(skip(self))]
    pub async fn get_matchlist(&self, event_id: u32) -> Result<MatchList> {
        vlr_scraper::matchlist::get_matchlist(&self.http, event_id).await
    }

    /// Fetch full details for a specific match by ID.
    #[instrument(skip(self))]
    pub async fn get_match(&self, match_id: u32) -> Result<Match> {
        vlr_scraper::match_detail::get_match(&self.http, match_id).await
    }

    /// Fetch a paginated list of matches a player has participated in.
    #[instrument(skip(self))]
    pub async fn get_player_matchlist(&self, player_id: u32, page: u8) -> Result<PlayerMatchList> {
        vlr_scraper::player::get_player_matchlist(&self.http, player_id, page).await
    }

    /// Fetch a complete player profile including info, teams, agent stats, news, and event placements.
    #[instrument(skip(self))]
    pub async fn get_player(&self, player_id: u32, timespan: AgentStatsTimespan) -> Result<Player> {
        vlr_scraper::player::get_player(&self.http, player_id, timespan).await
    }
}

impl Default for VlrClient {
    fn default() -> Self {
        Self::new()
    }
}
