use chrono::{NaiveDate, NaiveTime};
use itertools::{izip, Itertools};
use scraper::{ElementRef, Selector};
use tracing::{debug, instrument};

use crate::error::{Result, VlrError};
use crate::model::{
    AgentStatsTimespan, Player, PlayerAgentStats, PlayerEventPlacement, PlayerInfo,
    PlayerMatchList, PlayerMatchListItem, PlayerMatchListTeam, PlayerNewsItem,
    PlayerPlacementEntry, PlayerSocial, PlayerTeam,
};
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

/// Fetch a complete player profile: basic info, teams, agent stats, news, and event placements.
#[instrument(skip(client))]
pub(crate) async fn get_player(
    client: &reqwest::Client,
    player_id: u32,
    timespan: AgentStatsTimespan,
) -> Result<Player> {
    let overview_url = format!("https://www.vlr.gg/player/{player_id}/?timespan={timespan}");

    // Fetch the overview page and agent stats concurrently
    let overview_doc = scraper::get_document(client, &overview_url).await?;

    let (info, current_teams, past_teams) = parse_player_overview(&overview_doc, player_id)?;
    let news = parse_player_news(&overview_doc)?;
    let (event_placements, total_winnings) = parse_event_placements(&overview_doc)?;
    let agent_stats = parse_agent_stats(&overview_doc)?;

    debug!(player_id, name = %info.name, "parsed player profile");

    Ok(Player {
        info,
        current_teams,
        past_teams,
        agent_stats,
        news,
        event_placements,
        total_winnings,
    })
}

/// Parse agent stats from the table on a player overview page.
pub(crate) fn parse_agent_stats(document: &scraper::Html) -> Result<Vec<PlayerAgentStats>> {
    let row_selector = Selector::parse("table.wf-table tbody tr")?;
    let td_selector = Selector::parse("td")?;
    let img_selector = Selector::parse("img")?;

    document
        .select(&row_selector)
        .map(|row| {
            let cells: Vec<ElementRef> = row.select(&td_selector).collect();
            if cells.len() < 17 {
                return Err(VlrError::ElementNotFound {
                    context: "agent stats row: expected 17 columns",
                });
            }

            // Agent name from img alt attribute
            let agent = cells[0]
                .select(&img_selector)
                .next()
                .and_then(|img| img.value().attr("alt"))
                .unwrap_or_default()
                .to_string();

            // Usage: "(95) 20%" -> count=95, pct=0.20
            let use_text = cell_text(&cells[1]);
            let (usage_count, usage_pct) = parse_usage(&use_text);

            let rounds = parse_u32(&cell_text(&cells[2]));
            let rating = parse_f32(&cell_text(&cells[3]));
            let acs = parse_f32(&cell_text(&cells[4]));
            let kd = parse_f32(&cell_text(&cells[5]));
            let adr = parse_f32(&cell_text(&cells[6]));
            let kast = parse_pct(&cell_text(&cells[7]));
            let kpr = parse_f32(&cell_text(&cells[8]));
            let apr = parse_f32(&cell_text(&cells[9]));
            let fkpr = parse_f32(&cell_text(&cells[10]));
            let fdpr = parse_f32(&cell_text(&cells[11]));
            let kills = parse_u32(&cell_text(&cells[12]));
            let deaths = parse_u32(&cell_text(&cells[13]));
            let assists = parse_u32(&cell_text(&cells[14]));
            let first_kills = parse_u32(&cell_text(&cells[15]));
            let first_deaths = parse_u32(&cell_text(&cells[16]));

            Ok(PlayerAgentStats {
                agent,
                usage_count,
                usage_pct,
                rounds,
                rating,
                acs,
                kd,
                adr,
                kast,
                kpr,
                apr,
                fkpr,
                fdpr,
                kills,
                deaths,
                assists,
                first_kills,
                first_deaths,
            })
        })
        .collect()
}

/// Extract trimmed text from a table cell.
fn cell_text(el: &ElementRef) -> String {
    el.text()
        .map(|t| t.trim())
        .filter(|t| !t.is_empty())
        .collect::<Vec<_>>()
        .join("")
}

/// Parse usage text like "(95) 20%" into (count, fraction).
fn parse_usage(text: &str) -> (u32, f32) {
    // Format: "(95) 20%"
    let count = text
        .split(')')
        .next()
        .and_then(|s| s.strip_prefix('('))
        .and_then(|s| s.trim().parse::<u32>().ok())
        .unwrap_or(0);

    let pct = text
        .split(')')
        .nth(1)
        .and_then(|s| s.trim().strip_suffix('%'))
        .and_then(|s| s.trim().parse::<f32>().ok())
        .map(|p| p / 100.0)
        .unwrap_or(0.0);

    (count, pct)
}

/// Parse a percentage string like "77%" into a fraction (0.77).
fn parse_pct(text: &str) -> f32 {
    text.strip_suffix('%')
        .and_then(|s| s.trim().parse::<f32>().ok())
        .map(|p| p / 100.0)
        .unwrap_or(0.0)
}

fn parse_f32(text: &str) -> f32 {
    text.trim().parse().unwrap_or(0.0)
}

fn parse_u32(text: &str) -> u32 {
    text.trim().parse().unwrap_or(0)
}

/// Parse the player overview page and return basic info and team lists.
pub(crate) fn parse_player_overview(
    document: &scraper::Html,
    player_id: u32,
) -> Result<(PlayerInfo, Vec<PlayerTeam>, Vec<PlayerTeam>)> {
    let info = parse_player_info(document, player_id)?;
    let current_teams = parse_teams_section(document, "Current Teams")?;
    let past_teams = parse_teams_section(document, "Past Teams")?;
    Ok((info, current_teams, past_teams))
}

fn parse_player_info(document: &scraper::Html, player_id: u32) -> Result<PlayerInfo> {
    let header_selector = Selector::parse("div.player-header")?;
    let header = document
        .select(&header_selector)
        .next()
        .ok_or(VlrError::ElementNotFound {
            context: "player header",
        })?;

    // Name
    let name_selector = Selector::parse("h1.wf-title")?;
    let name = select_text(&header, &name_selector);

    // Real name
    let real_name_selector = Selector::parse("h2.player-real-name")?;
    let real_name = {
        let text = select_text(&header, &real_name_selector);
        if text.is_empty() {
            None
        } else {
            Some(text)
        }
    };

    // Avatar
    let avatar_selector = Selector::parse("div.wf-avatar img")?;
    let avatar_url = header
        .select(&avatar_selector)
        .next()
        .and_then(|e| e.value().attr("src"))
        .map(normalize_img_url);

    // Country code from <i class="flag mod-{code}">
    let flag_selector = Selector::parse("i.flag")?;
    let country_code = header.select(&flag_selector).next().and_then(|e| {
        e.value()
            .classes()
            .find(|c| c.starts_with("mod-"))
            .map(|c| c.strip_prefix("mod-").unwrap_or_default().to_string())
    });

    // Country name from the text near the flag
    let country_div_selector = Selector::parse("div.ge-text-light")?;
    let country = header
        .select(&country_div_selector)
        .filter_map(|e| {
            let text: String = e
                .text()
                .map(|t| t.trim())
                .filter(|t| !t.is_empty())
                .collect::<Vec<_>>()
                .join(" ");
            if text.is_empty() {
                None
            } else {
                Some(text)
            }
        })
        .last();

    // Social links: plain <a> tags in .player-header with non-empty href and text
    let social_selector = Selector::parse("a")?;
    let socials = header
        .select(&social_selector)
        .filter_map(|a| {
            let href = a
                .value()
                .attr("href")
                .unwrap_or_default()
                .trim()
                .to_string();
            let display_text: String = a
                .text()
                .map(|t| t.trim())
                .filter(|t| !t.is_empty())
                .collect::<Vec<_>>()
                .join("");
            if href.is_empty() || display_text.is_empty() {
                return None;
            }
            let platform = infer_platform(&href);
            Some(PlayerSocial {
                platform,
                url: href,
                display_text,
            })
        })
        .collect();

    Ok(PlayerInfo {
        id: player_id,
        name,
        real_name,
        avatar_url,
        country,
        country_code,
        socials,
    })
}

fn infer_platform(url: &str) -> String {
    let url_lower = url.to_lowercase();
    if url_lower.contains("twitter.com") || url_lower.contains("x.com") {
        "twitter".to_string()
    } else if url_lower.contains("twitch.tv") {
        "twitch".to_string()
    } else if url_lower.contains("instagram.com") {
        "instagram".to_string()
    } else if url_lower.contains("youtube.com") || url_lower.contains("youtu.be") {
        "youtube".to_string()
    } else if url_lower.contains("tiktok.com") {
        "tiktok".to_string()
    } else {
        "other".to_string()
    }
}

fn parse_teams_section(document: &scraper::Html, section_title: &str) -> Result<Vec<PlayerTeam>> {
    let label_selector = Selector::parse("h2.wf-label")?;
    let team_link_selector = Selector::parse("a.wf-module-item")?;

    // Find the h2.wf-label with the matching text, then get teams from its next sibling card
    let label = document.select(&label_selector).find(|el| {
        el.text()
            .map(|t| t.trim())
            .collect::<String>()
            .contains(section_title)
    });

    let label = match label {
        Some(l) => l,
        None => return Ok(Vec::new()),
    };

    // The next sibling element should be the wf-card containing team links
    let card = label.next_siblings().filter_map(ElementRef::wrap).next();

    let card = match card {
        Some(c) => c,
        None => return Ok(Vec::new()),
    };

    let teams = card
        .select(&team_link_selector)
        .filter_map(|a| parse_player_team(a).ok())
        .collect();

    Ok(teams)
}

fn parse_player_team(element: ElementRef) -> Result<PlayerTeam> {
    let href = element.value().attr("href").unwrap_or_default().to_string();

    // Parse /team/{id}/{slug}
    let (id, slug) = href
        .strip_prefix("/team/")
        .and_then(|s| s.split('/').collect_tuple())
        .map(|(id, slug): (&str, &str)| (id.parse::<u32>().unwrap_or_default(), slug.to_string()))
        .unwrap_or_default();

    // Team logo
    let img_selector = Selector::parse("img")?;
    let logo_url = element
        .select(&img_selector)
        .next()
        .and_then(|e| e.value().attr("src"))
        .map(normalize_img_url)
        .unwrap_or_default();

    // Team name: first div with font-weight: 500 style, or first meaningful text
    let name_divs: Vec<_> = element
        .select(&Selector::parse("div")?)
        .filter(|d| {
            d.value()
                .attr("style")
                .map(|s| s.contains("font-weight"))
                .unwrap_or(false)
        })
        .collect();

    let name = if let Some(name_div) = name_divs.first() {
        name_div
            .text()
            .map(|t| t.trim())
            .filter(|t| !t.is_empty())
            .collect::<Vec<_>>()
            .join("")
    } else {
        String::new()
    };

    // Info text (e.g. "joined in October 2021") from div.ge-text-light
    let info_selector = Selector::parse("div.ge-text-light")?;
    let info = element
        .select(&info_selector)
        .filter_map(|e| {
            let text: String = e
                .text()
                .map(|t| t.trim())
                .filter(|t| !t.is_empty())
                .collect::<Vec<_>>()
                .join(" ");
            if text.is_empty() {
                None
            } else {
                Some(text)
            }
        })
        .last();

    Ok(PlayerTeam {
        id,
        slug,
        href,
        name,
        logo_url,
        info,
    })
}

/// Parse the Latest News section from a player overview page.
pub(crate) fn parse_player_news(document: &scraper::Html) -> Result<Vec<PlayerNewsItem>> {
    let label_selector = Selector::parse("h2.wf-label")?;
    let item_selector = Selector::parse("a.wf-module-item")?;
    let date_selector = Selector::parse("div.ge-text-light")?;

    let label = document.select(&label_selector).find(|el| {
        el.text()
            .map(|t| t.trim())
            .collect::<String>()
            .contains("Latest News")
    });

    let label = match label {
        Some(l) => l,
        None => return Ok(Vec::new()),
    };

    let card = label.next_siblings().filter_map(ElementRef::wrap).next();

    let card = match card {
        Some(c) => c,
        None => return Ok(Vec::new()),
    };

    let news = card
        .select(&item_selector)
        .filter_map(|a| {
            let href = a
                .value()
                .attr("href")
                .unwrap_or_default()
                .trim()
                .to_string();
            if href.is_empty() {
                return None;
            }

            let date = select_text(&a, &date_selector);

            // Title is the text in the div with font-weight: 500
            let title: String = a
                .children()
                .filter_map(ElementRef::wrap)
                .filter(|d| {
                    d.value()
                        .attr("style")
                        .map(|s| s.contains("font-weight"))
                        .unwrap_or(false)
                })
                .flat_map(|d| d.text())
                .map(|t| t.trim())
                .filter(|t| !t.is_empty())
                .collect::<Vec<_>>()
                .join("");

            if title.is_empty() {
                return None;
            }

            Some(PlayerNewsItem { href, date, title })
        })
        .collect();

    Ok(news)
}

/// Parse the Event Placements section and total winnings from a player overview page.
pub(crate) fn parse_event_placements(
    document: &scraper::Html,
) -> Result<(Vec<PlayerEventPlacement>, Option<String>)> {
    let label_selector = Selector::parse("h2.wf-label")?;
    let event_item_selector = Selector::parse("a.player-event-item")?;

    let label = document.select(&label_selector).find(|el| {
        el.text()
            .map(|t| t.trim())
            .collect::<String>()
            .contains("Event Placements")
    });

    let label = match label {
        Some(l) => l,
        None => return Ok((Vec::new(), None)),
    };

    let card = label.next_siblings().filter_map(ElementRef::wrap).next();

    let card = match card {
        Some(c) => c,
        None => return Ok((Vec::new(), None)),
    };

    // Total winnings: first span in the header div with font-size: 22px
    let winnings_selector = Selector::parse("span")?;
    let total_winnings = card
        .children()
        .filter_map(ElementRef::wrap)
        .next() // first child div (header)
        .and_then(|header| {
            header
                .select(&winnings_selector)
                .next()
                .map(|s| cell_text(&s))
        })
        .filter(|s| !s.is_empty());

    let event_name_selector = Selector::parse("div.text-of")?;
    let stage_selector = Selector::parse("span.ge-text-light")?;
    let prize_selector = Selector::parse("span[style]")?;

    let placements = card
        .select(&event_item_selector)
        .filter_map(|a| {
            let href = a
                .value()
                .attr("href")
                .unwrap_or_default()
                .trim()
                .to_string();

            // Parse /event/{id}/{slug}
            let (event_id, event_slug) = href
                .strip_prefix("/event/")
                .and_then(|s| s.split('/').collect_tuple())
                .map(|(id, slug): (&str, &str)| {
                    (id.parse::<u32>().unwrap_or_default(), slug.to_string())
                })
                .unwrap_or_default();

            let event_name = select_text(&a, &event_name_selector);

            // Year is in the last child div (not inside the flex: 1 div)
            let year: String = a
                .children()
                .filter_map(ElementRef::wrap)
                .last()
                .map(|d| cell_text(&d))
                .unwrap_or_default();

            // Placement entries: divs inside the flex: 1 container (skip the event name div)
            let flex_container = a.children().filter_map(ElementRef::wrap).next(); // first child div (flex: 1)

            let entries: Vec<PlayerPlacementEntry> = flex_container
                .map(|container| {
                    container
                        .children()
                        .filter_map(ElementRef::wrap)
                        .filter(|d| {
                            // Skip the event name div (has class text-of)
                            !d.value().classes().any(|c| c == "text-of")
                        })
                        .filter_map(|entry_div| {
                            // Stage + placement from span.ge-text-light
                            let stage_placement = entry_div
                                .select(&stage_selector)
                                .next()
                                .map(|s| cell_text(&s))
                                .unwrap_or_default();

                            if stage_placement.is_empty() {
                                return None;
                            }

                            // Split "Playoffs – 1st" into stage and placement
                            let (stage, placement) =
                                if let Some((s, p)) = stage_placement.split_once('–') {
                                    (s.trim().to_string(), p.trim().to_string())
                                } else {
                                    (stage_placement.clone(), String::new())
                                };

                            // Prize from span with font-weight: 700
                            let prize = entry_div
                                .select(&prize_selector)
                                .find(|s| {
                                    s.value()
                                        .attr("style")
                                        .map(|st| st.contains("font-weight"))
                                        .unwrap_or(false)
                                })
                                .map(|s| cell_text(&s))
                                .filter(|s| !s.is_empty());

                            // Team name: text nodes in the entry div that are not inside spans
                            let team_name: String = entry_div
                                .text()
                                .map(|t| t.trim())
                                .filter(|t| !t.is_empty())
                                .collect::<Vec<_>>()
                                .join(" ");

                            // Remove the stage/placement and prize text to get just the team name
                            let team_name =
                                team_name.replace(&stage_placement, "").replace('–', "");
                            let team_name = if let Some(ref p) = prize {
                                team_name.replace(p, "")
                            } else {
                                team_name
                            };
                            let team_name = team_name.trim().to_string();

                            Some(PlayerPlacementEntry {
                                stage,
                                placement,
                                prize,
                                team_name,
                            })
                        })
                        .collect()
                })
                .unwrap_or_default();

            if entries.is_empty() {
                return None;
            }

            Some(PlayerEventPlacement {
                event_id,
                event_slug,
                event_href: href,
                event_name,
                placements: entries,
                year,
            })
        })
        .collect();

    Ok((placements, total_winnings))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{EventType, Region};

    #[tokio::test]
    async fn test_parse_player_overview() {
        let client = reqwest::Client::new();
        let url = "https://www.vlr.gg/player/17323";
        let document = scraper::get_document(&client, url).await.unwrap();
        let (info, current_teams, past_teams) = parse_player_overview(&document, 17323).unwrap();

        assert_eq!(info.name, "mimi");
        assert_eq!(info.id, 17323);
        assert!(info.real_name.is_some());
        assert!(info.avatar_url.is_some());
        assert!(info.country.is_some());
        assert!(info.country_code.is_some());
        assert!(!info.socials.is_empty());

        assert!(!current_teams.is_empty());
        let g2 = &current_teams[0];
        assert!(g2.name.contains("G2 Gozen"));
        assert!(g2.id > 0);
        assert!(!g2.slug.is_empty());

        assert!(!past_teams.is_empty());
    }

    #[tokio::test]
    async fn test_parse_agent_stats() {
        let client = reqwest::Client::new();
        let url = "https://www.vlr.gg/player/17323?timespan=all";
        let document = scraper::get_document(&client, url).await.unwrap();
        let stats = parse_agent_stats(&document).unwrap();

        assert!(!stats.is_empty());
        let first = &stats[0];
        assert!(!first.agent.is_empty());
        assert!(first.usage_count > 0);
        assert!(first.usage_pct > 0.0);
        assert!(first.rounds > 0);
        assert!(first.rating > 0.0);
        assert!(first.kills > 0);
        assert!(first.deaths > 0);
    }

    #[tokio::test]
    async fn test_parse_player_news() {
        let client = reqwest::Client::new();
        let url = "https://www.vlr.gg/player/17323";
        let document = scraper::get_document(&client, url).await.unwrap();
        let news = parse_player_news(&document).unwrap();

        assert!(!news.is_empty());
        let first = &news[0];
        assert!(!first.href.is_empty());
        assert!(!first.date.is_empty());
        assert!(!first.title.is_empty());
    }

    #[tokio::test]
    async fn test_parse_event_placements() {
        let client = reqwest::Client::new();
        let url = "https://www.vlr.gg/player/17323";
        let document = scraper::get_document(&client, url).await.unwrap();
        let (placements, total_winnings) = parse_event_placements(&document).unwrap();

        assert!(total_winnings.is_some());
        assert!(total_winnings.unwrap().contains('$'));

        assert!(!placements.is_empty());
        let first = &placements[0];
        assert!(first.event_id > 0);
        assert!(!first.event_slug.is_empty());
        assert!(!first.event_name.is_empty());
        assert!(!first.year.is_empty());
        assert!(!first.placements.is_empty());

        let entry = &first.placements[0];
        assert!(!entry.stage.is_empty());
        assert!(!entry.placement.is_empty());
        assert!(!entry.team_name.is_empty());

        // The first event (Championship Seoul) has multiple placement entries
        assert!(first.placements.len() >= 2);
    }

    #[tokio::test]
    async fn test_get_player() {
        let client = reqwest::Client::new();
        let player = get_player(&client, 17323, Default::default())
            .await
            .unwrap();

        // Basic info
        assert_eq!(player.info.name, "mimi");
        assert!(
            player
                .info
                .country
                .as_deref()
                .map(|c| c.contains("DENMARK") || c.contains("Denmark"))
                .unwrap_or(false),
            "expected country to contain Denmark, got {:?}",
            player.info.country,
        );

        // Current team
        assert!(!player.current_teams.is_empty());

        // Event placements
        assert!(!player.event_placements.is_empty());
    }

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
