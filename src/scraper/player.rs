use chrono::{NaiveDate, NaiveTime};
use itertools::{izip, Itertools};
use ::scraper::{ElementRef, Selector};
use tracing::{debug, instrument};

use crate::error::{Result, VlrError};
use crate::model::{PlayerMatchList, PlayerMatchListItem, PlayerMatchListTeam};
use crate::scraper::{self, normalize_img_url, select_text};

const MATCH_DATE_FORMAT: &str = "%Y/%m/%d";
const MATCH_TIME_FORMAT: &str = "%I:%M %p";

#[instrument(skip(client))]
pub(crate) async fn get_player_matchlist(
    client: &reqwest::Client,
    player_id: u32,
    page: u8,
) -> Result<PlayerMatchList> {
    let url = format!("https://www.vlr.gg/player/matches/{player_id}/?page={page}");
    let document = scraper::get_document(client, &url).await?;
    let matches = parse_matchlist(&document)?;
    debug!(
        count = matches.len(),
        player_id, page, "parsed player match list"
    );
    Ok(matches)
}

fn parse_matchlist(document: &scraper::Html) -> Result<PlayerMatchList> {
    let match_item_selector = "div#wrapper div.col a.m-item";
    let selector = Selector::parse(match_item_selector)?;
    document
        .select(&selector)
        .map(parse_match_item)
        .collect::<Result<_>>()
}

fn parse_match_item(element: ElementRef) -> Result<PlayerMatchListItem> {
    let href = element.value().attr("href");
    let (id, slug) = href
        .and_then(|href| {
            href.strip_prefix("/")
                .unwrap_or_default()
                .split('/')
                .collect_tuple()
        })
        .map(|(id, slug)| (id.parse().unwrap_or_default(), slug.to_string()))
        .ok_or(VlrError::ElementNotFound {
            context: "player match item href",
        })?;

    let league_icon_selector = Selector::parse("div.m-item-thumb img")?;
    let league_icon = element
        .select(&league_icon_selector)
        .next()
        .and_then(|e| e.value().attr("src"))
        .map(normalize_img_url)
        .unwrap_or_default();

    let league_name_selector = Selector::parse("div.m-item-event div")?;
    let league_name = select_text(&element, &league_name_selector);

    let league_series_selector = Selector::parse("div.m-item-event")?;
    let league_series_name = element
        .select(&league_series_selector)
        .filter_map(|t| t.text().last())
        .map(|t| t.trim().to_string())
        .last()
        .unwrap_or_default()
        .replace('\n', "")
        .replace('\t', "");

    let teams_selector = Selector::parse("div.m-item-team")?;
    let logos_selector = Selector::parse("div.m-item-logo img")?;
    let scores_selector = Selector::parse("div.m-item-result span")?;
    let teams = izip!(
        element.select(&teams_selector),
        element.select(&logos_selector),
        element.select(&scores_selector)
    )
    .map(|(team, logo, score)| parse_team(team, logo, score))
    .collect::<Result<_>>()?;

    let vods_selector = Selector::parse("div.m-item-vods div.wf-tag span.full")?;
    let vods = element
        .select(&vods_selector)
        .filter_map(|t| t.text().last())
        .map(|t| t.trim().to_string())
        .collect_vec();

    let date_selector = Selector::parse("div.m-item-date div")?;
    let date = select_text(&element, &date_selector);
    let date = NaiveDate::parse_from_str(&date, MATCH_DATE_FORMAT).ok();

    let time_selector = Selector::parse("div.m-item-date")?;
    let time = element
        .select(&time_selector)
        .filter_map(|t| t.text().last())
        .map(|t| t.trim().to_string())
        .last()
        .unwrap_or_default()
        .replace('\n', "")
        .replace('\t', "");
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
) -> Result<PlayerMatchListTeam> {
    let name_selector = Selector::parse("span.m-item-team-name")?;
    let name = select_text(&team_element, &name_selector);

    let tag_selector = Selector::parse("span.m-item-team-tag")?;
    let tag = select_text(&team_element, &tag_selector);

    let logo_url = logo_element
        .value()
        .attr("src")
        .map(normalize_img_url)
        .unwrap_or_default();

    let score = score_element
        .text()
        .last()
        .map(|s| s.trim())
        .unwrap_or_default()
        .parse()
        .ok();

    Ok(PlayerMatchListTeam {
        name,
        tag,
        logo_url,
        score,
    })
}

#[cfg(test)]
mod tests {
    use crate::model::{EventType, Region};

    use super::*;

    #[tokio::test]
    async fn test_get_player_matchlist() {
        let client = reqwest::Client::new();

        let events =
            crate::scraper::events::get_events(&client, EventType::Completed, Region::All, 1)
                .await
                .unwrap();
        let event_id = events.events[0].id;

        let matches = crate::scraper::matchlist::get_matchlist(&client, event_id)
            .await
            .unwrap();
        let match_id = matches[0].id;

        let vlr_match = crate::scraper::match_detail::get_match(&client, match_id)
            .await
            .unwrap();
        let player_id = vlr_match.games[0].teams[0].players[0].id;

        let player_matchlist = get_player_matchlist(&client, player_id, 1).await.unwrap();
        assert!(!player_matchlist.is_empty());
    }
}
