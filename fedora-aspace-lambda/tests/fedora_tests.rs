use mockito::Server;
use tokio;
use rstest::{fixture, rstest};
use std::fs;
use fedora_tools::{fedora_api::{self, FedoraResource, HyraxModel}, fedora_auth::FedoraAuth};
use std::path::PathBuf;


#[fixture]
fn uris() -> Vec<(&'static str, &'static str)> {
    vec![("fcrepo/rest/development/cc/49/3b/7e/cc493b7e-b7f4-4783-8d94-569479fb5011", "ArchivalDocument"), ("fcrepo/rest/development/fb/3e/29/53/fb3e2953-2e03-4d05-a13e-44ef5821ead8", "GwWork")]
}

fn load_turtle(uri: &'static str) -> String {
    let file_name = uri.rsplit_once("/").unwrap().1;
    let mut d = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    d.push(format!("tests/data/{file_name}.txt"));
    fs::read_to_string(d).unwrap()
}


#[rstest]
#[tokio::test]
async fn parses_turtle_response(uris: Vec<(&'static str, &'static str)>) {
    let mut server = Server::new_async().await;
    let url = server.url();
    for uri in uris.iter().copied() {
        let turtle_text = load_turtle(uri.0);
        let uri = format!("/{}", uri.0);
        server.mock("GET", uri.as_str())
            .with_body(turtle_text)
            .create_async().await;
    }
    let client = fedora_api::create_client().unwrap();
    let auth = FedoraAuth::new("", "", &url).unwrap();
    for uri in uris.iter().copied() {
        let resource = fedora_api::get_fedora_resource(&client, uri.0, &auth).await.unwrap();
        match uri.1 {
            "ArchivalDocument" => {
                assert!(!resource.is_none());
                match resource {
                    Some(r) => assert_eq!(r, FedoraResource{ uri: uri.0.to_owned(), ref_id: "some-archivesspace-identifier".to_owned(), model: HyraxModel::ArchivalDocument, related_url: None}),
                    None => ()
                }
            },
            _ => assert!(resource.is_none())
        }
    }
}
