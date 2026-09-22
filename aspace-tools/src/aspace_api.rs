use reqwest::{header::{HeaderValue, HeaderMap}, Client, Url};
use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use fedora_tools::fedora_api::FedoraResource;
use std::sync::Arc;
use crate::aspace_objects::{ArchivesSpaceRecordId::{ArchivalObjectRefId, ArchivalObjectURI, DigitalObjectURI}, *};
/*
 * To Do:
 *   - Mock API testing
 *   - Implement tracing
 **
 */


/*
 * State machine for managing the steps required to update a digital object in ASPace via API, given a reference to an archival object
 */
#[derive(Debug)]
pub struct ArchivesSpaceUpdate {
    client: Arc<ArchivesSpaceClient>, // to clone across threads
    object_id: ArchivesSpaceRecordId,
    hyrax_ref: String,
    object: Option<ArchivesSpaceData>,
}

/*
 *  Contains thread-safe clones
 */
#[derive(Debug)]
pub struct ArchivesSpaceClient {
    client: Client, // clone of reqwest::Client (uses Arc internally to share across threads)
    base_url: Url, // cloned from auth
    session: SessionShared,
}

impl ArchivesSpaceClient {
    pub async fn new(auth: Auth) -> Result<Arc<Self>> {
        let client = Client::builder().build()?;
        let session = SessionShared::create_session(&auth, &client).await?;

        Ok(Arc::new(ArchivesSpaceClient {
            client: client,
            session: session,
            base_url: auth.base_url,
        }))
    }
    /*
     * Construct the authentication header for an ASpace API request
     */
    fn construct_headers(&self) -> Result<HeaderMap> {
        let mut header_map = HeaderMap::new();
        header_map.insert("X-ArchivesSpace-Session", HeaderValue::from_str(self.session.session())?);
        Ok(header_map)
    }
    /*
     * Get an object of type T from the ASpace API
     */
    pub async fn get_object<T>(&self, object_uri: Url) -> Result<T>
        where T:  serde::de::DeserializeOwned
    {
        let headers = self.construct_headers()?;
        let response = self.client.get(object_uri)
            .headers(headers)
                .send()
                .await?;
        match response.error_for_status() {
            Ok(res) => {
                let data = res.json::<T>().await?;
                Ok(data)
            }
            Err(err) => Err(anyhow!(err))
        }
    }
    /*
     * Update an object of type T via ASpace API
     */
    pub async fn update_object<T>(&self, object_uri: Url, object: &T) -> Result<()>
        where T: serde::Serialize {
            let headers = self.construct_headers()?;
            let response = self.client.post(object_uri)
                                .headers(headers)
                                .json(object)
                                .send()
                                .await?;
            match response.error_for_status() {
                Ok(_) => Ok(()),
                Err(err) => Err(anyhow!(err))
            }
    }
}

impl ArchivesSpaceUpdate {
    /*
     * Reuses a client and session (cloned by an Arc<T>), so that the same client can be used for concurrent instances of the update pipeline
     */
    pub fn new(client: Arc<ArchivesSpaceClient>, resource: &FedoraResource) -> Result<Self> {
        let id = ArchivesSpaceRecordId::from_fedora_resource(resource, &client.base_url)?;
        Ok(ArchivesSpaceUpdate {
            client: client,
            object_id: id,
            hyrax_ref: resource.uri.clone(),
            object: None,  // Holds JSON after request
        })
    }
    /*
     * Handles the first two phases of the ASpace pipeline:
     * 1) retrieve the archival object corresponding to the given object URI or ref ID
     * 2) retrieve the digital object corresponding to a digital object URI
     * Internal state is updated with the retrieved object
     */
    pub async fn get_object(&mut self) -> Result<()> {
        let obj = match self.object_id {
            ArchivalObjectRefId(_) => {
                let obj: Value = self.client.get_object(self.object_id.url()).await?;
                let obj = obj.get("archival_objects")
                            .and_then(|v| v.get(0)) // expect 1 and only 1 archival object per response
                            .and_then(|v| v.get("_resolve"))
                            .ok_or(anyhow!("Archival Object not found"))?;
               ArchivesSpaceData::ArchivalObject(ArchivalObject::deserialize(obj)?)

            },
            ArchivalObjectURI(_) => {
                let obj: ArchivalObject = self.client.get_object(self.object_id.url()).await?;
                 ArchivesSpaceData::ArchivalObject(obj)
            },
            DigitalObjectURI(_) => {
                let json_obj: Value = self.client.get_object(self.object_id.url()).await?;
                ArchivesSpaceData::DigitalObject(DigitalObjectWrapper::new(json_obj)?)
            }
        };
        let _ = self.object.insert(obj);
        Ok(())
    }
    /*
     * Extracts a digital object URI from an archival object.
     * This should be run AFTER phase 1 above and before phase 2.
     */
    pub fn extract_digital_object_ref(&mut self) -> Result<()> {
        match &self.object {
            Some(ArchivesSpaceData::ArchivalObject(obj)) => {
                if let Some(digital_object_ref) = obj.extract_digital_object_ref() {
                    self.object_id = ArchivesSpaceRecordId::from_digital_object_ref(&digital_object_ref, &self.client.base_url)?
                }
                Ok(())
            }
            _ => Ok(())
        }
    }
    /*
     * Phase 3: update the digital object with a new file version containing the URI to the Hyrax resource
     */
    pub async fn update_digital_object(&mut self) -> Result<()> {
        match &mut self.object {
            Some(ArchivesSpaceData::DigitalObject(obj)) => {
                let file_version = FileVersion::new(&self.hyrax_ref);
                obj.add_file_version(file_version);
                self.client.update_object(self.object_id.url(), obj.get_json()).await?;
                Ok(())
            },
            _ => Err(anyhow!("No digital object found to update"))
        }
    }

}
