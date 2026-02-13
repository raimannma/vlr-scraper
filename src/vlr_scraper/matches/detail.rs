use chrono::NaiveDateTime;
use itertools::Itertools;
use scraper::{CaseSensitivity, ElementRef, Selector};
use tracing::{debug, instrument};

use crate::error::{Result, VlrError};
use crate::model::{
    HeadToHeadMatch, KillMatrixEntry, Match, MatchEconomy, MatchGame, MatchGamePlayer,
    MatchGameRound, MatchGameTeam, MatchHeader, MatchHeaderTeam, MatchPerformance, MatchStream,
    PastMatch, PlayerPerformance, TeamEconomy, TeamPastMatches,
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
    let mut result = parse_match(id, &column)?;

    // Fetch performance and economy tabs concurrently
    let perf_url = format!("https://www.vlr.gg/{id}/?tab=performance");
    let econ_url = format!("https://www.vlr.gg/{id}/?tab=economy");
    let (perf_result, econ_result) = futures::join!(
        vlr_scraper::get_document(client, &perf_url),
        vlr_scraper::get_document(client, &econ_url),
    );

    let col_selector = Selector::parse("div.col.mod-3").unwrap_or_else(|_| unreachable!());

    result.performance = match perf_result {
        Ok(perf_doc) => perf_doc
            .select(&col_selector)
            .next()
            .and_then(|col| parse_performance(&col, &result).ok()),
        Err(e) => {
            debug!(id, error = %e, "failed to fetch performance tab");
            None
        }
    };

    result.economy = match econ_result {
        Ok(econ_doc) => econ_doc
            .select(&col_selector)
            .next()
            .and_then(|col| parse_economy(&col).ok()),
        Err(e) => {
            debug!(id, error = %e, "failed to fetch economy tab");
            None
        }
    };

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

    let head_to_head = parse_head_to_head(document)?;
    let past_matches = parse_past_matches(&header, document)?;

    Ok(Match {
        id,
        header,
        streams,
        vods,
        games,
        head_to_head,
        past_matches,
        performance: None,
        economy: None,
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

    let patch_selector =
        Selector::parse("div.match-header-super div.match-header-date > div:nth-child(3)")?;
    let patch_raw = select_text(header, &patch_selector);
    let patch = patch_raw
        .strip_prefix("Patch ")
        .unwrap_or(&patch_raw)
        .to_string();

    let vs_note_selector = Selector::parse("div.match-header-vs-note")?;
    let vs_notes: Vec<String> = header
        .select(&vs_note_selector)
        .map(|e| e.text().next().unwrap_or_default().trim().to_string())
        .collect();
    let status = vs_notes.first().cloned().unwrap_or_default();
    let format = vs_notes.get(1).cloned().unwrap_or_default();

    let event_link_selector = Selector::parse("div.match-header-super a.match-header-event")?;
    let event_href = header
        .select(&event_link_selector)
        .next()
        .and_then(|e| e.value().attr("href"))
        .unwrap_or_default();
    let (event_id, event_slug) = {
        let parts: Vec<&str> = event_href
            .strip_prefix("/event/")
            .unwrap_or_default()
            .splitn(3, '/')
            .collect();
        let id = parts
            .first()
            .and_then(|s| s.parse::<u32>().ok())
            .unwrap_or_default();
        let slug = parts.get(1).unwrap_or(&"").to_string();
        (id, slug)
    };

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
        event_id,
        event_slug,
        date,
        patch,
        format,
        status,
        note,
        teams,
    })
}

fn parse_head_to_head(document: &ElementRef) -> Result<Vec<HeadToHeadMatch>> {
    let item_selector = Selector::parse("div.match-h2h a.wf-module-item.mod-h2h")?;
    let event_icon_selector = Selector::parse("div.match-h2h-matches-event img")?;
    let event_name_selector = Selector::parse("div.match-h2h-matches-event-name")?;
    let event_series_selector = Selector::parse("div.match-h2h-matches-event-series")?;
    let score_rf_selector = Selector::parse("span.rf")?;
    let score_ra_selector = Selector::parse("span.ra")?;
    let date_selector = Selector::parse("div.match-h2h-matches-date")?;

    let matches = document
        .select(&item_selector)
        .filter_map(|e| {
            let href = e.value().attr("href").unwrap_or_default();
            let href_trimmed = href.strip_prefix('/').unwrap_or(href);
            let (match_id_str, match_slug) = href_trimmed.split_once('/').unwrap_or(("", ""));
            let match_id = match_id_str.parse::<u32>().ok()?;
            let match_slug = match_slug.to_string();

            let event_icon = e
                .select(&event_icon_selector)
                .next()
                .and_then(|img| img.value().attr("src"))
                .map(normalize_img_url)
                .unwrap_or_default();
            let event_name = select_text(&e, &event_name_selector);
            let event_series = select_text(&e, &event_series_selector);

            let rf_el = e.select(&score_rf_selector).next()?;
            let ra_el = e.select(&score_ra_selector).next()?;
            let team1_score: u8 = rf_el
                .text()
                .next()
                .unwrap_or_default()
                .trim()
                .parse()
                .ok()?;
            let team2_score: u8 = ra_el
                .text()
                .next()
                .unwrap_or_default()
                .trim()
                .parse()
                .ok()?;

            let winner_index = if rf_el
                .value()
                .has_class("mod-win", CaseSensitivity::CaseSensitive)
            {
                0u8
            } else {
                1u8
            };

            let date = select_text(&e, &date_selector);

            Some(HeadToHeadMatch {
                match_id,
                match_slug,
                event_name,
                event_series,
                event_icon,
                team1_score,
                team2_score,
                winner_index,
                date,
            })
        })
        .collect_vec();

    Ok(matches)
}

fn parse_past_matches(header: &MatchHeader, document: &ElementRef) -> Result<Vec<TeamPastMatches>> {
    let card_selector = Selector::parse("div.match-histories")?;
    let item_selector = Selector::parse("a.match-histories-item")?;
    let score_rf_selector = Selector::parse("span.rf")?;
    let score_ra_selector = Selector::parse("span.ra")?;
    let opponent_name_selector = Selector::parse("span.match-histories-item-opponent-name")?;
    let opponent_logo_selector = Selector::parse("img.match-histories-item-opponent-logo")?;
    let date_selector = Selector::parse("div.match-histories-item-date")?;

    let past_matches = document
        .select(&card_selector)
        .enumerate()
        .map(|(i, card)| {
            let team_id = header.teams.get(i).map(|t| t.id).unwrap_or_default();
            let matches = card
                .select(&item_selector)
                .filter_map(|e| {
                    let href = e.value().attr("href").unwrap_or_default();
                    let href_trimmed = href.strip_prefix('/').unwrap_or(href);
                    let (match_id_str, match_slug) =
                        href_trimmed.split_once('/').unwrap_or(("", ""));
                    let match_id = match_id_str.parse::<u32>().ok()?;
                    let match_slug = match_slug.to_string();

                    let score_for: u8 = e
                        .select(&score_rf_selector)
                        .next()?
                        .text()
                        .next()
                        .unwrap_or_default()
                        .trim()
                        .parse()
                        .ok()?;
                    let score_against: u8 = e
                        .select(&score_ra_selector)
                        .next()?
                        .text()
                        .next()
                        .unwrap_or_default()
                        .trim()
                        .parse()
                        .ok()?;

                    let is_win = e
                        .value()
                        .has_class("mod-win", CaseSensitivity::CaseSensitive);

                    let opponent_name = select_text(&e, &opponent_name_selector);
                    let opponent_logo = e
                        .select(&opponent_logo_selector)
                        .next()
                        .and_then(|img| img.value().attr("src"))
                        .map(normalize_img_url)
                        .unwrap_or_default();

                    let date = select_text(&e, &date_selector);

                    Some(PastMatch {
                        match_id,
                        match_slug,
                        score_for,
                        score_against,
                        is_win,
                        opponent_name,
                        opponent_logo,
                        date,
                    })
                })
                .collect_vec();

            TeamPastMatches { team_id, matches }
        })
        .collect_vec();

    Ok(past_matches)
}

/// Build a name→id lookup from all players in the match games.
fn build_player_name_map(m: &Match) -> std::collections::HashMap<String, u32> {
    let mut map = std::collections::HashMap::new();
    for game in &m.games {
        for team in &game.teams {
            for player in &team.players {
                if player.id != 0 && !player.name.is_empty() {
                    map.insert(player.name.clone(), player.id);
                }
            }
        }
    }
    map
}

fn parse_performance(document: &ElementRef, m: &Match) -> Result<MatchPerformance> {
    let name_map = build_player_name_map(m);

    // The "all" game section contains the aggregated performance tables
    let all_game_selector = Selector::parse("div.vm-stats div.vm-stats-game[data-game-id='all']")?;
    let all_game = document
        .select(&all_game_selector)
        .next()
        .ok_or(VlrError::ElementNotFound {
            context: "performance all-game section",
        })?;

    // --- Kill Matrix (table.mod-normal) ---
    let matrix_selector = Selector::parse("table.mod-normal")?;
    let matrix_table =
        all_game
            .select(&matrix_selector)
            .next()
            .ok_or(VlrError::ElementNotFound {
                context: "kill matrix table (table.mod-normal)",
            })?;

    let row_selector = Selector::parse("tbody tr")?;
    let cell_selector = Selector::parse("td")?;
    let stats_sq_selector = Selector::parse("div.stats-sq")?;
    let team_div_selector = Selector::parse("div.team > div")?;

    let rows: Vec<ElementRef> = matrix_table.select(&row_selector).collect();

    // First row = column headers (victim players)
    let victim_names: Vec<String> = rows
        .first()
        .map(|row| {
            row.select(&cell_selector)
                .skip(1) // skip empty corner cell
                .map(|cell| {
                    select_text(&cell, &team_div_selector)
                })
                .collect()
        })
        .unwrap_or_default();

    let victim_ids: Vec<u32> = victim_names
        .iter()
        .map(|name| name_map.get(name).copied().unwrap_or(0))
        .collect();

    // Data rows (row 1+) = killer players
    let mut kill_matrix = Vec::new();
    for row in rows.iter().skip(1) {
        let cells: Vec<ElementRef> = row.select(&cell_selector).collect();
        if cells.is_empty() {
            continue;
        }

        let killer_name = select_text(&cells[0], &team_div_selector);
        let killer_id = name_map.get(&killer_name).copied().unwrap_or(0);

        for (ci, cell) in cells.iter().skip(1).enumerate() {
            let stat_squares: Vec<String> = cell
                .select(&stats_sq_selector)
                .map(|s| s.text().next().unwrap_or_default().trim().to_string())
                .collect();

            // Each cell has [kills, deaths, diff] - we only need kills and deaths
            let kills: u16 = stat_squares
                .first()
                .and_then(|s| s.parse().ok())
                .unwrap_or(0);
            let deaths: u16 = stat_squares
                .get(1)
                .and_then(|s| s.parse().ok())
                .unwrap_or(0);

            let victim_id = victim_ids.get(ci).copied().unwrap_or(0);

            kill_matrix.push(KillMatrixEntry {
                killer_id,
                victim_id,
                kills,
                deaths,
            });
        }
    }

    // --- Advanced Stats (table.mod-adv-stats) ---
    let adv_selector = Selector::parse("table.mod-adv-stats")?;
    let adv_table = all_game
        .select(&adv_selector)
        .next()
        .ok_or(VlrError::ElementNotFound {
            context: "advanced stats table (table.mod-adv-stats)",
        })?;

    let mut player_performances = Vec::new();
    for row in adv_table.select(&row_selector) {
        let cells: Vec<ElementRef> = row.select(&cell_selector).collect();
        // Player rows have 14 cells: [name, agent, 2K, 3K, 4K, 5K, 1v1, 1v2, 1v3, 1v4, 1v5, ECON, PL, DE]
        if cells.len() < 14 {
            continue;
        }

        let player_name = select_text(&cells[0], &team_div_selector);
        if player_name.is_empty() {
            continue;
        }
        let player_id = name_map.get(&player_name).copied().unwrap_or(0);

        let parse_u8 = |idx: usize| -> u8 {
            cells
                .get(idx)
                .and_then(|c| c.text().next())
                .map(|t| t.trim())
                .and_then(|s| s.parse().ok())
                .unwrap_or(0)
        };
        let parse_u16 = |idx: usize| -> u16 {
            cells
                .get(idx)
                .and_then(|c| c.text().next())
                .map(|t| t.trim())
                .and_then(|s| s.parse().ok())
                .unwrap_or(0)
        };

        player_performances.push(PlayerPerformance {
            player_id,
            player_name,
            multi_kills_2k: parse_u8(2),
            multi_kills_3k: parse_u8(3),
            multi_kills_4k: parse_u8(4),
            multi_kills_5k: parse_u8(5),
            clutch_1v1: parse_u8(6),
            clutch_1v2: parse_u8(7),
            clutch_1v3: parse_u8(8),
            clutch_1v4: parse_u8(9),
            clutch_1v5: parse_u8(10),
            econ_rating: parse_u16(11),
            plants: parse_u8(12),
            defuses: parse_u8(13),
        });
    }

    Ok(MatchPerformance {
        kill_matrix,
        player_performances,
    })
}

fn parse_economy(document: &ElementRef) -> Result<MatchEconomy> {
    let all_game_selector = Selector::parse("div.vm-stats div.vm-stats-game[data-game-id='all']")?;
    let all_game = document
        .select(&all_game_selector)
        .next()
        .ok_or(VlrError::ElementNotFound {
            context: "economy all-game section",
        })?;

    let table_selector = Selector::parse("table.mod-econ")?;
    let table = all_game
        .select(&table_selector)
        .next()
        .ok_or(VlrError::ElementNotFound {
            context: "economy table (table.mod-econ)",
        })?;

    let row_selector = Selector::parse("tbody tr")?;
    let cell_selector = Selector::parse("td")?;
    let stats_sq_selector = Selector::parse("div.stats-sq")?;

    let teams = table
        .select(&row_selector)
        .filter_map(|row| {
            let cells: Vec<ElementRef> = row.select(&cell_selector).collect();
            // Team rows have 6 td cells: [name, pistol_won, eco(won), $(won), $$(won), $$$(won)]
            if cells.len() < 6 {
                return None;
            }

            let team_name = cells[0].text().collect::<String>().trim().to_string();
            if team_name.is_empty() {
                return None;
            }

            let sq_text = |cell: &ElementRef| -> String {
                cell.select(&stats_sq_selector)
                    .next()
                    .map(|s| s.text().collect::<String>().trim().to_string())
                    .unwrap_or_default()
            };

            // Parse "total (won)" format, e.g. "9 (3)" -> (9, 3)
            let parse_rounds_won = |text: &str| -> (u8, u8) {
                // Split on '(' to get "9 " and "3)"
                if let Some((total_str, won_part)) = text.split_once('(') {
                    let rounds: u8 = total_str.trim().parse().unwrap_or(0);
                    let won: u8 = won_part.trim_end_matches(')').trim().parse().unwrap_or(0);
                    (rounds, won)
                } else {
                    (0, 0)
                }
            };

            let pistol_won: u8 = sq_text(&cells[1]).parse().unwrap_or(0);

            let (eco_rounds, eco_won) = parse_rounds_won(&sq_text(&cells[2]));
            let (semi_eco_rounds, semi_eco_won) = parse_rounds_won(&sq_text(&cells[3]));
            let (semi_buy_rounds, semi_buy_won) = parse_rounds_won(&sq_text(&cells[4]));
            let (full_buy_rounds, full_buy_won) = parse_rounds_won(&sq_text(&cells[5]));

            Some(TeamEconomy {
                team_name,
                pistol_won,
                eco_rounds,
                eco_won,
                semi_eco_rounds,
                semi_eco_won,
                semi_buy_rounds,
                semi_buy_won,
                full_buy_rounds,
                full_buy_won,
            })
        })
        .collect_vec();

    Ok(MatchEconomy { teams })
}

fn parse_games(header: &MatchHeader, games: &[ElementRef]) -> Result<Vec<MatchGame>> {
    games.iter().map(|g| parse_game(header, g)).collect()
}

fn parse_game(header: &MatchHeader, game: &ElementRef) -> Result<MatchGame> {
    let map_name_selector =
        Selector::parse("div.vm-stats-game-header div.map div:first-child span")?;
    let map = select_text(game, &map_name_selector);

    let picked_by_selector = Selector::parse("div.vm-stats-game-header div.map span.picked")?;
    let picked_by = game.select(&picked_by_selector).next().and_then(|e| {
        if e.value().has_class("mod-1", CaseSensitivity::CaseSensitive) {
            header.teams.first().map(|t| t.id)
        } else if e.value().has_class("mod-2", CaseSensitivity::CaseSensitive) {
            header.teams.get(1).map(|t| t.id)
        } else {
            None
        }
    });

    let duration_selector = Selector::parse("div.vm-stats-game-header div.map-duration")?;
    let duration = {
        let text = select_text(game, &duration_selector);
        if text.is_empty() {
            None
        } else {
            Some(text)
        }
    };

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
    Ok(MatchGame {
        map,
        picked_by,
        duration,
        teams,
        rounds,
    })
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
        .filter_map(|e| e.value().attr("title"))
        .map(|s| s.to_string())
        .next()
        .unwrap_or_default();

    let stat_cells: Vec<ElementRef> = player.select(&Selector::parse("td.mod-stat")?).collect();

    let stat_both = |cell: Option<&ElementRef>| -> Option<String> {
        cell.and_then(|e| {
            let sel = Selector::parse("span.side.mod-both").unwrap();
            e.select(&sel)
                .next()
                .and_then(|s| s.text().next())
                .map(|t| t.trim().to_string())
        })
    };

    let rating = stat_both(stat_cells.first()).and_then(|s| s.parse::<f32>().ok());
    let acs = stat_both(stat_cells.get(1)).and_then(|s| s.parse::<u16>().ok());
    let kills = stat_both(stat_cells.get(2)).and_then(|s| s.parse::<u16>().ok());
    let deaths = stat_both(stat_cells.get(3)).and_then(|s| s.parse::<u16>().ok());
    let assists = stat_both(stat_cells.get(4)).and_then(|s| s.parse::<u16>().ok());
    let kd_diff = stat_both(stat_cells.get(5)).and_then(|s| s.replace('+', "").parse::<i16>().ok());
    let kast = stat_both(stat_cells.get(6))
        .and_then(|s| s.strip_suffix('%').unwrap_or(&s).parse::<f32>().ok())
        .map(|v| v / 100.0);
    let adr = stat_both(stat_cells.get(7)).and_then(|s| s.parse::<f32>().ok());
    let hs_pct = stat_both(stat_cells.get(8))
        .and_then(|s| s.strip_suffix('%').unwrap_or(&s).parse::<f32>().ok())
        .map(|v| v / 100.0);
    let first_kills = stat_both(stat_cells.get(9)).and_then(|s| s.parse::<u16>().ok());
    let first_deaths = stat_both(stat_cells.get(10)).and_then(|s| s.parse::<u16>().ok());
    let fk_diff =
        stat_both(stat_cells.get(11)).and_then(|s| s.replace('+', "").parse::<i16>().ok());

    Ok(MatchGamePlayer {
        nation,
        id: id.parse().unwrap_or_default(),
        slug,
        name,
        agent,
        rating,
        acs,
        kills,
        deaths,
        assists,
        kd_diff,
        kast,
        adr,
        hs_pct,
        first_kills,
        first_deaths,
        fk_diff,
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

        let events = crate::vlr_scraper::events::list::get_events(
            &client,
            EventType::Completed,
            Region::All,
            1,
        )
        .await
        .unwrap();
        let event_id = events.events[0].id;

        let matches = crate::vlr_scraper::events::matchlist::get_event_matchlist(&client, event_id)
            .await
            .unwrap();
        let match_id = matches[0].id;

        let vlr_match = get_match(&client, match_id).await;
        assert!(vlr_match.is_ok());
    }

    #[tokio::test]
    async fn test_get_match_enhanced_fields() {
        let client = reqwest::Client::new();
        let vlr_match = get_match(&client, 595657).await.unwrap();

        // Header metadata assertions
        assert!(
            !vlr_match.header.patch.is_empty(),
            "patch should be non-empty"
        );
        assert!(
            !vlr_match.header.format.is_empty(),
            "format should be non-empty"
        );
        assert_eq!(vlr_match.header.status, "final");

        // Player stats: at least one player should have kills populated
        let has_player_stats = vlr_match.games.iter().any(|game| {
            game.teams
                .iter()
                .any(|team| team.players.iter().any(|p| p.kills.is_some()))
        });
        assert!(
            has_player_stats,
            "at least one player should have kills populated"
        );

        // Head-to-head entries
        assert!(
            !vlr_match.head_to_head.is_empty(),
            "head_to_head should have entries"
        );

        // Performance data
        assert!(
            vlr_match.performance.is_some(),
            "performance data should be present"
        );

        // Economy data
        assert!(
            vlr_match.economy.is_some(),
            "economy data should be present"
        );

        // Map picks: at least one game should have a pick
        let has_map_pick = vlr_match.games.iter().any(|g| g.picked_by.is_some());
        assert!(has_map_pick, "at least one game should have picked_by set");
    }
}
