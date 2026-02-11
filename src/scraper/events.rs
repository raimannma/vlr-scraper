use std::str::FromStr;

use ::scraper::{ElementRef, Selector};
use itertools::Itertools;
use tracing::{debug, instrument};

use crate::error::{Result, VlrError};
use crate::model::{Event, EventStatus, EventType, EventsData, Region};
use crate::scraper::{self, normalize_img_url, select_text};

#[instrument(skip(client), fields(region = %region, page))]
pub(crate) async fn get_events(
    client: &reqwest::Client,
    event_type: EventType,
    region: Region,
    page: u8,
) -> Result<EventsData> {
    let url = format!("https://www.vlr.gg/events/{region}?page={page}");
    let document = scraper::get_document(client, &url).await?;
    let events = parse_events(&event_type, &document)?;
    let total_pages = parse_total_pages(event_type, &document)?;

    debug!(count = events.len(), total_pages, "parsed events page");

    Ok(EventsData {
        events,
        page,
        total_pages,
    })
}

fn parse_total_pages(event_type: EventType, document: &scraper::Html) -> Result<u8> {
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

fn parse_events(event_type: &EventType, document: &scraper::Html) -> Result<Vec<Event>> {
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
        .filter_map(|el| parse_event(el).ok())
        .collect();
    Ok(events)
}

fn parse_event(element: ElementRef) -> Result<Event> {
    let href = element.value().attr("href").unwrap_or_default().to_string();
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
        .map(normalize_img_url)
        .unwrap_or_default();

    let title_selector = Selector::parse("div.event-item-inner div.event-item-title")?;
    let title = select_text(&element, &title_selector);

    let status_selector = Selector::parse(
        "div.event-item-inner div.event-item-desc-item span.event-item-desc-item-status",
    )?;
    let status =
        EventStatus::from_str(&select_text(&element, &status_selector)).unwrap_or_default();

    let price_selector =
        Selector::parse("div.event-item-inner div.event-item-desc-item.mod-prize")?;
    let price = select_text(&element, &price_selector);

    let dates_selector =
        Selector::parse("div.event-item-inner div.event-item-desc-item.mod-dates")?;
    let dates = select_text(&element, &dates_selector);

    let region_selector =
        Selector::parse("div.event-item-inner div.event-item-desc-item.mod-location i")?;
    let region = element
        .select(&region_selector)
        .next()
        .and_then(|r| r.value().classes().find(|c| c.starts_with("mod-")))
        .map(|c| c.strip_prefix("mod-").unwrap_or_default())
        .unwrap_or_default()
        .trim()
        .to_string();

    Ok(Event {
        id: id
            .parse()
            .map_err(|e: std::num::ParseIntError| VlrError::IntParse(e))?,
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

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_get_upcoming_events() {
        let client = reqwest::Client::new();
        let events_data = get_events(&client, EventType::Upcoming, Region::All, 1).await;
        assert!(events_data.is_ok());
        let events_data = events_data.unwrap();
        assert!(!events_data.events.is_empty());
    }

    #[tokio::test]
    async fn test_get_completed_events() {
        let client = reqwest::Client::new();
        let events_data = get_events(&client, EventType::Completed, Region::All, 2).await;
        assert!(events_data.is_ok());
        let events_data = events_data.unwrap();
        assert!(!events_data.events.is_empty());
    }
}
