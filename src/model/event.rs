use serde::{Deserialize, Serialize};
use strum_macros::EnumString;

/// Filter for the type of events to retrieve.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum EventType {
    Upcoming,
    Completed,
}

/// Paginated response containing a list of events.
#[derive(Debug, Clone, Serialize)]
pub struct EventsData {
    pub events: Vec<Event>,
    pub page: u8,
    pub total_pages: u8,
}

/// A single esports event (tournament/league).
#[derive(Debug, Clone, Serialize)]
pub struct Event {
    pub status: EventStatus,
    pub region: String,
    pub id: u32,
    pub title: String,
    pub slug: String,
    pub href: String,
    pub icon_url: String,
    pub price: String,
    pub dates: String,
}

/// The current status of an event.
#[derive(
    Debug, Default, Clone, Serialize, EnumString, strum_macros::Display, strum_macros::FromRepr,
)]
#[strum(serialize_all = "lowercase")]
pub enum EventStatus {
    Completed,
    Ongoing,
    Upcoming,
    #[default]
    #[strum(disabled)]
    Unknown,
}

/// Region filter for event queries.
#[derive(Debug, Clone, strum_macros::Display)]
#[strum(serialize_all = "kebab-case")]
pub enum Region {
    All,
    NorthAmerica,
    Europe,
    Brazil,
    AsiaPacific,
    Korea,
    Japan,
    LatinAmerica,
    Oceania,
    MiddleEastNorthAfrica,
    GameChangers,
    Collegiate,
}
