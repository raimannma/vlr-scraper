# vlr-scraper

A Rust library for scraping Valorant esports data from [vlr.gg](https://www.vlr.gg).

## Features

- **Events** -- browse upcoming and completed tournaments, filtered by region
- **Event match lists** -- get all matches for a given event
- **Match details** -- full per-game stats including maps, rounds, players, and agents
- **Players** -- full profiles (info, teams, agent stats, news, event placements) and paginated match history
- **Teams** -- full profiles (info, roster, event placements), paginated match history, and roster transaction history
- **Structured errors** -- every error carries context (URL, element, parse detail)
- **Tracing** -- all operations are instrumented with [`tracing`](https://docs.rs/tracing) spans

## Installation

Add to your `Cargo.toml`:

```toml
[dependencies]
vlr-scraper = { git = "https://github.com/raimannma/vlr-scraper" }
```

You will also need an async runtime such as [Tokio](https://tokio.rs):

```toml
[dependencies]
tokio = { version = "1", features = ["macros", "rt-multi-thread"] }
```

## Quick start

```rust
use vlr_scraper::{VlrClient, EventType, Region};

#[tokio::main]
async fn main() -> vlr_scraper::Result<()> {
    let client = VlrClient::new();

    // Fetch the first page of upcoming events across all regions
    let events = client.get_events(EventType::Upcoming, Region::All, 1).await?;
    println!("Found {} events", events.events.len());

    // Fetch matches for the first event
    let matches = client.get_event_matchlist(events.events[0].id).await?;
    println!("Found {} matches", matches.len());

    // Get detailed stats for a match
    let match_detail = client.get_match(matches[0].id).await?;
    println!("{} vs {}",
        match_detail.header.teams[0].name,
        match_detail.header.teams[1].name,
    );

    Ok(())
}
```

## API overview

All functionality is accessed through [`VlrClient`](src/client.rs):

| Method | Description |
|---|---|
| `get_events(event_type, region, page)` | Paginated list of events |
| `get_event_matchlist(event_id)` | All matches for an event |
| `get_match(match_id)` | Full match detail (header, games, rounds, players) |
| `get_player(player_id, timespan)` | Full player profile (info, teams, agent stats, news, placements) |
| `get_player_matchlist(player_id, page)` | Paginated match history for a player |
| `get_team(team_id)` | Full team profile (info, roster, placements, winnings) |
| `get_team_matchlist(team_id, page)` | Paginated match history for a team |
| `get_team_transactions(team_id)` | Roster transaction history for a team |

### Custom HTTP client

Use `VlrClient::with_client` to supply your own `reqwest::Client` with custom timeouts, proxies, or headers:

```rust
use vlr_scraper::VlrClient;

let http = reqwest::Client::builder()
    .timeout(std::time::Duration::from_secs(10))
    .build()
    .unwrap();

let client = VlrClient::with_client(http);
```

## Error handling

All methods return `vlr_scraper::Result<T>`, which uses the [`VlrError`](src/error.rs) enum:

| Variant | When |
|---|---|
| `Http { url, source }` | Network / DNS / TLS failure |
| `UnexpectedStatus { url, status }` | Non-2xx HTTP response |
| `ResponseBody { url, source }` | Failed to read response body |
| `Selector(String)` | Invalid CSS selector (internal bug) |
| `IntParse(ParseIntError)` | Scraped text couldn't be parsed as integer |
| `DateParse(ParseError)` | Scraped text couldn't be parsed as date |
| `ElementNotFound { context }` | Expected HTML element missing from page |

## Tracing

All public methods and HTTP requests are instrumented with `tracing`. To see logs, add a subscriber in your application:

```rust
// Example using tracing-subscriber
tracing_subscriber::fmt::init();
```

## Project structure

```
src/
├── lib.rs                  # Public API surface and re-exports
├── client.rs               # VlrClient entry point
├── error.rs                # VlrError and Result type alias
├── model/                  # Public data types (pure structs, no logic)
│   ├── common.rs           # Shared types (Social, EventPlacement, PlacementEntry)
│   ├── event.rs            # Event, EventsData, EventType, EventStatus, Region
│   ├── event_matchlist.rs  # EventMatchListItem, EventMatchListTeam
│   ├── match_detail.rs     # Match, MatchHeader, MatchGame, player/round types
│   ├── match_item.rs       # MatchItem, MatchItemTeam (shared match list item)
│   ├── player.rs           # Player, PlayerInfo, PlayerAgentStats, PlayerTeam, ...
│   └── team.rs             # Team, TeamInfo, TeamRosterMember, TeamTransaction
└── vlr_scraper/            # Private HTML parsing (not part of public API)
    ├── mod.rs              # Shared utilities (HTTP fetch, text extraction, URL helpers)
    ├── events/
    │   ├── list.rs         # Event listing parser
    │   └── matchlist.rs    # Event match list parser
    ├── matches/
    │   ├── mod.rs          # Shared match item parsing (used by player/team matchlists)
    │   └── detail.rs       # Full match detail parser
    ├── players/
    │   ├── info.rs         # Player profile parser (info, teams, stats, news, placements)
    │   └── matchlist.rs    # Player match history parser
    └── teams/
        ├── info.rs         # Team profile parser (info, roster, placements)
        ├── matchlist.rs    # Team match history parser
        └── transactions.rs # Team roster transaction parser
```

## License

See [LICENSE](LICENSE) for details.
