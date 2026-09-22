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

fn parse_stomp_headers(headers: Vec<(String, String)>) -> Option<String> {

    let headers: HashMap::<String, String> = HashMap::from_iter(headers); // convert Vec<(String, String)> to HashMap<String, String>
    let (Some(identifier), Some(event_type)) = (
            headers.get("org.fcrepo.jms.identifier"),
            headers.get("org.fcrepo.jms.eventType"),
    ) else { return None; };

    if event_type != "https://www.w3.org/ns/activitystreams#Create" { return None; };
    Some(identifier.to_owned()) // (clone the string, so that we can return the HashMap

}

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

    loop {
        match tokio::time::timeout(Duration::from_millis(10000), conn.next()).await {
            Ok(Some(Ok(message))) => {
                if let FromServer::Message { headers, .. } = message.content {
                    if let Some(identifier) = parse_stomp_headers(headers) {
                        fedora_uris.insert(identifier, ());
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
    Ok(fedora_uris)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json;
    use std::fs::File;
    use std::io::BufReader;
    use std::path::PathBuf;
    use glob::glob;

    #[test]
    fn filters_for_events() {
        let mut d = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        d.push(format!("tests/data"));
        let path_string = d.to_str().unwrap();
        let path_string = format!("{}/*.json", path_string);
        for path in glob(&path_string).unwrap() {
            match path {
                Ok(path) => {
                    let file = File::open(&path).unwrap();
                    let reader = BufReader::new(file);
                    let message_headers: Vec<(String, String)> = serde_json::from_reader(reader).unwrap();
                    let result = path.file_stem()
                        .and_then(|fs| fs.to_str())
                        .and_then(|key| key.rsplit_once('_'))
                        .and_then(move |(_, tag)| {
                            Some((tag, parse_stomp_headers(message_headers)))
                        });
                    match result {
                        Some((tag, Some(identifier))) => {
                            assert_eq!(tag, "create");
                            assert_eq!(identifier, "/development/4a/0d/8a/83/4a0d8a83-c024-483e-90e7-884274f13b56");
                        },
                        Some((tag, None)) => assert!(tag == "update" || tag == "follow"),
                        None => panic!("Unable to create test match"),
                    }
                },
                Err(error) => eprintln!("{:?}", error),
            }
        }
    }
}
