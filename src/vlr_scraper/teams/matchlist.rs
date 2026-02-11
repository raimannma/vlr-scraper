use tracing::{debug, instrument};

use crate::error::Result;
use crate::model::MatchItem;
use crate::vlr_scraper::{self, matches};

#[instrument(skip(client))]
pub(crate) async fn get_team_matchlist(
    client: &reqwest::Client,
    team_id: u32,
    page: u8,
) -> Result<Vec<MatchItem>> {
    let url = format!("https://www.vlr.gg/team/matches/{team_id}/?page={page}");
    let document = vlr_scraper::get_document(client, &url).await?;
    let matches = matches::parse_match_items(&document)?;
    debug!(
        count = matches.len(),
        team_id, page, "parsed team match list"
    );
    Ok(matches)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_get_team_matchlist() {
        let client = reqwest::Client::new();
        let matches = get_team_matchlist(&client, 6530, 1).await.unwrap();

        assert!(!matches.is_empty());

        let first = &matches[0];
        assert!(first.id > 0);
        assert!(!first.league_name.is_empty());
        assert_eq!(first.teams.len(), 2);
        assert!(!first.teams[0].name.is_empty());
        assert!(!first.teams[1].name.is_empty());
    }

    #[tokio::test]
    async fn test_get_team_matchlist_page2() {
        let client = reqwest::Client::new();
        let matches = get_team_matchlist(&client, 6530, 2).await.unwrap();

        assert!(!matches.is_empty());
    }
}
