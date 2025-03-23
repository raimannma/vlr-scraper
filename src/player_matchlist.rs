use chrono::{NaiveDate, NaiveDateTime, NaiveTime};
use itertools::{izip, Itertools};
use scraper::{ElementRef, Html, Selector};
use serde::Serialize;

use crate::enums::VlrScraperError;
use crate::utils;
use crate::utils::get_element_selector_value;

const MATCH_DATE_FORMAT: &str = "%Y/%m/%d";
const MATCH_TIME_FORMAT: &str = "%I:%M %p";

pub async fn get_player_matchlist(
    client: &reqwest::Client,
    player_id: u32,
    page: u8,
) -> Result<PlayerMatchList, VlrScraperError> {
    let url = format!(
        "https://www.vlr.gg/player/matches/{}/?page={}",
        player_id, page
    );
    let document = utils::get_document(client, url).await?;
    parse_matchlist(&document)
}

fn parse_matchlist(document: &Html) -> Result<PlayerMatchList, VlrScraperError> {
    let match_item_selector = "div#wrapper div.col a.m-item";
    let selector = Selector::parse(match_item_selector).map_err(VlrScraperError::SelectorError)?;
    document
        .select(&selector)
        .map(parse_match)
        .collect::<Result<_, _>>()
}

fn parse_match(element: ElementRef) -> Result<PlayerMatchListItem, VlrScraperError> {
    let href = element.value().attr("href");
    let (id, slug) = href
        .and_then(|href| {
            href.strip_prefix("/")
                .unwrap_or_default()
                .split("/")
                .collect_tuple()
        })
        .map(|(id, slug)| (id.parse().unwrap_or_default(), slug.to_string()))
        .ok_or(VlrScraperError::ParseError(
            "Failed to parse match URL".to_string(),
        ))?;

    let league_icon_selector =
        Selector::parse("div.m-item-thumb img").map_err(VlrScraperError::SelectorError)?;
    let league_icon = element
        .select(&league_icon_selector)
        .next()
        .and_then(|e| e.value().attr("src"))
        .map(utils::parse_img_link)
        .unwrap_or_default();

    let league_name_selector =
        Selector::parse("div.m-item-event div").map_err(VlrScraperError::SelectorError)?;
    let league_name = get_element_selector_value(&element, &league_name_selector);

    let league_series_selector =
        Selector::parse("div.m-item-event").map_err(VlrScraperError::SelectorError)?;
    let league_series_name = element
        .select(&league_series_selector)
        .filter_map(|t| t.text().last())
        .map(|t| t.trim().to_string())
        .last()
        .unwrap_or_default()
        .replace("\n", "")
        .replace("\t", "");

    let teams_selector =
        Selector::parse("div.m-item-team").map_err(VlrScraperError::SelectorError)?;
    let logos_selector =
        Selector::parse("div.m-item-logo img").map_err(VlrScraperError::SelectorError)?;
    let scores_selector =
        Selector::parse("div.m-item-result span").map_err(VlrScraperError::SelectorError)?;
    let teams = izip!(
        element.select(&teams_selector),
        element.select(&logos_selector),
        element.select(&scores_selector)
    )
    .map(|(team, logo, score)| parse_team(team, logo, score))
    .collect::<Result<_, _>>()?;

    let vods_selector = Selector::parse("div.m-item-vods div.wf-tag span.full")
        .map_err(VlrScraperError::SelectorError)?;
    let vods = element
        .select(&vods_selector)
        .filter_map(|t| t.text().last())
        .map(|t| t.trim().to_string())
        .collect_vec();

    let date_selector =
        Selector::parse("div.m-item-date div").map_err(VlrScraperError::SelectorError)?;
    let date = get_element_selector_value(&element, &date_selector);
    let date = NaiveDate::parse_from_str(&date, MATCH_DATE_FORMAT).ok();

    let time_selector =
        Selector::parse("div.m-item-date").map_err(VlrScraperError::SelectorError)?;
    let time = element
        .select(&time_selector)
        .filter_map(|t| t.text().last())
        .map(|t| t.trim().to_string())
        .last()
        .unwrap_or_default()
        .replace("\n", "")
        .replace("\t", "");
    let time = NaiveTime::parse_from_str(&time, MATCH_TIME_FORMAT).ok();

    Ok(PlayerMatchListItem {
        id,
        slug,
        league_icon,
        league_name,
        league_series_name,
        teams,
        vods,
        match_start: date.and_then(|d| time.map(|t| d.and_time(t))),
    })
}

fn parse_team(
    team_element: ElementRef,
    logo_element: ElementRef,
    score_element: ElementRef,
) -> Result<PlayerMatchListItemTeam, VlrScraperError> {
    let name_selector =
        Selector::parse("span.m-item-team-name").map_err(VlrScraperError::SelectorError)?;
    let name = get_element_selector_value(&team_element, &name_selector);

    let tag_selector =
        Selector::parse("span.m-item-team-tag").map_err(VlrScraperError::SelectorError)?;
    let tag = get_element_selector_value(&team_element, &tag_selector);

    let logo_url = logo_element
        .value()
        .attr("src")
        .map(utils::parse_img_link)
        .unwrap_or_default();

    let score = score_element
        .text()
        .last()
        .map(|s| s.trim())
        .unwrap_or_default()
        .parse()
        .ok();

    Ok(PlayerMatchListItemTeam {
        name,
        tag,
        logo_url,
        score,
    })
}

pub type PlayerMatchList = Vec<PlayerMatchListItem>;

#[derive(Debug, Clone, Serialize)]
pub struct PlayerMatchListItem {
    pub id: u32,
    pub slug: String,
    pub league_icon: String,
    pub league_name: String,
    pub league_series_name: String,
    pub teams: Vec<PlayerMatchListItemTeam>,
    pub vods: Vec<String>,
    pub match_start: Option<NaiveDateTime>,
}

#[derive(Debug, Clone, Serialize)]
pub struct PlayerMatchListItemTeam {
    pub name: String,
    pub tag: String,
    pub logo_url: String,
    pub score: Option<u8>,
}

#[cfg(test)]
mod tests {
    use crate::enums::Region;
    use crate::events::EventType;
    use crate::get_match;
    use crate::matchlist::get_matchlist;

    use super::*;

    #[tokio::test]
    async fn test_get_player_matchlist() {
        let client = reqwest::Client::new();

        let events = crate::events::get_events(&client, EventType::Completed, Region::All, 1)
            .await
            .unwrap();
        let event_id = events.events[0].id;

        let matches = get_matchlist(&client, event_id).await.unwrap();
        let match_id = matches[0].id;

        let r#match = get_match(&client, match_id).await.unwrap();
        let player_id = r#match.games[0].teams[0].players[0].id;

        let player_matchlist = get_player_matchlist(&client, player_id, 1).await.unwrap();
        assert!(!player_matchlist.is_empty());
        println!("{:#?}", player_matchlist);
    }
}
