use chrono::NaiveDateTime;
use itertools::Itertools;
use scraper::{ElementRef, Selector};

use crate::models::VlrScraperError;
use crate::utils;
use crate::utils::get_element_selector_value;

pub async fn get_match(client: &reqwest::Client, id: u32) -> Result<Match, VlrScraperError> {
    let url = format!("https://www.vlr.gg/{}", id);
    let document = utils::get_document(client, url).await?;
    let column_selector =
        Selector::parse("div.col.mod-3").map_err(VlrScraperError::SelectorError)?;
    let column = document
        .select(&column_selector)
        .next()
        .ok_or(VlrScraperError::ParseError(
            "Failed to parse match".to_string(),
        ))?;
    parse_match(id, &column)
}

fn parse_match(id: u32, document: &ElementRef) -> Result<Match, VlrScraperError> {
    let header_selector =
        Selector::parse("div.match-header").map_err(VlrScraperError::SelectorError)?;
    let header = document
        .select(&header_selector)
        .next()
        .ok_or(VlrScraperError::ParseError(
            "Failed to parse match header".to_string(),
        ))?;
    let header = parse_header(header)?;

    let streams_container_selector =
        Selector::parse("div.match-streams div.match-streams-container div.match-streams-btn")
            .map_err(VlrScraperError::SelectorError)?;
    let streams_name_selector = Selector::parse("div.match-streams-btn-embed span")
        .map_err(VlrScraperError::SelectorError)?;
    let streams_link_selector =
        Selector::parse("a.match-streams-btn-external").map_err(VlrScraperError::SelectorError)?;
    let streams = document
        .select(&streams_container_selector)
        .map(|e| {
            let name = get_element_selector_value(e, &streams_name_selector);
            let link = e
                .select(&streams_link_selector)
                .next()
                .and_then(|e| e.value().attr("href"))
                .unwrap_or_default()
                .to_string();
            MatchStream { name, link }
        })
        .collect_vec();

    let vods_selector = Selector::parse("div.match-vods div.match-streams-container a")
        .map_err(VlrScraperError::SelectorError)?;
    let vods = document
        .select(&vods_selector)
        .map(|e| {
            let name = e.text().next().unwrap_or_default().trim().to_string();
            let link = e.value().attr("href").unwrap_or_default().to_string();
            MatchStream { name, link }
        })
        .collect_vec();

    Ok(Match {
        id,
        header,
        streams,
        vods,
    })
}

fn parse_header(header: ElementRef) -> Result<MatchHeader, VlrScraperError> {
    let event_icon_selector = Selector::parse("div.match-header-super a.match-header-event img")
        .map_err(VlrScraperError::SelectorError)?;
    let event_icon = header
        .select(&event_icon_selector)
        .next()
        .map(|e| {
            e.value()
                .attr("src")
                .map(|s| {
                    if s.starts_with("//") {
                        format!("https:{}", s)
                    } else {
                        s.to_string()
                    }
                })
                .unwrap_or_default()
                .to_string()
        })
        .ok_or(VlrScraperError::ParseError(
            "Failed to parse event icon".to_string(),
        ))?;

    let event_title_selector =
        Selector::parse("div.match-header-super a.match-header-event div div:first-child")
            .map_err(VlrScraperError::SelectorError)?;
    let event_title = get_element_selector_value(header, &event_title_selector);

    let event_series_name_selector = Selector::parse(
        "div.match-header-super a.match-header-event div div.match-header-event-series",
    )
    .map_err(VlrScraperError::SelectorError)?;
    let event_series_name = get_element_selector_value(header, &event_series_name_selector);

    let match_date_selector =
        Selector::parse("div.match-header-super div.match-header-date div.moment-tz-convert")
            .map_err(VlrScraperError::SelectorError)?;
    let element = header
        .select(&match_date_selector)
        .next()
        .ok_or(VlrScraperError::ParseError(
            "Failed to parse match date".to_string(),
        ))?;
    let date = element.value().attr("data-utc-ts").unwrap_or_default();
    let date = NaiveDateTime::parse_from_str(date, "%Y-%m-%d %H:%M:%S")
        .map_err(|_| VlrScraperError::ParseError("Failed to parse match date".to_string()))?;

    let note_selector =
        Selector::parse("div.match-header-super div.match-header-date *:not(.moment-tz-convert)")
            .map_err(VlrScraperError::SelectorError)?;
    let note = get_element_selector_value(header, &note_selector);

    let team_links_selector = Selector::parse("div.match-header-vs a.match-header-link")
        .map_err(VlrScraperError::SelectorError)?;
    let team_links = header
        .select(&team_links_selector)
        .filter_map(|e| e.value().attr("href"))
        .collect_vec();
    let team_id_slug: Vec<(u32, String)> = team_links
        .iter()
        .map(|e| {
            e.strip_prefix("/team/")
                .unwrap_or_default()
                .split('/')
                .map(|s| s.to_string())
                .collect_tuple()
                .unwrap_or_default()
        })
        .map(|(id, slug)| (id.parse().unwrap_or_default(), slug.to_string()))
        .collect_vec();
    let team_links = team_links
        .iter()
        .map(|e| {
            format!(
                "https://www.vlr.gg{}",
                e.strip_prefix("/").unwrap_or_default()
            )
        })
        .collect_vec();

    let team_names_selector =
        Selector::parse("div.match-header-vs a.match-header-link div.wf-title-med")
            .map_err(VlrScraperError::SelectorError)?;
    let team_names = header
        .select(&team_names_selector)
        .map(|e| e.text().next().unwrap_or_default().trim().to_string())
        .collect_vec();

    let team_scores_selector = Selector::parse(
        "div.match-header-vs div.match-header-vs-score div.match-header-vs-score span:not(.match-header-vs-score-colon)",
    ).ok();
    let team_scores: Vec<Option<u8>> = team_scores_selector
        .map(|team_scores_selector| {
            header
                .select(&team_scores_selector)
                .map(|e| e.text().next().unwrap_or_default().trim().to_string())
                .map(|s| s.parse().ok())
                .collect_vec()
        })
        .unwrap_or(vec![None, None]);

    let team_scores = match team_scores.len() == 2 {
        true => team_scores,
        false => vec![None, None],
    };

    let teams = team_id_slug
        .into_iter()
        .zip(team_links)
        .zip(team_names)
        .zip(team_scores)
        .map(|((((id, slug), href), name), score)| MatchHeaderTeam {
            id,
            slug,
            href,
            name,
            score,
        })
        .collect_vec();

    Ok(MatchHeader {
        event_icon,
        event_title,
        event_series_name,
        date,
        note,
        teams,
    })
}

#[derive(Debug, Clone)]
pub struct Match {
    pub id: u32,
    pub header: MatchHeader,
    pub streams: Vec<MatchStream>,
    pub vods: Vec<MatchStream>,
}

#[derive(Debug, Clone)]
pub struct MatchHeader {
    pub event_icon: String,
    pub event_title: String,
    pub event_series_name: String,
    pub date: NaiveDateTime,
    pub note: String,
    pub teams: Vec<MatchHeaderTeam>,
}

#[derive(Debug, Clone)]
pub struct MatchHeaderTeam {
    pub id: u32,
    pub slug: String,
    pub href: String,
    pub name: String,
    pub score: Option<u8>,
}

#[derive(Debug, Clone)]
pub struct MatchStream {
    pub name: String,
    pub link: String,
}

#[cfg(test)]
mod tests {
    use crate::events::EventType;
    use crate::matchlist::get_matchlist;
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
        let match_id = matches[0].id;

        let r#match = get_match(&client, match_id).await;
        assert!(r#match.is_ok());
        let r#match = r#match.unwrap();
        println!("{:#?}", r#match);
    }
}
