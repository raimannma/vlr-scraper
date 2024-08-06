use chrono::NaiveDateTime;
use itertools::Itertools;
use scraper::{CaseSensitivity, ElementRef, Selector};
use serde::Serialize;

use crate::enums::VlrScraperError;
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
    let header = parse_header(&header)?;

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
            let name = get_element_selector_value(&e, &streams_name_selector);
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

    let games_selector = Selector::parse(
        "div.vm-stats div.vm-stats-container div.vm-stats-game:not([data-game-id='all'])",
    )
    .map_err(VlrScraperError::SelectorError)?;
    let games = document.select(&games_selector).collect_vec();
    let games = parse_games(&header, &games)?;

    Ok(Match {
        id,
        header,
        streams,
        vods,
        games,
    })
}

fn parse_header(header: &ElementRef) -> Result<MatchHeader, VlrScraperError> {
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
                "https://www.vlr.gg/{}",
                e.strip_prefix("/").unwrap_or_default()
            )
        })
        .collect_vec();
    let team_icons_selector = Selector::parse("div.match-header-vs a.match-header-link img")
        .map_err(VlrScraperError::SelectorError)?;
    let team_icons = header
        .select(&team_icons_selector)
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
        .zip(team_icons)
        .map(
            |(((((id, slug), href), name), score), icon)| MatchHeaderTeam {
                id,
                slug,
                href,
                name,
                score,
                icon,
            },
        )
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

fn parse_games(
    header: &MatchHeader,
    games: &[ElementRef],
) -> Result<Vec<MatchGame>, VlrScraperError> {
    games.iter().map(|g| parse_game(header, g)).collect()
}

fn parse_game(header: &MatchHeader, game: &ElementRef) -> Result<MatchGame, VlrScraperError> {
    let map_name_selector =
        Selector::parse("div.vm-stats-game-header div.map div:first-child span")
            .map_err(VlrScraperError::SelectorError)?;
    let map = get_element_selector_value(game, &map_name_selector);

    let team_name_selectors = Selector::parse("div.vm-stats-game-header div.team")
        .map_err(VlrScraperError::SelectorError)?;
    let teams: Vec<MatchGameTeam> = game
        .select(&team_name_selectors)
        .map(parse_game_team)
        .collect();

    let rounds_selector =
        Selector::parse("div.vlr-rounds div.vlr-rounds-row-col:not(:first-child,.mod-spacing)")
            .map_err(VlrScraperError::SelectorError)?;
    let rounds = game.select(&rounds_selector).collect_vec();
    let round_number_selector =
        Selector::parse("div.rnd-num").map_err(VlrScraperError::SelectorError)?;
    let round_result_selector =
        Selector::parse("div.rnd-sq").map_err(VlrScraperError::SelectorError)?;
    let rounds: Vec<MatchGameRound> = rounds
        .iter()
        .filter_map(|r| {
            let round = get_element_selector_value(r, &round_number_selector)
                .parse()
                .unwrap_or_default();
            let winning_team = r
                .select(&round_result_selector)
                .map(|e| {
                    e.value()
                        .classes()
                        .map(|c| c.trim().to_string())
                        .collect_vec()
                })
                .find_position(|c| c.contains(&"mod-win".to_string()));
            if let Some((winning_team_index, winning_team)) = winning_team {
                header
                    .teams
                    .get(winning_team_index)
                    .map(|t| t.id)
                    .map(|team_id| MatchGameRound {
                        round,
                        winning_team: team_id,
                        winning_site: if winning_team.contains(&"mod-t".to_string()) {
                            "t".to_string()
                        } else {
                            "ct".to_string()
                        },
                    })
            } else {
                None
            }
        })
        .collect_vec();
    Ok(MatchGame { map, teams, rounds })
}

fn parse_game_team(team: ElementRef) -> MatchGameTeam {
    let name_selector = Selector::parse("div.team-name").unwrap();
    let name = get_element_selector_value(&team, &name_selector);

    let score_selector = Selector::parse("div.score").unwrap();
    let score = get_element_selector_value(&team, &score_selector)
        .parse()
        .ok();

    let score_t_selector = Selector::parse("span.mod-t").unwrap();
    let score_t = get_element_selector_value(&team, &score_t_selector)
        .parse()
        .ok();

    let score_ct_selector = Selector::parse("span.mod-ct").unwrap();
    let score_ct = get_element_selector_value(&team, &score_ct_selector)
        .parse()
        .ok();

    let is_winner = team
        .select(&score_selector)
        .next()
        .map(|e| {
            e.value()
                .has_class("mod-win", CaseSensitivity::CaseSensitive)
        })
        .unwrap_or_default();

    MatchGameTeam {
        name,
        score,
        score_t,
        score_ct,
        is_winner,
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct Match {
    pub id: u32,
    pub header: MatchHeader,
    pub streams: Vec<MatchStream>,
    pub vods: Vec<MatchStream>,
    pub games: Vec<MatchGame>,
}

#[derive(Debug, Clone, Serialize)]
pub struct MatchHeader {
    pub event_icon: String,
    pub event_title: String,
    pub event_series_name: String,
    pub date: NaiveDateTime,
    pub note: String,
    pub teams: Vec<MatchHeaderTeam>,
}

#[derive(Debug, Clone, Serialize)]
pub struct MatchHeaderTeam {
    pub id: u32,
    pub slug: String,
    pub href: String,
    pub name: String,
    pub score: Option<u8>,
    pub icon: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct MatchStream {
    pub name: String,
    pub link: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct MatchGame {
    pub map: String,
    pub teams: Vec<MatchGameTeam>,
    pub rounds: Vec<MatchGameRound>,
}

#[derive(Debug, Clone, Serialize)]
pub struct MatchGameTeam {
    pub name: String,
    pub score: Option<u8>,
    pub score_t: Option<u8>,
    pub score_ct: Option<u8>,
    pub is_winner: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct MatchGameRound {
    pub round: u8,
    pub winning_team: u32,
    pub winning_site: String,
}

#[cfg(test)]
mod tests {
    use crate::enums::Region;
    use crate::events::EventType;
    use crate::matchlist::get_matchlist;

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
