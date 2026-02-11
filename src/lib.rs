//! # vlr-scraper
//!
//! A Rust library for scraping Valorant esports data from [vlr.gg](https://www.vlr.gg).
//!
//! ## Quick start
//!
//! ```no_run
//! # async fn example() -> vlr_scraper::Result<()> {
//! use vlr_scraper::{EventType, Region, VlrClient};
//!
//! let client = VlrClient::new();
//!
//! // Fetch upcoming events
//! let events = client
//!     .get_events(EventType::Upcoming, Region::All, 1)
//!     .await?;
//!
//! // Fetch matches for the first event
//! let matches = client.get_matchlist(events.events[0].id).await?;
//!
//! // Get detailed match info
//! let match_detail = client.get_match(matches[0].id).await?;
//!
//! // Fetch a player profile (info, teams, agent stats, news, placements)
//! let player = client
//!     .get_player(17323, AgentStatsTimespan::default())
//!     .await?;
//! println!("{} from {:?}", player.info.name, player.info.country);
//!
//! // Fetch a player's match history
//! let player_matches = client.get_player_matchlist(17323, 1).await?;
//! # Ok(())
//! # }
//! ```

mod client;
pub mod error;
pub mod model;
mod vlr_scraper;

// Re-export the client as the primary public API.
pub use client::VlrClient;
// Re-export error types at the crate root for convenience.
pub use error::{Result, VlrError};
// Re-export all model types at the crate root for convenience.
pub use model::*;
