use itertools::Itertools;
use scraper::Selector;
use tracing::{debug, instrument};

use crate::error::Result;
use crate::model::{EventPlacement, PlacementEntry, Social, Team, TeamInfo, TeamRosterMember};
use crate::vlr_scraper::{self, infer_platform, normalize_img_url, select_text};

#[instrument(skip(client))]
pub(crate) async fn get_team(client: &reqwest::Client, team_id: u32) -> Result<Team> {
    let url = format!("https://www.vlr.gg/team/{team_id}");
    let document = vlr_scraper::get_document(client, &url).await?;

    let info = parse_team_header(&document, team_id)?;
    let roster = parse_roster(&document)?;
    let (event_placements, total_winnings) = parse_event_placements(&document)?;

    debug!(team_id, name = %info.name, "parsed team profile");

    Ok(Team {
        info,
        roster,
        event_placements,
        total_winnings,
    })
}

fn parse_team_header(document: &scraper::Html, team_id: u32) -> Result<TeamInfo> {
    let header_selector = Selector::parse(".team-header")?;
    let header = document
        .select(&header_selector)
        .next()
        .ok_or(crate::error::VlrError::ElementNotFound {
            context: "team header",
        })?;

    // Name from h1.wf-title inside .team-header
    let name_selector = Selector::parse("h1.wf-title")?;
    let name = select_text(&header, &name_selector);

    // Tag from h2.wf-title.team-header-tag
    let tag_selector = Selector::parse("h2.wf-title.team-header-tag")?;
    let tag = {
        let text = select_text(&header, &tag_selector);
        if text.is_empty() {
            None
        } else {
            Some(text)
        }
    };

    // Logo from .team-header-logo img
    let logo_selector = Selector::parse(".team-header-logo img")?;
    let logo_url = header
        .select(&logo_selector)
        .next()
        .and_then(|e| e.value().attr("src"))
        .map(normalize_img_url);

    // Country text from .team-header-country
    let country_selector = Selector::parse(".team-header-country")?;
    let country = {
        let text = header
            .select(&country_selector)
            .next()
            .map(|e| {
                e.text()
                    .map(|t| t.trim())
                    .filter(|t| !t.is_empty())
                    .collect::<Vec<_>>()
                    .join(" ")
            })
            .unwrap_or_default();
        if text.is_empty() {
            None
        } else {
            Some(text)
        }
    };

    // Country code from .team-header-country i.flag class mod-{code}
    let flag_selector = Selector::parse(".team-header-country i.flag")?;
    let country_code = header.select(&flag_selector).next().and_then(|e| {
        e.value()
            .classes()
            .find(|c| c.starts_with("mod-"))
            .map(|c| c.strip_prefix("mod-").unwrap_or_default().to_string())
    });

    // Socials from .team-header-links a
    let links_selector = Selector::parse(".team-header-links a")?;
    let socials = header
        .select(&links_selector)
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
            Some(Social {
                platform,
                url: href,
                display_text,
            })
        })
        .collect();

    Ok(TeamInfo {
        id: team_id,
        name,
        tag,
        logo_url,
        country,
        country_code,
        socials,
    })
}

fn parse_roster(document: &scraper::Html) -> Result<Vec<TeamRosterMember>> {
    let item_selector = Selector::parse(".team-roster-item")?;
    let link_selector = Selector::parse("a[href]")?;
    let alias_selector = Selector::parse(".team-roster-item-name-alias")?;
    let real_name_selector = Selector::parse(".team-roster-item-name-real")?;
    let flag_selector = Selector::parse("i.flag")?;
    let img_selector = Selector::parse(".team-roster-item-img img")?;
    let star_selector = Selector::parse("i.fa-star")?;
    let role_selector = Selector::parse(".team-roster-item-name-role")?;

    let roster = document
        .select(&item_selector)
        .filter_map(|item| {
            let link = item.select(&link_selector).next()?;
            let href = link.value().attr("href")?.trim().to_string();

            // Parse /player/{id}/{slug}
            let (id, slug) = href
                .strip_prefix("/player/")
                .and_then(|s| s.split('/').collect_tuple())
                .map(|(id, slug): (&str, &str)| {
                    (id.parse::<u32>().unwrap_or_default(), slug.to_string())
                })?;

            // Alias: text content of .team-roster-item-name-alias, excluding child element text
            let alias = item
                .select(&alias_selector)
                .next()
                .map(|el| {
                    el.text()
                        .map(|t| t.trim())
                        .filter(|t| !t.is_empty())
                        .collect::<Vec<_>>()
                        .join("")
                })
                .unwrap_or_default();

            // Real name
            let real_name = {
                let text = item
                    .select(&real_name_selector)
                    .next()
                    .map(|el| {
                        el.text()
                            .map(|t| t.trim())
                            .filter(|t| !t.is_empty())
                            .collect::<Vec<_>>()
                            .join("")
                    })
                    .unwrap_or_default();
                if text.is_empty() {
                    None
                } else {
                    Some(text)
                }
            };

            // Country code from flag class
            let country_code = item.select(&flag_selector).next().and_then(|e| {
                e.value()
                    .classes()
                    .find(|c| c.starts_with("mod-"))
                    .map(|c| c.strip_prefix("mod-").unwrap_or_default().to_string())
            });

            // Avatar
            let avatar_url = item
                .select(&img_selector)
                .next()
                .and_then(|e| e.value().attr("src"))
                .map(normalize_img_url);

            // Captain star
            let is_captain = item.select(&star_selector).next().is_some();

            // Role from .team-roster-item-name-role, defaulting to "player"
            let role = {
                let text = item
                    .select(&role_selector)
                    .next()
                    .map(|el| {
                        el.text()
                            .map(|t| t.trim())
                            .filter(|t| !t.is_empty())
                            .collect::<Vec<_>>()
                            .join("")
                    })
                    .unwrap_or_default();
                if text.is_empty() {
                    "player".to_string()
                } else {
                    text
                }
            };

            Some(TeamRosterMember {
                id,
                slug,
                href,
                alias,
                real_name,
                country_code,
                avatar_url,
                role,
                is_captain,
            })
        })
        .collect();

    Ok(roster)
}

fn parse_event_placements(
    document: &scraper::Html,
) -> Result<(Vec<EventPlacement>, Option<String>)> {
    let event_item_selector = Selector::parse("a.team-event-item")?;
    let event_name_selector = Selector::parse("div.text-of")?;
    let series_selector = Selector::parse("span.team-event-item-series")?;
    let prize_selector = Selector::parse("span[style]")?;
    let winnings_label_selector = Selector::parse("div.wf-module-label")?;

    // Total winnings: find the .wf-module-label with "Total Winnings" text, then get the sibling span
    let total_winnings = document
        .select(&winnings_label_selector)
        .find(|el| {
            el.text()
                .map(|t| t.trim())
                .collect::<String>()
                .contains("Total Winnings")
        })
        .and_then(|label| {
            label
                .next_siblings()
                .filter_map(scraper::ElementRef::wrap)
                .next()
        })
        .map(|span| {
            span.text()
                .map(|t| t.trim())
                .filter(|t| !t.is_empty())
                .collect::<String>()
        })
        .filter(|s| !s.is_empty());

    let placements = document
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
                })?;

            let event_name = select_text(&a, &event_name_selector);

            // Year from last child div
            let year: String = a
                .children()
                .filter_map(scraper::ElementRef::wrap)
                .last()
                .map(|d| {
                    d.text()
                        .map(|t| t.trim())
                        .filter(|t| !t.is_empty())
                        .collect::<String>()
                })
                .unwrap_or_default();

            // Stage + placement from span.team-event-item-series
            let stage_placement = a
                .select(&series_selector)
                .next()
                .map(|s| {
                    s.text()
                        .map(|t| t.trim())
                        .filter(|t| !t.is_empty())
                        .collect::<Vec<_>>()
                        .join(" ")
                })
                .unwrap_or_default();

            // Split on en-dash '–'
            let (stage, placement) = if let Some((s, p)) = stage_placement.split_once('–') {
                (s.trim().to_string(), p.trim().to_string())
            } else {
                (stage_placement, String::new())
            };

            // Prize from span with font-weight: 700
            let prize = a
                .select(&prize_selector)
                .find(|s| {
                    s.value()
                        .attr("style")
                        .map(|st| st.contains("font-weight"))
                        .unwrap_or(false)
                })
                .map(|s| {
                    s.text()
                        .map(|t| t.trim())
                        .filter(|t| !t.is_empty())
                        .collect::<String>()
                })
                .filter(|s| !s.is_empty());

            let entry = PlacementEntry {
                stage,
                placement,
                prize,
                team_name: None,
            };

            Some(EventPlacement {
                event_id,
                event_slug,
                event_href: href,
                event_name,
                placements: vec![entry],
                year,
            })
        })
        .collect();

    Ok((placements, total_winnings))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_get_team() {
        let client = reqwest::Client::new();
        let team = get_team(&client, 6530).await.unwrap();

        // Team info
        assert_eq!(team.info.name, "G2 Gozen");
        assert_eq!(team.info.tag, Some("G2G".to_string()));
        assert_eq!(team.info.country, Some("Europe".to_string()));

        // Roster is non-empty
        assert!(!team.roster.is_empty());

        // At least one player role and at least one staff role
        assert!(
            team.roster.iter().any(|m| m.role == "player"),
            "expected at least one roster member with role 'player'"
        );
        assert!(
            team.roster.iter().any(|m| m.role != "player"),
            "expected at least one roster member with a staff role"
        );

        // Event placements are non-empty
        assert!(!team.event_placements.is_empty());

        // Total winnings is present and non-empty
        assert!(
            team.total_winnings.is_some(),
            "expected total_winnings to be Some"
        );
        assert!(
            !team.total_winnings.as_ref().unwrap().is_empty(),
            "expected total_winnings to be non-empty"
        );
    }
}
