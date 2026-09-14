use async_stomp::FromServer;
use async_stomp::client::{Connector, Subscriber};
use std::collections::HashMap;
use uuid::Uuid;
use futures::stream::StreamExt;
use futures::sink::SinkExt;
use anyhow::Result;
use tokio;
use std::time::Duration;

pub type FedoraURIs = HashMap<String, ()>;

pub async fn get_messages(server_url: &str, queue: &str) -> Result<FedoraURIs> {
    let mut conn = Connector::builder()
        .server(server_url)
        .virtualhost("/")
        .heartbeat(2000, 2000)
        //.headers(vec![("heart-beat".to_string(), "1000,1000".to_string())]) // TO DO: Make configurable
        .connect()
        .await?;

    let subscribe = Subscriber::builder()
        .destination(queue)
        .id(Uuid::new_v4().to_string())
        .subscribe();
    conn.send(subscribe).await?;

    let mut fedora_uris = FedoraURIs::new();

    println!("Waiting for messages");

    loop {
        match tokio::time::timeout(Duration::from_millis(10000), conn.next()).await {
            Ok(Some(Ok(message))) => {

                if let FromServer::Message { headers, .. } = message.content {
                    println!("{:?}", headers);

                    let headers: HashMap::<String, String> = HashMap::from_iter(headers); // convert Vec<(String, String)> to HashMap<String, String>
                    if let (Some(identifier), Some(event_type)) = (
                        headers.get("org.fcrepo.jms.identifier"),
                        headers.get("org.fcrepo.jms.eventType"),
                    ) {
                        if event_type == "https://www.w3.org/ns/activitystreams#Create" {
                            fedora_uris.insert(identifier.to_owned(), ()); // (clone the string, so that we can return the HashMap
                        }
                    }
                }
            },
            Ok(Some(Err(e))) => {
                eprintln!("Connection lost: {e}");
                break;
            }
            Ok(None) => {
                println!("Server closed the connection");
                break;
            }
            Err(_) => {
                println!("Timeout: no more messages");
                break;
            }
        }
    }

    println!("Server disconnected");
    Ok(fedora_uris)
}
