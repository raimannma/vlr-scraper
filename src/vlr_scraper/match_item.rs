use chrono::{NaiveDate, NaiveTime};
use itertools::{izip, Itertools};
use scraper::{ElementRef, Selector};

use crate::error::{Result, VlrError};
use crate::model::{MatchItem, MatchItemTeam};
use crate::vlr_scraper::{normalize_img_url, select_text};

pub(crate) const MATCH_DATE_FORMAT: &str = "%Y/%m/%d";
pub(crate) const MATCH_TIME_FORMAT: &str = "%I:%M %p";

pub(crate) fn parse_match_items(document: &scraper::Html) -> Result<Vec<MatchItem>> {
    let match_item_selector = "div#wrapper div.col a.m-item";
    let selector = Selector::parse(match_item_selector)?;
    document
        .select(&selector)
        .map(parse_match_item)
        .collect::<Result<_>>()
}

fn parse_match_item(element: ElementRef) -> Result<MatchItem> {
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
            context: "match item href",
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
        .replace(['\n', '\t'], "");

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
        .replace(['\n', '\t'], "");
    let time = NaiveTime::parse_from_str(&time, MATCH_TIME_FORMAT).ok();

    Ok(MatchItem {
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
) -> Result<MatchItemTeam> {
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

    Ok(MatchItemTeam {
        name,
        tag,
        logo_url,
        score,
    })
}
