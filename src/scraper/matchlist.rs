use chrono::{NaiveDate, NaiveTime};
use itertools::Itertools;
use ::scraper::{CaseSensitivity, ElementRef, Selector};
use tracing::{debug, instrument, warn};

use crate::error::Result;
use crate::model::{MatchList, MatchListItem, MatchListTeam};
use crate::scraper::{self, select_text};

const MATCH_DATE_FORMAT: &str = "%a, %B %e, %Y";
const MATCH_DATE_FORMAT_ALT: &str = "%a, %b %e, %Y";
const MATCH_TIME_FORMAT: &str = "%I:%M %p";

#[instrument(skip(client))]
pub(crate) async fn get_matchlist(client: &reqwest::Client, event_id: u32) -> Result<MatchList> {
    let url = format!("https://www.vlr.gg/event/matches/{event_id}");
    let document = scraper::get_document(client, &url).await?;
    let matches = parse_matches(&document)?;
    debug!(count = matches.len(), event_id, "parsed match list");
    Ok(matches)
}

fn parse_matches(document: &scraper::Html) -> Result<MatchList> {
    let match_item_selector = "div#wrapper :is(div.wf-label.mod-large,div.wf-card a.match-item)";
    let selector = Selector::parse(match_item_selector)?;
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
                    NaiveDate::parse_from_str(&last_date_raw, MATCH_DATE_FORMAT).or_else(|_| {
                        NaiveDate::parse_from_str(&last_date_raw, MATCH_DATE_FORMAT_ALT)
                    })?,
                );
            }
        } else {
            match parse_match_item(&element, last_date) {
                Ok(item) => matches.push(item),
                Err(e) => warn!(error = %e, "skipping unparsable match item"),
            }
        }
    }
    Ok(matches)
}

fn parse_match_item(element: &ElementRef, date: Option<NaiveDate>) -> Result<MatchListItem> {
    let href = element.value().attr("href").unwrap_or_default().to_string();
    let (id, slug) = href
        .strip_prefix("/")
        .and_then(|s| s.split('/').map(|s| s.to_string()).collect_tuple())
        .unwrap_or_default();
    let href = format!("https://www.vlr.gg{href}");

    let time_selector = Selector::parse("div.match-item-time")?;
    let time = select_text(element, &time_selector);
    let time = NaiveTime::parse_from_str(&time, MATCH_TIME_FORMAT).ok();
    let date_time = date.and_then(|d| time.map(|t| d.and_time(t)));

    let teams_selector = Selector::parse("div.match-item-vs div.match-item-vs-team")?;
    let teams = element.select(&teams_selector).collect_vec();
    let teams = parse_teams(&teams)?;

    let tags_selector = Selector::parse("div.match-item-vod div.wf-tag")?;
    let tags = element
        .select(&tags_selector)
        .filter_map(|t| t.text().last())
        .map(|t| t.trim().to_string())
        .collect_vec();

    let event_text_selector = Selector::parse("div.match-item-event.text-of")?;
    let event_text = element
        .select(&event_text_selector)
        .filter_map(|t| t.text().last())
        .map(|t| t.trim().to_string())
        .last()
        .unwrap_or_default();

    let event_series_text_selector =
        Selector::parse("div.match-item-event.text-of div.match-item-event-series.text-of")?;
    let event_series_text = select_text(element, &event_series_text_selector);

    Ok(MatchListItem {
        id: id.parse()?,
        slug,
        href,
        date_time,
        teams,
        tags,
        event_text,
        event_series_text,
    })
}

fn parse_teams(teams: &[ElementRef]) -> Result<Vec<MatchListTeam>> {
    teams.iter().map(parse_team).collect()
}

fn parse_team(team: &ElementRef) -> Result<MatchListTeam> {
    let is_winner = team
        .value()
        .has_class("mod-winner", CaseSensitivity::CaseSensitive);

    let name_selector = Selector::parse("div.match-item-vs-team-name div.text-of")?;
    let name = select_text(team, &name_selector);

    let score_selector = Selector::parse("div.match-item-vs-team-score")?;
    let score = select_text(team, &score_selector);
    let score = score.parse().ok();

    Ok(MatchListTeam {
        name,
        is_winner,
        score,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{EventType, Region};

    #[tokio::test]
    async fn test_get_matches() {
        let client = reqwest::Client::new();

        let events =
            crate::scraper::events::get_events(&client, EventType::Completed, Region::All, 1)
                .await
                .unwrap();
        let event_id = events.events[0].id;

        let matches = get_matchlist(&client, event_id).await.unwrap();
        assert!(!matches.is_empty());
    }
}
