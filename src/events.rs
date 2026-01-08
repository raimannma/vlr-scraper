use itertools::Itertools;
use log::{info, warn};
use scraper::error::SelectorErrorKind;
use scraper::{ElementRef, Html, Selector};
use serde::Serialize;

use crate::enums::{Region, VlrScraperError};
use crate::utils;
use crate::utils::get_element_selector_value;

pub enum EventType {
    Upcoming,
    Completed,
}

pub async fn get_events(
    client: &reqwest::Client,
    event_type: EventType,
    region: Region,
    page: u8,
) -> Result<EventsData, VlrScraperError> {
    let url = format!("https://www.vlr.gg/events/{region}?page={page}");
    let document = utils::get_document(client, url).await?;
    let events = parse_events(&event_type, &document)?;
    let total_pages = parse_total_pages(event_type, document)?;

    Ok(EventsData {
        events,
        page,
        total_pages,
    })
}

fn parse_total_pages(event_type: EventType, document: Html) -> Result<u8, VlrScraperError> {
    let total_pages_selector = match event_type {
        EventType::Upcoming => {
            "div#wrapper div.action-container div.action-container-pages:first-child :is(span,a)"
        }
        EventType::Completed => {
            "div#wrapper div.action-container div.action-container-pages:last-child :is(span,a)"
        }
    };
    let selector = Selector::parse(total_pages_selector)?;
    let mut total_pages_elements = document.select(&selector);
    let total_pages = total_pages_elements
        .next_back()
        .and_then(|e| e.text().next())
        .and_then(|t| t.parse::<u8>().ok())
        .unwrap_or(1);
    Ok(total_pages)
}

fn parse_events(event_type: &EventType, document: &Html) -> Result<Vec<Event>, VlrScraperError> {
    let event_item_selector = match event_type {
        EventType::Upcoming => {
            "div#wrapper div.events-container div.events-container-col:first-child a.event-item"
        }
        EventType::Completed => {
            "div#wrapper div.events-container div.events-container-col:last-child a.event-item"
        }
    };
    let selector = Selector::parse(event_item_selector)?;
    let events: Vec<Event> = document
        .select(&selector)
        .map(Event::try_from)
        .filter_map(Result::ok)
        .collect();
    Ok(events)
}

#[derive(Debug, Clone, Serialize)]
pub struct EventsData {
    pub events: Vec<Event>,
    pub page: u8,
    pub total_pages: u8,
}

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

#[derive(Debug, Clone, Serialize)]
pub enum EventStatus {
    Completed,
    Ongoing,
    Upcoming,
    Unknown,
}

impl From<String> for EventStatus {
    fn from(s: String) -> Self {
        match s.to_lowercase().as_str() {
            "ongoing" => Self::Ongoing,
            "upcoming" => Self::Upcoming,
            "completed" => Self::Completed,
            _ => {
                warn!("Unknown event status: {s}");
                Self::Unknown
            }
        }
    }
}

impl<'a> TryFrom<ElementRef<'a>> for Event {
    type Error = SelectorErrorKind<'a>;

    fn try_from(element: ElementRef<'a>) -> Result<Self, Self::Error> {
        info!("Convert element to Event: {element:?}");

        let href = element.value().attr("href");
        let href = href.unwrap_or_default().to_string();
        let (id, slug) = href
            .strip_prefix("/event/")
            .and_then(|s| s.split('/').map(|s| s.to_string()).collect_tuple())
            .unwrap_or_default();
        let href = format!("https://www.vlr.gg{href}");

        let icon_selector = Selector::parse("div.event-item-thumb img")?;
        let icon_url = element
            .select(&icon_selector)
            .next()
            .and_then(|icon| icon.value().attr("src"))
            .map(utils::parse_img_link)
            .unwrap_or_default();

        let title_selector = Selector::parse("div.event-item-inner div.event-item-title")?;
        let title = get_element_selector_value(&element, &title_selector);

        let status_selector = Selector::parse(
            "div.event-item-inner div.event-item-desc-item span.event-item-desc-item-status",
        )?;
        let status = get_element_selector_value(&element, &status_selector)
            .to_string()
            .into();

        let price_selector =
            Selector::parse("div.event-item-inner div.event-item-desc-item.mod-prize")?;
        let price = get_element_selector_value(&element, &price_selector);

        let dates_selector =
            Selector::parse("div.event-item-inner div.event-item-desc-item.mod-dates")?;
        let dates = get_element_selector_value(&element, &dates_selector);

        let region_selector =
            Selector::parse("div.event-item-inner div.event-item-desc-item.mod-location i")?;
        let region = element
            .select(&region_selector)
            .next()
            .and_then(|region| region.value().classes().find(|c| c.starts_with("mod-")))
            .map(|c| c.strip_prefix("mod-").unwrap_or_default())
            .unwrap_or_default()
            .trim()
            .to_string();

        Ok(Self {
            id: id.parse().unwrap_or_default(),
            title,
            slug,
            region,
            href,
            icon_url,
            status,
            price,
            dates,
        })
    }
}

#[cfg(test)]
mod tests {
    use crate::events::{get_events, EventType};

    use super::*;

    #[tokio::test]
    async fn test_get_upcoming_events() {
        let client = reqwest::Client::new();
        let events_data = get_events(&client, EventType::Upcoming, Region::All, 1).await;
        assert!(events_data.is_ok());
        let events_data = events_data.unwrap();
        assert!(!events_data.events.is_empty());
        println!("{:#?}", events_data.events.first());
    }

    #[tokio::test]
    async fn test_get_completed_events() {
        let client = reqwest::Client::new();
        let events_data = get_events(&client, EventType::Completed, Region::All, 2).await;
        assert!(events_data.is_ok());
        let events_data = events_data.unwrap();
        assert!(!events_data.events.is_empty());
        println!("{:#?}", events_data.events.first());
    }
}
