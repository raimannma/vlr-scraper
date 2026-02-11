use chrono::NaiveDateTime;
use itertools::Itertools;
use scraper::{CaseSensitivity, ElementRef, Selector};
use tracing::{debug, instrument};

use crate::error::{Result, VlrError};
use crate::model::{
    Match, MatchGame, MatchGamePlayer, MatchGameRound, MatchGameTeam, MatchHeader, MatchHeaderTeam,
    MatchStream,
};
use crate::vlr_scraper::{self, normalize_img_url, select_text};

#[instrument(skip(client))]
pub(crate) async fn get_match(client: &reqwest::Client, id: u32) -> Result<Match> {
    let url = format!("https://www.vlr.gg/{id}");
    let document = vlr_scraper::get_document(client, &url).await?;
    let column_selector = Selector::parse("div.col.mod-3")?;
    let column = document
        .select(&column_selector)
        .next()
        .ok_or(VlrError::ElementNotFound {
            context: "match page column (div.col.mod-3)",
        })?;
    let result = parse_match(id, &column)?;
    debug!(id, games = result.games.len(), "parsed match detail");
    Ok(result)
}

fn parse_match(id: u32, document: &ElementRef) -> Result<Match> {
    let header_selector = Selector::parse("div.match-header")?;
    let header = document
        .select(&header_selector)
        .next()
        .ok_or(VlrError::ElementNotFound {
            context: "match header (div.match-header)",
        })?;
    let header = parse_header(&header)?;

    let streams_container_selector =
        Selector::parse("div.match-streams div.match-streams-container div.match-streams-btn")?;
    let streams_name_selector = Selector::parse("div.match-streams-btn-embed span")?;
    let streams_link_selector = Selector::parse("a.match-streams-btn-external")?;
    let streams = document
        .select(&streams_container_selector)
        .map(|e| {
            let name = select_text(&e, &streams_name_selector);
            let link = e
                .select(&streams_link_selector)
                .next()
                .and_then(|e| e.value().attr("href"))
                .unwrap_or_default()
                .to_string();
            MatchStream { name, link }
        })
        .collect_vec();

    let vods_selector = Selector::parse("div.match-vods div.match-streams-container a")?;
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
    )?;
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

fn parse_header(header: &ElementRef) -> Result<MatchHeader> {
    let event_icon_selector = Selector::parse("div.match-header-super a.match-header-event img")?;
    let event_icon = header
        .select(&event_icon_selector)
        .next()
        .map(|e| {
            e.value()
                .attr("src")
                .map(normalize_img_url)
                .unwrap_or_default()
        })
        .ok_or(VlrError::ElementNotFound {
            context: "event icon (match-header-event img)",
        })?;

    let event_title_selector =
        Selector::parse("div.match-header-super a.match-header-event div div:first-child")?;
    let event_title = select_text(header, &event_title_selector);

    let event_series_name_selector = Selector::parse(
        "div.match-header-super a.match-header-event div div.match-header-event-series",
    )?;
    let event_series_name = select_text(header, &event_series_name_selector);

    let match_date_selector =
        Selector::parse("div.match-header-super div.match-header-date div.moment-tz-convert")?;
    let element = header
        .select(&match_date_selector)
        .next()
        .ok_or(VlrError::ElementNotFound {
            context: "match date element (moment-tz-convert)",
        })?;
    let date = element.value().attr("data-utc-ts").unwrap_or_default();
    let date = NaiveDateTime::parse_from_str(date, "%Y-%m-%d %H:%M:%S")?;

    let note_selector =
        Selector::parse("div.match-header-super div.match-header-date *:not(.moment-tz-convert)")?;
    let note = select_text(header, &note_selector);

    let team_links_selector = Selector::parse("div.match-header-vs a.match-header-link")?;
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
        .map(|(id, slug)| (id.parse().unwrap_or_default(), slug))
        .collect_vec();
    let team_hrefs = team_links
        .iter()
        .map(|e| {
            format!(
                "https://www.vlr.gg/{}",
                e.strip_prefix('/').unwrap_or_default()
            )
        })
        .collect_vec();
    let team_icons_selector = Selector::parse("div.match-header-vs a.match-header-link img")?;
    let team_icons = header
        .select(&team_icons_selector)
        .map(|e| {
            e.value()
                .attr("src")
                .map(normalize_img_url)
                .unwrap_or_default()
        })
        .collect_vec();

    let team_names_selector =
        Selector::parse("div.match-header-vs a.match-header-link div.wf-title-med")?;
    let team_names = header
        .select(&team_names_selector)
        .map(|e| e.text().next().unwrap_or_default().trim().to_string())
        .collect_vec();

    let team_scores_selector = Selector::parse(
        "div.match-header-vs div.match-header-vs-score div.match-header-vs-score span:not(.match-header-vs-score-colon)",
    ).ok();
    let team_scores: Vec<Option<u8>> = team_scores_selector
        .map(|sel| {
            header
                .select(&sel)
                .map(|e| e.text().next().unwrap_or_default().trim().to_string())
                .map(|s| s.parse().ok())
                .collect_vec()
        })
        .unwrap_or(vec![None, None]);

    let team_scores = if team_scores.len() == 2 {
        team_scores
    } else {
        vec![None, None]
    };

    let teams = team_id_slug
        .into_iter()
        .zip(team_hrefs)
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

fn parse_games(header: &MatchHeader, games: &[ElementRef]) -> Result<Vec<MatchGame>> {
    games.iter().map(|g| parse_game(header, g)).collect()
}

fn parse_game(header: &MatchHeader, game: &ElementRef) -> Result<MatchGame> {
    let map_name_selector =
        Selector::parse("div.vm-stats-game-header div.map div:first-child span")?;
    let map = select_text(game, &map_name_selector);

    let rounds_selector =
        Selector::parse("div.vlr-rounds div.vlr-rounds-row-col:not(:first-child,.mod-spacing)")?;
    let rounds = game.select(&rounds_selector).collect_vec();
    let rounds = parse_rounds(header, rounds)?;

    let players1_selector = Selector::parse(
        "div.vm-stats-container div div:first-child table tbody tr:has(td.mod-player)",
    )?;
    let players2_selector = Selector::parse(
        "div.vm-stats-container div div:last-child table tbody tr:has(td.mod-player)",
    )?;
    let players1 = game
        .select(&players1_selector)
        .map(parse_player)
        .collect::<Result<_>>()?;
    let players2 = game
        .select(&players2_selector)
        .map(parse_player)
        .collect::<Result<_>>()?;

    let team_name_selectors = Selector::parse("div.vm-stats-game-header div.team")?;
    let teams: Vec<MatchGameTeam> = game
        .select(&team_name_selectors)
        .zip(vec![players1, players2])
        .map(|(t, p)| parse_game_team(t, p))
        .collect();
    Ok(MatchGame { map, teams, rounds })
}

fn parse_player(player: ElementRef) -> Result<MatchGamePlayer> {
    let name_column_selector = Selector::parse("td.mod-player")?;
    let name_column =
        player
            .select(&name_column_selector)
            .next()
            .ok_or(VlrError::ElementNotFound {
                context: "player name column (td.mod-player)",
            })?;
    let nation_selector = Selector::parse("i.flag")?;
    let nation = name_column
        .select(&nation_selector)
        .next()
        .and_then(|e| e.value().attr("title"))
        .unwrap_or_default()
        .trim()
        .to_string();

    let a_tag_selector = Selector::parse("a")?;
    let a_tag = name_column.select(&a_tag_selector).next();
    let href = a_tag
        .and_then(|e| e.value().attr("href"))
        .unwrap_or_default()
        .to_string();
    let (id, slug) = href
        .strip_prefix("/player/")
        .unwrap_or_default()
        .split('/')
        .map(|s| s.to_string())
        .collect_tuple()
        .unwrap_or_default();
    let name_selector = Selector::parse("a div:first-child")?;
    let name = select_text(&name_column, &name_selector);

    let agent_selector = Selector::parse("td.mod-agents div span img")?;
    let agent = player
        .select(&agent_selector)
        .next()
        .and_then(|e| e.value().attr("title"))
        .unwrap_or_default()
        .to_string();

    Ok(MatchGamePlayer {
        nation,
        id: id.parse().unwrap_or_default(),
        slug,
        name,
        agent,
    })
}

fn parse_rounds(header: &MatchHeader, rounds: Vec<ElementRef>) -> Result<Vec<MatchGameRound>> {
    let round_number_selector = Selector::parse("div.rnd-num")?;
    let round_result_selector = Selector::parse("div.rnd-sq")?;
    let rounds: Vec<MatchGameRound> = rounds
        .iter()
        .filter_map(|r| {
            let round = select_text(r, &round_number_selector)
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
    Ok(rounds)
}

fn parse_game_team(team: ElementRef, players: Vec<MatchGamePlayer>) -> MatchGameTeam {
    let name_selector = Selector::parse("div.team-name").unwrap();
    let name = select_text(&team, &name_selector);

    let score_selector = Selector::parse("div.score").unwrap();
    let score = select_text(&team, &score_selector).parse().ok();

    let score_t_selector = Selector::parse("span.mod-t").unwrap();
    let score_t = select_text(&team, &score_t_selector).parse().ok();

    let score_ct_selector = Selector::parse("span.mod-ct").unwrap();
    let score_ct = select_text(&team, &score_ct_selector).parse().ok();

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
        players,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{EventType, Region};

    #[tokio::test]
    async fn test_get_match() {
        let client = reqwest::Client::new();

        let events =
            crate::vlr_scraper::events::get_events(&client, EventType::Completed, Region::All, 1)
                .await
                .unwrap();
        let event_id = events.events[0].id;

        let matches = crate::vlr_scraper::event_matchlist::get_event_matchlist(&client, event_id)
            .await
            .unwrap();
        let match_id = matches[0].id;

        let vlr_match = get_match(&client, match_id).await;
        assert!(vlr_match.is_ok());
    }
}
