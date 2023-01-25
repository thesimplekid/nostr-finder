use nostr_types::{ClientMessage, Event, Filter, Id, SubscriptionId};

use serde::{Deserialize, Serialize};
use serde_json::Value;
use serde_yaml::{self};

use anyhow::Error;

use std::{collections::HashSet, time::Duration};

use tokio::task::JoinSet;
use tokio::time::timeout;

use tungstenite::{connect, Message};

use url::Url;

#[derive(Debug, Serialize, Deserialize)]
struct Relays {
    relays: HashSet<String>,
}

async fn get_event_from(
    relay: String,
    filters: Vec<Filter>,
) -> Result<(String, Option<Event>), Error> {
    let subscription_id = SubscriptionId("afsdfsafg".to_string());
    let message = ClientMessage::Req(subscription_id.clone(), filters);
    let (mut socket, _response) = connect(Url::parse(&relay)?)?;

    let message = serde_json::json!(message).to_string();
    socket.write_message(Message::Text(message))?;

    let handle = tokio::spawn(async move {
        loop {
            let msg = socket.read_message()?;
            let msg = msg.into_text()?;

            let event: Value = serde_json::from_str(&msg)?;

            if event[0] == "EOSE" && event[1].as_str() == Some(&subscription_id) {
                return Ok((relay.to_string(), None));
            }

            if let Ok(event) = serde_json::from_value::<Event>(event[2].clone()) {
                let message = ClientMessage::Close(subscription_id);
                let message = serde_json::json!(message).to_string();
                socket.write_message(Message::Text(message))?;
                return Ok((relay.to_string(), Some(event)));
            }
        }
    });

    handle.await?
}

//struct Request {}

#[tokio::main]
async fn main() -> Result<(), Error> {
    let event_id = "afc9b647c6f7870cdd64954913c48531bfa8df8e1b870976f69332c0c6b58c13";
    let event_id = Id::try_from_hex_string(event_id).unwrap();

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

    let total_relays = relays_vec.len();

    println!("Querying {} relays, this could take a bit", total_relays);
    let mut set = JoinSet::new();
    let timeout_duration = Duration::from_secs(15);
    for relay in relays_vec.clone() {
        let filters = filters.clone();
        set.spawn(timeout(timeout_duration, get_event_from(relay, filters)));
    }
    let mut relays_with = vec![];

    let mut event: Option<Event> = None;

    while let Some(res) = set.join_next().await {
        if let Ok(Ok(r)) = res {
            match r {
                Ok((relay, Some(e))) => {
                    event = Some(e);
                    relays_with.push(relay);
                }
                _ => (),
            }
        }
    }

    set.abort_all();

    let relays_without: Vec<String> = relays_vec
        .into_iter()
        .filter(|x| !relays_with.contains(x))
        .collect();

    let count_with = relays_with.len();

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

    println!("{count_with}/{total_relays} of the relays have the event");
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

    Ok(())
}
