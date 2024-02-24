use serde::{Deserialize, Serialize};
use sha2::Digest;
use std::collections::HashMap;
use thiserror::Error;
use tracing::warn;

#[derive(Serialize, Deserialize, Debug)]
pub enum TrustedDocuments {
    /// Allow all documents and use the map to get the document by its hash if query isn't present and query_hash is
    All(Option<HashMap<String, String>>),
    /// Allow only documents contained in the map
    MatchingOnly(HashMap<String, String>),
}

impl TrustedDocuments {
    pub fn all() -> TrustedDocuments {
        TrustedDocuments::All(None)
    }

    pub fn from_map(mapping: HashMap<String, String>, allow_all: bool) -> TrustedDocuments {
        if allow_all {
            TrustedDocuments::All(Some(mapping))
        } else {
            TrustedDocuments::MatchingOnly(mapping)
        }
    }

    pub fn get<'a>(&'a self, key: &str) -> Option<&'a str> {
        match self {
            TrustedDocuments::All(mapping) => mapping.as_ref().and_then(|mapping| mapping.get(key)),
            TrustedDocuments::MatchingOnly(mapping) => mapping.get(key),
        }
        .map(|s| s.as_str())
    }

    pub fn resolve_unchecked<'a>(
        &'a self,
        query: Option<&'a str>,
        query_hash: Option<&str>,
    ) -> Result<&str, TrustedDocumentResolutionError> {
        match (query, query_hash) {
            (Some(query), None) => Ok(query),
            (None, Some(query_hash)) => self
                .get(query_hash)
                .ok_or(TrustedDocumentResolutionError::NotFound),
            (Some(_), Some(_)) => Err(TrustedDocumentResolutionError::BothPresent),
            _ => Err(TrustedDocumentResolutionError::NonePresent),
        }
    }

    pub fn resolve<'a>(
        &'a self,
        query: Option<&'a str>,
        query_hash: Option<&str>,
    ) -> Result<&str, TrustedDocumentResolutionError> {
        match self {
            TrustedDocuments::All(_) => self.resolve_unchecked(query, query_hash),
            TrustedDocuments::MatchingOnly(_) => {
                match (query, query_hash) {
                    (Some(query), None) => {
                        warn!("Query sent when sending only the query_hash is sufficient and efficient");
                        let query_hash = sha2::Sha256::digest(query.as_bytes());
                        let query_hash = base16ct::lower::encode_string(&query_hash);
                        self.get(&query_hash)
                            .ok_or(TrustedDocumentResolutionError::NotTrusted)
                    }
                    (None, Some(query_hash)) => self
                        .get(query_hash)
                        .ok_or(TrustedDocumentResolutionError::NotTrusted),
                    (Some(_), Some(_)) => Err(TrustedDocumentResolutionError::BothPresent),
                    _ => Err(TrustedDocumentResolutionError::NonePresent),
                }
            }
        }
    }
}

#[derive(Error, Debug)]
pub enum TrustedDocumentResolutionError {
    #[error("The document is not trusted")]
    NotTrusted,

    #[error("The hash present in the query does not match any trusted document")]
    NotFound,

    #[error("Both query and query_hash present in the payload, only one should be")]
    BothPresent,

    #[error("Neither query not query_hash are in the payload")]
    NonePresent,
}
