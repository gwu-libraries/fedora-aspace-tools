use crate::fedora_queue::FedoraURIs;
use crate::fedora_auth::FedoraAuth;
use reqwest::{header, header::ACCEPT, Client};
use anyhow::{Result};
use rdf::{reader::{rdf_parser::RdfParser, turtle_parser::TurtleParser}, graph::Graph, uri::Uri, node::Node, triple::Triple};

const HYRAX_MODEL_PREDICATE: &'static str = "info:fedora/fedora-system:def/model#hasModel";
const ASPACE_REF_PREDICATE: &'static str = "http://purl.org/dc/terms/identifier";
const HYRAX_MODEL_OBJECT: &'static str = "ArchivalDocument";

#[derive(Debug, PartialEq, Eq)]
pub enum HyraxModel {
    ArchivalDocument,
    FileSet
}

#[derive(Debug, PartialEq, Eq)]
pub struct FedoraResource {
    pub uri: String,
    pub model: HyraxModel,
    pub ref_id: String, // ArchivesSpace ref_id

}

fn has_desired_model(triple: &Triple) -> bool {
    if let Node::LiteralNode { literal, ..} = triple.object() {
        literal == HYRAX_MODEL_OBJECT
    } else {
        false
    }
}

fn extract_identifier(triple: &Triple) -> Option<String> {
    if let Node::LiteralNode { literal, ..} = triple.object() {
        // Additional tests here to skip other identifiers?
        Some(literal.clone())
    } else {
            None
    }
}

impl FedoraResource {
    pub fn from_graph(g: &Graph, uri: &str) -> Option<Self> {

        let model_pred = Node::UriNode{uri: Uri::new(HYRAX_MODEL_PREDICATE.to_string())};
        //let model_object = Node::LiteralNode { literal: HYRAX_MODEL_OBJECT.to_string(), data_type: None, language: None };
        let aref_pred = Node::UriNode{uri: Uri::new(ASPACE_REF_PREDICATE.to_string())};
        let model_triples = g.get_triples_with_predicate(&model_pred);
        let aref_triples = g.get_triples_with_predicate(&aref_pred);
        // Filter for desired model(s) here
        if model_triples.into_iter().any(has_desired_model) {
            // expect some sort of reference to an ArchivesSpace object
            let Some(a_ref) = aref_triples.into_iter().find_map(extract_identifier) else {
                return None;
            };
            Some(FedoraResource {
                    uri: uri.to_owned(),
                    model: HyraxModel::ArchivalDocument, // implement a conversion from the model string
                    ref_id: a_ref,
            })
        } else {
            None
        }
    }
}

pub fn create_client() -> Result<Client> {
    let mut headers = header::HeaderMap::new();
    headers.insert(ACCEPT, header::HeaderValue::from_static("text/turtle"));
    let client = reqwest::Client::builder()
        .default_headers(headers)
        .build()?;
    Ok(client)
}

pub async fn get_fedora_resource(client: &Client, uri: &str, fedora_auth: &FedoraAuth) -> Result<Option<FedoraResource>> {
    let url = fedora_auth.root_url.join(uri)?;
    let resp = client.get(url)
        .basic_auth(&fedora_auth.username, Some(&fedora_auth.password))
        .send()
        .await?
        .text()
        .await?;
    let mut reader = TurtleParser::from_string(&resp);
    match reader.decode() {
        Ok(graph) => Ok(FedoraResource::from_graph(&graph, uri)),
        Err(error) => Err(anyhow::Error::msg(format!("{:?}", error))), // Maybe a more elegant way to handle this --> the Error type implemented by the rdf crate is not Send + Sync, which makes using it in this async context problematic
    }
}
