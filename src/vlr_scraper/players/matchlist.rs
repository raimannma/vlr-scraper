use tracing::{debug, instrument};

use crate::error::Result;
use crate::model::MatchItem;
use crate::vlr_scraper::{self, matches};

#[instrument(skip(client))]
pub(crate) async fn get_player_matchlist(
    client: &reqwest::Client,
    player_id: u32,
    page: u8,
) -> Result<Vec<MatchItem>> {
    let url = format!("https://www.vlr.gg/player/matches/{player_id}/?page={page}");
    let document = vlr_scraper::get_document(client, &url).await?;
    let matches = matches::parse_match_items(&document)?;
    debug!(
        count = matches.len(),
        player_id, page, "parsed player match list"
    );
    Ok(matches)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{EventType, Region};

    #[tokio::test]
    async fn test_get_player_matchlist() {
        let client = reqwest::Client::new();

        let events =
            crate::vlr_scraper::events::list::get_events(&client, EventType::Completed, Region::All, 1)
                .await
                .unwrap();
        let event_id = events.events[0].id;

        let matches = crate::vlr_scraper::events::matchlist::get_event_matchlist(&client, event_id)
            .await
            .unwrap();
        let match_id = matches[0].id;

        let vlr_match = crate::vlr_scraper::matches::detail::get_match(&client, match_id)
            .await
            .unwrap();
        let player_id = vlr_match.games[0].teams[0].players[0].id;

        let player_matchlist = get_player_matchlist(&client, player_id, 1).await.unwrap();
        assert!(!player_matchlist.is_empty());
    }
}
