use reqwest::{Client, Url};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use fedora_tools::fedora_api::FedoraResource;
use std::sync::Arc;
use anyhow::{anyhow, Result};

/*
 * Stores a reference to an object in ArchivesSpace, either archival or digital. The archival-object reference may be the URI for getting the object directly, or the archival object reference identifier, which can be used to query the API for the object.
 *
 * The digital-object reference should be the URI for retrieving the digital object directly from the API.
 */
#[derive(Debug)]
pub enum ArchivesSpaceRecordId {
    ArchivalObjectRefId(Url), // if we have only the Archival Object ref Id, this will be a query
    ArchivalObjectURI(Url),
    DigitalObjectURI(Url)
}

/*
 * Used internally to deserialize the response from the ASPace session API
 */
#[derive(Debug, Deserialize)]
struct Session {
     session: String
}

#[derive(Debug)]
pub struct SessionShared {
    session: String
}

/* To be created from ENV; used for creating an ASPace session. The base_url will be cloned across multiple requests. */
#[derive(Debug)]
pub struct Auth {
    pub base_url: Url,
    username: String,
    password: String,
}

/*
 * Used to serialize the reference to a digital object stored in an archival object
 */
#[derive(Debug)]
#[derive(Deserialize)]
pub struct DigitalObjectRef {
    #[serde(rename = "ref")] // ref is Rust keyword
    ref_: String,
}

/*
 * An instance may or may not contain a reference to a digital object
 */
#[derive(Debug)]
#[derive(Deserialize)]
pub struct Instance {
    instance_type: String,
    digital_object: Option<DigitalObjectRef>,
}
/*
 * An archival object will contain 0 or more instances, at least one of which may be a digital object. This is an incomplete version of the object model, used just to access an instance of a digital object (reference).
 */
#[derive(Debug)]
#[derive(Deserialize)]
pub struct ArchivalObject {
    instances: Vec<Instance>,
}
/*
 * Internal data for a version of a digital object
 */
#[derive(Debug)]
#[derive(Serialize, Deserialize)]
pub struct FileVersion {
    jsonmodel_type: String,
    file_uri: String,
    use_statement: String,
    xlink_actuate_attribute: String,
    xlink_show_attribute: String,
    publish: bool,
    is_representative: bool,
}
/* A digital object record from the ASpace API. This is an incomplete representation: just the fields needed to add a new file version to an existing digital object. */
#[derive(Debug)]
#[derive(Serialize, Deserialize)]
pub struct DigitalObject {
    // uri: String,  # probably don't need this
    file_versions: Vec<FileVersion>,
}

/*
 * Wrapping enum for use in preserving state between API calls
 */
#[derive(Debug)]
pub enum ArchivesSpaceData {
    ArchivalObject(ArchivalObject),
    DigitalObject(DigitalObjectWrapper)
}

/*
 * Stores both the serde::Value representation of the JSON object as well as the struct representation from above. The former contains all the fields, most of which we don't need to modify, but we use them to avoid POSTing an incomplete object to ASpace
 */
#[derive(Debug)]
pub struct DigitalObjectWrapper {
    digital_object: DigitalObject,
    object_json: Value,
}

impl Auth {
    pub fn new(username: &str, password: &str, base_url: &str) -> Result<Self> {
        Ok(Auth {
            username: username.to_owned(),
            password: password.to_owned(),
            base_url: Url::parse(base_url)?
            })
    }
}
/*
 * Queries for a login session and stores the token in an Arc for thread safety
 */
impl SessionShared {
    pub async fn create_session(auth: &Auth, client: &Client) -> Result<Self> {
        let url = auth.base_url.join(&format!("users/{}/login", auth.username))?;
        let params = [("password", &auth.password)];
        let response = client.post(url)
                .form(&params)
                .send()
                .await?;
        match response.error_for_status() {
            Ok(res) => {
                let session = res.json::<Session>().await?;
                Ok(SessionShared { session: Arc::new(session.session) })
            }
            Err(err) => Err(anyhow!(err))
        }

    }
    /*
     * Returns a reference to the token as a &str for use in constructing the authentication header for subsequent API calls
     */
    pub fn session(&self) -> &str {
        self.session.as_str()
    }
}
/*
 * TO DO: Makes these fields more configurable?
 */

impl FileVersion {
    pub fn new(file_uri: &str) -> Self {
        FileVersion {
            jsonmodel_type: "file_version".to_owned(),
            file_uri: file_uri.to_owned(),
            use_statement: "public-access".to_owned(),
            xlink_actuate_attribute: "onRequest".to_owned(),
            xlink_show_attribute: "new".to_owned(),
            publish: true,
            is_representative: false
        }
    }
}

impl ArchivesSpaceRecordId {
    /*
     * Creates an instance of an archival object URI from the value retrieved from Fedora
     */
    pub fn from_fedora_resource(resource: &FedoraResource, base_url: &Url) -> Result<Self> {
        // If a related_url is provided, use that
        if let FedoraResource { related_url: Some(url_str), .. } = resource {
            let url: Url = { if url_str.starts_with("http") {
                    Url::parse(url_str)?
                } else {
                    base_url.join(url_str)?
                }
            };
            Ok(ArchivesSpaceRecordId::ArchivalObjectURI(url))
        } else {
            // Otherwise, we need to construct a query from the archival object reference ID
            let url = base_url.join(&format!("find_by_id/archival_objects?ref_id[]={};resolve[]=archival_objects", resource.ref_id))?;
            Ok(ArchivesSpaceRecordId::ArchivalObjectRefId(url))
        }
    }
    /*
     * Creates the URI to a digital object, given its reference
     */
    pub fn from_digital_object_ref(digital_object_ref: &str, base_url: &Url) -> Result<Self> {
        if digital_object_ref.starts_with("http") {
            Ok(ArchivesSpaceRecordId::DigitalObjectURI(Url::parse(digital_object_ref)?))
        } else {
            Ok(ArchivesSpaceRecordId::DigitalObjectURI(base_url.join(digital_object_ref)?))
        }
    }
    /*
     * Clones the url, since the Client takes an owned value
     */
    pub fn url(&self) -> Url {
        match self {
            Self::ArchivalObjectURI(url) => url.clone(),
            Self::ArchivalObjectRefId(url) => url.clone(),
            Self::DigitalObjectURI(url) => url.clone(),
        }
    }
}

impl DigitalObjectWrapper {
    /*
     * Deserializes the provided serde_json::Value for ease of manipulating the file_versions attribute
     */
    pub fn new(json_obj: Value) -> Result<Self> {
        let digital_object: DigitalObject = serde_json::from_value(json_obj.clone())?;
        Ok(DigitalObjectWrapper { digital_object, object_json: json_obj })
    }
    /*
     * Adds a new file version to the list
     */
    pub fn add_file_version(&mut self, file_version: FileVersion) {
        self.digital_object.file_versions.push(file_version);
        self.object_json["file_versions"] = json!(self.digital_object.file_versions);
    }
    pub fn get_json(&self) -> &Value {
        &self.object_json
    }
}

impl ArchivalObject {
    /*
     * Extracts a reference to a digital object from an archival object
     */
    pub fn extract_digital_object_ref(&self) -> Option<String> {
        let instances = self.instances
                            .iter()
                            .filter(|instance|
                                instance.instance_type == "digital_object")
                            .collect::<Vec<&Instance>>();
        if let Some(Instance { digital_object: Some(digital_object), .. }) =          instances.first() {
            return Some(digital_object.ref_.clone());
        }
        None
    }
}
