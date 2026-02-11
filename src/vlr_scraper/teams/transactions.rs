use chrono::NaiveDate;
use itertools::Itertools;
use scraper::{ElementRef, Selector};
use tracing::{debug, instrument};

use crate::error::Result;
use crate::model::TeamTransaction;
use crate::vlr_scraper;

#[instrument(skip(client))]
pub(crate) async fn get_team_transactions(
    client: &reqwest::Client,
    team_id: u32,
) -> Result<Vec<TeamTransaction>> {
    let url = format!("https://www.vlr.gg/team/transactions/{team_id}/");
    let document = vlr_scraper::get_document(client, &url).await?;
    let transactions = parse_transactions(&document)?;
    debug!(
        count = transactions.len(),
        team_id, "parsed team transactions"
    );
    Ok(transactions)
}

fn parse_transactions(document: &scraper::Html) -> Result<Vec<TeamTransaction>> {
    let row_selector = Selector::parse("tr.txn-item")?;
    document
        .select(&row_selector)
        .map(|row| parse_transaction_row(&row))
        .collect()
}

fn parse_transaction_row(element: &ElementRef) -> Result<TeamTransaction> {
    let td_selector = Selector::parse("td")?;
    let tds: Vec<ElementRef> = element.select(&td_selector).collect();

    // 1. Date from first <td>
    let date = tds.first().and_then(|td| {
        let text: String = td
            .text()
            .map(|t| t.trim())
            .filter(|t| !t.is_empty())
            .collect();
        if text == "Unknown" || text.is_empty() {
            None
        } else {
            NaiveDate::parse_from_str(&text, "%Y/%m/%d").ok()
        }
    });

    // 2. Action from <td> with class txn-item-action
    let action_selector = Selector::parse("td.txn-item-action")?;
    let action = element
        .select(&action_selector)
        .next()
        .map(|td| {
            td.text()
                .map(|t| t.trim())
                .filter(|t| !t.is_empty())
                .collect::<String>()
        })
        .unwrap_or_default();

    // 3. Country code from <i> with class starting with "flag"
    let flag_selector = Selector::parse("i.flag")?;
    let player_country_code = element.select(&flag_selector).next().and_then(|e| {
        e.value()
            .classes()
            .find(|c| c.starts_with("mod-"))
            .map(|c| c.strip_prefix("mod-").unwrap_or_default().to_string())
    });

    // 4. Player from <a> in player <td>
    let link_selector = Selector::parse("a[href^=\"/player/\"]")?;
    let real_name_selector = Selector::parse("div.ge-text-light")?;

    let player_link = element.select(&link_selector).next();

    let (player_id, player_slug, player_alias) = player_link
        .map(|a| {
            let href = a.value().attr("href").unwrap_or_default();
            let (id, slug) = href
                .strip_prefix("/player/")
                .and_then(|s| s.split('/').collect_tuple())
                .map(|(id, slug): (&str, &str)| {
                    (id.parse::<u32>().unwrap_or_default(), slug.to_string())
                })
                .unwrap_or_default();
            let alias: String = a
                .text()
                .map(|t| t.trim())
                .filter(|t| !t.is_empty())
                .collect();
            (id, slug, alias)
        })
        .unwrap_or_default();

    let player_real_name = element
        .select(&real_name_selector)
        .next()
        .map(|el| {
            el.text()
                .map(|t| t.trim())
                .filter(|t| !t.is_empty())
                .collect::<String>()
        })
        .filter(|s| !s.is_empty());

    // 5. Position from 5th <td> (index 4)
    let position = tds
        .get(4)
        .map(|td| {
            td.text()
                .map(|t| t.trim())
                .filter(|t| !t.is_empty())
                .collect::<String>()
        })
        .unwrap_or_default();

    // 6. Reference URL from last <td>
    let ref_link_selector = Selector::parse("a[href]")?;
    let reference_url = tds.last().and_then(|td| {
        td.select(&ref_link_selector).next().and_then(|a| {
            let href = a.value().attr("href").unwrap_or_default().trim();
            if href.is_empty() {
                None
            } else {
                Some(href.to_string())
            }
        })
    });

    Ok(TeamTransaction {
        date,
        action,
        player_id,
        player_slug,
        player_alias,
        player_real_name,
        player_country_code,
        position,
        reference_url,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_get_team_transactions() {
        let client = reqwest::Client::new();
        let transactions = get_team_transactions(&client, 6530).await.unwrap();

        assert!(!transactions.is_empty());

        let first = &transactions[0];
        assert!(
            first.date.is_some(),
            "expected first transaction to have a date"
        );
        assert!(
            first.action == "join" || first.action == "leave" || first.action == "inactive",
            "expected action to be join, leave, or inactive, got: {}",
            first.action
        );
        assert!(!first.player_alias.is_empty());
        assert!(!first.position.is_empty());
        assert!(first.player_id > 0);
    }
}
