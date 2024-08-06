use chrono::{NaiveDate, NaiveDateTime, NaiveTime};
use itertools::Itertools;
use scraper::{CaseSensitivity, ElementRef, Html, Selector};

use crate::models::VlrScraperError;
use crate::utils;
use crate::utils::get_element_selector_value;

pub type MatchList = Vec<MatchListItem>;

pub async fn get_matchlist(
    client: &reqwest::Client,
    event_id: u32,
) -> Result<MatchList, VlrScraperError> {
    let url = format!("https://www.vlr.gg/event/matches/{}", event_id);
    let document = utils::get_document(client, url).await?;
    parse_matches(&document)
}

const MATCH_DATE_FORMAT: &str = "%a, %B %e, %Y";
const MATCH_DATE_FORMAT_ALT: &str = "%a, %b %e, %Y";
const MATCH_TIME_FORMAT: &str = "%I:%M %p";

fn parse_matches(document: &Html) -> Result<MatchList, VlrScraperError> {
    let match_item_selector = "div#wrapper :is(div.wf-label.mod-large,div.wf-card a.match-item)";
    let selector = Selector::parse(match_item_selector).map_err(VlrScraperError::SelectorError)?;
    let mut matches = vec![];
    let mut last_date = None;
    for element in document.select(&selector) {
        if element
            .value()
            .has_class("wf-label", CaseSensitivity::CaseSensitive)
        {
            if let Some(last_date_raw) = element.text().next() {
                let last_date_raw = last_date_raw.trim().to_string();
                last_date = Some(
                    NaiveDate::parse_from_str(&last_date_raw, MATCH_DATE_FORMAT)
                        .or(NaiveDate::parse_from_str(
                            &last_date_raw,
                            MATCH_DATE_FORMAT_ALT,
                        ))
                        .map_err(|_| {
                            VlrScraperError::ParseError("Failed to parse match date".to_string())
                        })?,
                );
            }
        } else {
            matches.push(parse_match(element, last_date.unwrap_or_default())?);
        }
    }
    Ok(matches)
}

fn parse_match(element: ElementRef, date: NaiveDate) -> Result<MatchListItem, VlrScraperError> {
    let href = element.value().attr("href");
    let href = href.unwrap_or_default().to_string();
    let (id, slug) = href
        .strip_prefix("/")
        .and_then(|s| s.split('/').map(|s| s.to_string()).collect_tuple())
        .unwrap_or_default();
    let href = format!("https://www.vlr.gg{}", href);

    let time_selector =
        Selector::parse("div.match-item-time").map_err(VlrScraperError::SelectorError)?;
    let time = get_element_selector_value(element, &time_selector);
    let time = NaiveTime::parse_from_str(&time, MATCH_TIME_FORMAT)
        .map_err(|_| VlrScraperError::ParseError("Failed to parse match time".to_string()))?;
    let date_time = date.and_time(time);

    let teams_selector = Selector::parse("div.match-item-vs div.match-item-vs-team")
        .map_err(VlrScraperError::SelectorError)?;
    let teams = element.select(&teams_selector).collect_vec();
    let teams = parse_teams(teams)?;

    let tags_selector =
        Selector::parse("div.match-item-vod div.wf-tag").map_err(VlrScraperError::SelectorError)?;
    let tags = element
        .select(&tags_selector)
        .filter_map(|t| t.text().last())
        .map(|t| t.trim().to_string())
        .collect_vec();

    let event_text_selector =
        Selector::parse("div.match-item-event.text-of").map_err(VlrScraperError::SelectorError)?;
    let event_text = element
        .select(&event_text_selector)
        .filter_map(|t| t.text().last())
        .map(|t| t.trim().to_string())
        .last()
        .unwrap_or_default();

    let event_series_text_selector =
        Selector::parse("div.match-item-event.text-of div.match-item-event-series.text-of")
            .map_err(VlrScraperError::SelectorError)?;
    let event_series_text = get_element_selector_value(element, &event_series_text_selector);

    Ok(MatchListItem {
        id: id
            .parse()
            .map_err(|_| VlrScraperError::ParseError("Failed to parse match ID".to_string()))?,
        slug,
        href,
        date_time,
        teams,
        tags,
        event_text,
        event_series_text,
    })
}

fn parse_teams(teams: Vec<ElementRef>) -> Result<Vec<Team>, VlrScraperError> {
    teams.into_iter().map(parse_team).collect()
}

fn parse_team(team: ElementRef) -> Result<Team, VlrScraperError> {
    let is_winner = team
        .value()
        .has_class("mod-winner", CaseSensitivity::CaseSensitive);

    let name_selector = Selector::parse("div.match-item-vs-team-name div.text-of")
        .map_err(VlrScraperError::SelectorError)?;
    let name = get_element_selector_value(team, &name_selector);

    let score_selector =
        Selector::parse("div.match-item-vs-team-score").map_err(VlrScraperError::SelectorError)?;
    let score = get_element_selector_value(team, &score_selector);
    let score = score.parse().ok();

    Ok(Team {
        name,
        is_winner,
        score,
    })
}

#[derive(Debug, Clone)]
pub struct Team {
    pub name: String,
    pub is_winner: bool,
    pub score: Option<u8>,
}

#[derive(Debug, Clone)]
pub struct MatchListItem {
    pub id: u32,
    pub slug: String,
    pub href: String,
    pub date_time: NaiveDateTime,
    pub teams: Vec<Team>,
    pub tags: Vec<String>,
    pub event_text: String,
    pub event_series_text: String,
}

#[cfg(test)]
mod tests {
    use crate::events::EventType;
    use crate::models::Region;

    use super::*;

    #[tokio::test]
    async fn test_get_matches() {
        let client = reqwest::Client::new();

        let events = crate::events::get_events(&client, EventType::Completed, Region::All, 1)
            .await
            .unwrap();
        let event_id = events.events[0].id;

        let matches = get_matchlist(&client, event_id).await.unwrap();
        assert!(!matches.is_empty());
        println!("{:#?}", matches);
    }
}
