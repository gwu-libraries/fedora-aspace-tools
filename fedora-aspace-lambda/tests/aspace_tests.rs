use aspace_tools::aspace_api::{ArchivesSpaceClient, ArchivesSpaceUpdate};
use rstest::{fixture, rstest};
use aspace_tools::aspace_objects::{Auth};
use fedora_tools::fedora_api::{HyraxModel,FedoraResource};
use mockito::{Server, ServerOpts};
use tokio;
use std::collections::HashMap;
use std::path::PathBuf;
use std::fs;
use std::sync::Arc;
use reqwest::Url;
use anyhow::anyhow;
use serde_json::json;

#[fixture]
fn path() -> PathBuf {
    let mut d = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    d.push(format!("tests/data"));
    d
}

#[fixture]
fn archival_objects(path: PathBuf) -> HashMap<&'static str, String> {
    let json_str = fs::read_to_string(path.join("archival-object.json")).unwrap();
    let (first, last) = json_str.split_once("|instances|").unwrap();
    let analog_instance = fs::read_to_string(path.join("archival-object-analog-instance.json")).unwrap();
    let digital_instance = fs::read_to_string(path.join("archival-object-digital-instance.json")).unwrap();
    let mut test_objs = HashMap::new();
    test_objs.insert("no-instances", first.to_string() + last);
    test_objs.insert("analog-instance", first.to_string() + &analog_instance + last);
    test_objs.insert("digital-instance", first.to_string() + &analog_instance + ", " + &digital_instance + last);
    test_objs
}

#[fixture]
fn aspace_session(path: PathBuf) -> String {
    let data = fs::read_to_string(path.join("aspace-session.txt")).unwrap();
    data
}

#[fixture]
fn digital_objects(path: PathBuf) -> (&'static str, String, String) {
    let original = fs::read_to_string(path.join("digital-object.json")).unwrap();
    let updated = fs::read_to_string(path.join("digital-object-updated.json")).unwrap();
    ("/repositories/2/digital_objects/7", original, updated)
}

#[fixture]
fn fedora_resource() -> FedoraResource {
    FedoraResource {
        uri: "test_uri".to_owned(),
        model: HyraxModel::ArchivalDocument,
        ref_id: "id".to_owned(),
        related_url: None,
    }
}


#[rstest]
#[tokio::test]
async fn handles_workflow(aspace_session: String,
                            archival_objects: HashMap<&'static str, String>,
                            mut fedora_resource: FedoraResource,
                            digital_objects: (&'static str, String, String)) {
    let mut server = Server::new_async().await;
    server.mock("POST", format!("/{}", "users/admin/login").as_str())
            .with_body(&aspace_session)
            .create_async().await;
    let url = server.url();

    let auth = Auth::new("admin", "admin", &url).unwrap();
    let client = ArchivesSpaceClient::new(auth).await.unwrap();
    assert!(client.session.session().starts_with("a0385"));

    for (url, data) in archival_objects.iter() {
        server.mock("GET", format!("/{}",url).as_str())
                .with_body(data)
                .create_async().await;
    }
    let (digital_object_uri, digital_object_original, digital_object_updated) = digital_objects;
    server.mock("GET", digital_object_uri)
        .with_body(digital_object_original.clone())
        .create_async().await;

    server.mock("POST", digital_object_uri)
        .match_body(mockito::Matcher::Regex(fedora_resource.uri.clone()))
        .create_async()
        .await;

    for (url, _) in archival_objects {
        let _ = fedora_resource.related_url.insert(url.to_owned());
        let mut aspace_update = ArchivesSpaceUpdate::new(client.clone(), &fedora_resource).unwrap();
        let result = aspace_update.get_object().await;
        assert!(result.is_ok());
        match url {
            "no-instances" | "analog-instance" => {
                let r = aspace_update.extract_digital_object_ref();
                assert!(r.is_err_and(|e| format!("{}", e) == "No digital object identifier found in archival object"));

            },
            "digital-instance"=> {
                assert!(aspace_update.extract_digital_object_ref().is_ok());
                let result = aspace_update.get_object().await;
                assert!(result.is_ok());
                let result = aspace_update.update_digital_object().await;
                assert!(result.is_ok());
            },
            _ => {}
        }
    }

}
