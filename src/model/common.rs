use serde::Serialize;

/// A social media link from a profile.
#[derive(Debug, Clone, Serialize)]
pub struct Social {
    pub platform: String,
    pub url: String,
    pub display_text: String,
}

/// A placement history at a single event.
#[derive(Debug, Clone, Serialize)]
pub struct EventPlacement {
    pub event_id: u32,
    pub event_slug: String,
    pub event_href: String,
    pub event_name: String,
    pub placements: Vec<PlacementEntry>,
    pub year: String,
}

/// A single placement entry within an event (stage + result).
#[derive(Debug, Clone, Serialize)]
pub struct PlacementEntry {
    pub stage: String,
    pub placement: String,
    pub prize: Option<String>,
    pub team_name: Option<String>,
}
