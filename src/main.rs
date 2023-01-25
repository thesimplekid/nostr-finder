use nostr_types::{ClientMessage, Event, Filter, Id, SubscriptionId};

use serde::{Deserialize, Serialize};
use serde_json::Value;
use serde_yaml::{self};

use anyhow::Error;

use std::time::Duration;

use tokio::task;
use tokio::time::timeout;
use tungstenite::{connect, Message};

use url::Url;

#[derive(Debug, Serialize, Deserialize)]
struct Relays {
    relays: Vec<String>,
}

async fn get_event_from(
    relay: String,
    filters: Vec<Filter>,
) -> Result<(String, Option<Event>), Error> {
    let subscription_id = SubscriptionId("afsdfsafg".to_string());
    let message = ClientMessage::Req(subscription_id.clone(), filters);
    let (mut socket, _response) = connect(Url::parse(&relay)?)?;

    let message = serde_json::json!(message).to_string(); // serde_json::json!(["REQ", "jhfjsfgdsfg", &filters]).to_string();
    socket.write_message(Message::Text(message))?;

    loop {
        let msg = socket.read_message()?;
        let msg = msg.into_text()?;

        let event: Value = serde_json::from_str(&msg)?;

        if event[0] == "EOSE" && event[1].as_str() == Some(&subscription_id) {
            return Ok((relay.to_string(), None));
        }

        if let Ok(event) = serde_json::from_value::<Event>(event[2].clone()) {
            let message = ClientMessage::Close(subscription_id);
            let message = serde_json::json!(message).to_string(); // serde_json::json!(["REQ", "jhfjsfgdsfg", &filters]).to_string();
            socket.write_message(Message::Text(message))?;
            return Ok((relay.to_string(), Some(event)));
        }
    }
}

//struct Request {}

#[tokio::main]
async fn main() {
    // let event_id = "82dfe7fda41934847c1c068c1116a07b35746df1c789460fa72f5c339e38f952";
    let event_id =
        Id::try_from_hex_string("9dd41abbbfdd7bbc2b014ac0386e3321b8e7c1608e402596ed0b7bb88673a465")
            .unwrap();

    // Get list of relays from taml file
    let f = std::fs::File::open("relays.yaml").expect("Could not open file.");
    let relays: Relays = serde_yaml::from_reader(f).expect("Could not read values.");
    let relays_vec = relays.relays;

    let filters = vec![Filter {
        ids: vec![event_id.into()],
        authors: vec![],
        kinds: vec![],
        e: vec![],
        p: vec![],
        since: None,
        until: None,
        limit: Some(1),
    }];
    let mut handles = vec![];

    println!(
        "Querying {} relays, this could take a bit",
        relays_vec.len()
    );
    for relay in relays_vec.clone() {
        let filters = filters.clone();
        handles.push(task::spawn(get_event_from(relay, filters)));
    }
    let mut results = vec![];
    let timeout_time = Duration::from_secs(30);
    for handle in handles {
        results.push(timeout(timeout_time, handle).await);
    }

    let mut relays_with = vec![];

    let mut event: Option<Event> = None;

    for r in results {
        if let Ok(Ok(r)) = r {
            match r {
                Ok((relay, Some(e))) => {
                    event = Some(e);
                    relays_with.push(relay);
                }
                _ => (),
            }
        }
    }

    let relays_without: Vec<String> = relays_vec
        .into_iter()
        .filter(|x| !relays_with.contains(x))
        .collect();

    let count_with = relays_with.len();
    let count_without = relays_without.len();

    if !relays_without.is_empty() {
        println!("These realys dont have the event");
        for relay in relays_without {
            println!("- {relay}")
        }
    }

    if !relays_with.is_empty() {
        println!("These relays have the event");
        for relay in relays_with {
            println!("- {relay}")
        }
    }

    println!("{count_with}/{count_without} of the relays have the event");
    if let Some(event) = event {
        println!(
            "Event: {} created at: {}",
            event.id.as_hex_string(),
            event.created_at
        );
        println!("Author: {}", event.pubkey.as_hex_string());
        println!("Tags: {:?}", event.tags);
        println!("Content: {}", event.content);
    }
}
