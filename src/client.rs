use tracing::instrument;

use crate::error::Result;
use crate::model::*;
use crate::vlr_scraper;

/// The main entry point for interacting with VLR.gg.
///
/// `VlrClient` wraps a [`reqwest::Client`] and exposes methods
/// to fetch events, match lists, match details, player profiles, and player match histories.
///
/// # Examples
///
/// ```no_run
/// # async fn example() -> vlr_scraper::Result<()> {
/// use vlr_scraper::{AgentStatsTimespan, EventType, Region, VlrClient};
///
/// let client = VlrClient::new();
/// let events = client
///     .get_events(EventType::Upcoming, Region::All, 1)
///     .await?;
/// println!("Found {} events", events.events.len());
///
/// // Fetch a player profile
/// let player = client.get_player(17323, Default::default()).await?;
/// println!("{} ({:?})", player.info.name, player.info.country);
/// # Ok(())
/// # }
/// ```
pub struct VlrClient {
    http: reqwest::Client,
}

impl VlrClient {
    /// Create a new client with default settings.
    ///
    /// Uses a default [`reqwest::Client`] with no custom configuration.
    /// For custom timeouts, proxies, or headers, use [`VlrClient::with_client`].
    pub fn new() -> Self {
        Self {
            http: reqwest::Client::new(),
        }
    }

    /// Create a new client using the provided [`reqwest::Client`].
    ///
    /// Use this when you need to configure timeouts, proxies, headers, or
    /// other HTTP-level settings.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use vlr_scraper::VlrClient;
    ///
    /// let http = reqwest::Client::builder()
    ///     .timeout(std::time::Duration::from_secs(10))
    ///     .build()
    ///     .unwrap();
    /// let client = VlrClient::with_client(http);
    /// ```
    pub fn with_client(client: reqwest::Client) -> Self {
        Self { http: client }
    }

    /// Fetch a paginated list of events, filtered by type and region.
    ///
    /// Returns an [`EventsData`] containing a page of [`Event`] entries together
    /// with pagination info (`page` and `total_pages`). Each event includes its
    /// status, region, title, dates, icon URL, and price tier.
    ///
    /// # Arguments
    ///
    /// * `event_type` - Whether to retrieve [`EventType::Upcoming`] or [`EventType::Completed`] events.
    /// * `region` - Geographic filter (use [`Region::All`] for no filtering).
    /// * `page` - Page number (1-indexed).
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # async fn example() -> vlr_scraper::Result<()> {
    /// use vlr_scraper::{EventType, Region, VlrClient};
    ///
    /// let client = VlrClient::new();
    /// let data = client
    ///     .get_events(EventType::Upcoming, Region::Europe, 1)
    ///     .await?;
    /// for event in &data.events {
    ///     println!("[{}] {} ({})", event.status, event.title, event.dates);
    /// }
    /// # Ok(())
    /// # }
    /// ```
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
    ///
    /// Returns a [`MatchList`] (a `Vec<MatchListItem>`) where each item contains
    /// the match ID, slug, date/time, participating teams with scores, tags, and
    /// event series text.
    ///
    /// # Arguments
    ///
    /// * `event_id` - The VLR.gg event ID (found in [`Event::id`]).
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # async fn example() -> vlr_scraper::Result<()> {
    /// use vlr_scraper::VlrClient;
    ///
    /// let client = VlrClient::new();
    /// let matches = client.get_matchlist(2095).await?;
    /// for m in &matches {
    ///     let teams: Vec<_> = m.teams.iter().map(|t| t.name.as_str()).collect();
    ///     println!("{} — {}", teams.join(" vs "), m.event_series_text);
    /// }
    /// # Ok(())
    /// # }
    /// ```
    #[instrument(skip(self))]
    pub async fn get_matchlist(&self, event_id: u32) -> Result<MatchList> {
        vlr_scraper::matchlist::get_matchlist(&self.http, event_id).await
    }

    /// Fetch full details for a specific match by ID.
    ///
    /// Returns a [`Match`] containing:
    /// - [`MatchHeader`] — event info, date, and team names/scores
    /// - Live streams and VOD links as [`MatchStream`] entries
    /// - Per-map [`MatchGame`] data with team scores, player stats, and
    ///   round-by-round outcomes
    ///
    /// # Arguments
    ///
    /// * `match_id` - The VLR.gg match ID (found in [`MatchListItem::id`]).
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # async fn example() -> vlr_scraper::Result<()> {
    /// use vlr_scraper::VlrClient;
    ///
    /// let client = VlrClient::new();
    /// let m = client.get_match(429519).await?;
    /// println!("{} — {}", m.header.event_title, m.header.event_series_name);
    /// for game in &m.games {
    ///     println!(
    ///         "  {} — {} vs {}",
    ///         game.map, game.teams[0].name, game.teams[1].name
    ///     );
    /// }
    /// # Ok(())
    /// # }
    /// ```
    #[instrument(skip(self))]
    pub async fn get_match(&self, match_id: u32) -> Result<Match> {
        vlr_scraper::match_detail::get_match(&self.http, match_id).await
    }

    /// Fetch a paginated list of matches a player has participated in.
    ///
    /// Returns a [`PlayerMatchList`] (a `Vec<PlayerMatchListItem>`) where each
    /// entry contains the match ID, league name and icon, participating teams
    /// with scores, VOD links, and a match start timestamp.
    ///
    /// # Arguments
    ///
    /// * `player_id` - The VLR.gg player ID.
    /// * `page` - Page number (1-indexed).
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # async fn example() -> vlr_scraper::Result<()> {
    /// use vlr_scraper::VlrClient;
    ///
    /// let client = VlrClient::new();
    /// let matches = client.get_player_matchlist(17323, 1).await?;
    /// for m in &matches {
    ///     let teams: Vec<_> = m.teams.iter().map(|t| t.name.as_str()).collect();
    ///     println!("[{}] {}", m.league_name, teams.join(" vs "));
    /// }
    /// # Ok(())
    /// # }
    /// ```
    #[instrument(skip(self))]
    pub async fn get_player_matchlist(&self, player_id: u32, page: u8) -> Result<PlayerMatchList> {
        vlr_scraper::player::get_player_matchlist(&self.http, player_id, page).await
    }

    /// Fetch a complete player profile including info, teams, agent stats, news, and event placements.
    ///
    /// The returned [`Player`] contains:
    /// - [`PlayerInfo`] — name, real name, avatar URL, country/country code, and social links
    /// - Current and past [`PlayerTeam`] entries with team ID, name, logo, and join info
    /// - [`PlayerAgentStats`] for the given timespan (rating, ACS, K/D, ADR, KAST, etc.)
    /// - Recent [`PlayerNewsItem`] articles mentioning the player
    /// - [`PlayerEventPlacement`] history with per-stage results and total winnings
    ///
    /// # Arguments
    ///
    /// * `player_id` - The VLR.gg player ID.
    /// * `timespan` - Time window for agent statistics (see [`AgentStatsTimespan`]).
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # async fn example() -> vlr_scraper::Result<()> {
    /// use vlr_scraper::{AgentStatsTimespan, VlrClient};
    ///
    /// let client = VlrClient::new();
    /// let player = client.get_player(17323, AgentStatsTimespan::All).await?;
    ///
    /// println!("{} ({:?})", player.info.name, player.info.country);
    /// for team in &player.current_teams {
    ///     println!("  team: {}", team.name);
    /// }
    /// for stat in &player.agent_stats {
    ///     println!(
    ///         "  {} — rating {:.2}, K/D {:.2}",
    ///         stat.agent, stat.rating, stat.kd
    ///     );
    /// }
    /// # Ok(())
    /// # }
    /// ```
    #[instrument(skip(self))]
    pub async fn get_player(&self, player_id: u32, timespan: AgentStatsTimespan) -> Result<Player> {
        vlr_scraper::player::get_player(&self.http, player_id, timespan).await
    }

    /// Fetch a complete team profile including info, roster, event placements, and total winnings.
    ///
    /// The returned [`Team`] contains:
    /// - [`TeamInfo`] — name, tag, logo URL, country/country code, and social links
    /// - [`TeamRosterMember`] entries with player/staff info, roles, and captain status
    /// - [`EventPlacement`] history with stage results and prize earnings
    /// - Total career winnings as an optional string
    ///
    /// # Arguments
    ///
    /// * `team_id` - The VLR.gg team ID (found in team page URLs).
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # async fn example() -> vlr_scraper::Result<()> {
    /// use vlr_scraper::VlrClient;
    ///
    /// let client = VlrClient::new();
    /// let team = client.get_team(6530).await?;
    /// println!("{} ({:?})", team.info.name, team.info.tag);
    /// for member in &team.roster {
    ///     println!("  {} — {}", member.alias, member.role);
    /// }
    /// # Ok(())
    /// # }
    /// ```
    #[instrument(skip(self))]
    pub async fn get_team(&self, team_id: u32) -> Result<Team> {
        vlr_scraper::team::get_team(&self.http, team_id).await
    }
}

impl Default for VlrClient {
    fn default() -> Self {
        Self::new()
    }
}
