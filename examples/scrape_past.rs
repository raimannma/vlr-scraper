use tokio::time::sleep;

use vlr_scraper::enums::Region;
use vlr_scraper::events::EventType;
use vlr_scraper::{get_events, get_match, get_matchlist};

#[tokio::main]
async fn main() {
    let client = reqwest::Client::new();

    let mut page = 0;
    let mut all_events = vec![];
    loop {
        page += 1;
        println!("Getting page {page}");
        let events = get_events(&client, EventType::Completed, Region::All, page)
            .await
            .unwrap();
        if events.events.is_empty() {
            break;
        }
        all_events.extend(events.events);
        sleep(std::time::Duration::from_millis(100)).await;
    }
    println!("Found {} events", all_events.len());

    for event in all_events {
        println!("Getting matches for event {}", event.id);
        if std::fs::exists(format!("events/{}/event.json", event.id)).unwrap_or_default() {
            continue;
        }
        sleep(std::time::Duration::from_millis(100)).await;
        std::fs::create_dir_all(format!("events/{}/matches", event.id)).unwrap();
        serde_json::to_writer_pretty(
            std::fs::File::create(format!("events/{}/event.json", event.id)).unwrap(),
            &event,
        )
        .unwrap();
        let matches = get_matchlist(&client, event.id).await.unwrap();
        serde_json::to_writer_pretty(
            std::fs::File::create(format!("events/{}/matches.json", event.id)).unwrap(),
            &matches,
        )
        .unwrap();
        for match_item in matches {
            let match_data = get_match(&client, match_item.id).await.unwrap();
            serde_json::to_writer_pretty(
                std::fs::File::create(format!(
                    "events/{}/matches/{}.json",
                    event.id, match_data.id
                ))
                .unwrap(),
                &match_data,
            )
            .unwrap();
        }
    }
}
