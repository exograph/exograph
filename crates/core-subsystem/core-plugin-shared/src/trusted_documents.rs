use serde::{Deserialize, Serialize};
use sha2::Digest;
use std::collections::HashMap;
use thiserror::Error;
use tracing::warn;

#[derive(Serialize, Deserialize, Debug)]
pub enum TrustedDocuments {
    /// Allow all documents and use the map to get the document by its hash if query isn't present and query_hash is
    All(HashMap<String, String>),
    /// Allow only documents contained in the map
    MatchingOnly(HashMap<String, String>),
}

#[derive(Debug)]
pub enum TrustedDocumentEnforcement {
    Enforce,
    DoNotEnforce,
}

impl TrustedDocuments {
    pub fn all() -> TrustedDocuments {
        TrustedDocuments::All(HashMap::new())
    }

    pub fn from_map(mapping: HashMap<String, String>, allow_all: bool) -> TrustedDocuments {
        if allow_all {
            TrustedDocuments::All(mapping)
        } else {
            TrustedDocuments::MatchingOnly(mapping)
        }
    }

    pub fn resolve<'a>(
        &'a self,
        query: Option<&'a str>,
        query_hash: Option<&str>,
        enforcement: TrustedDocumentEnforcement,
    ) -> Result<&str, TrustedDocumentResolutionError> {
        let allow_untrusted = matches!(self, TrustedDocuments::All(_))
            || matches!(enforcement, TrustedDocumentEnforcement::DoNotEnforce);

        match (query, query_hash) {
            (Some(query), None) => {
                if allow_untrusted {
                    Ok(query)
                } else {
                    let query_hash = Self::sha256(query);
                    match self.get(&query_hash) {
                        Some(document) => {
                            warn!("Query sent when sending only the query hash would be efficient");
                            Ok(document)
                        }
                        None => Err(TrustedDocumentResolutionError::NotTrusted {
                            hash: None, // the client didn't send the hash
                            query: Some(query.to_string()),
                        }),
                    }
                }
            }
            (None, Some(query_hash)) => self.get(query_hash).ok_or(if allow_untrusted {
                TrustedDocumentResolutionError::NotFound
            } else {
                TrustedDocumentResolutionError::NotTrusted {
                    hash: Some(query_hash.to_string()),
                    query: None,
                }
            }),
            (Some(_), Some(_)) => Err(TrustedDocumentResolutionError::BothPresent),
            (None, None) => Err(TrustedDocumentResolutionError::NonePresent),
        }
    }

    fn get<'a>(&'a self, key: &str) -> Option<&'a str> {
        match self {
            TrustedDocuments::All(mapping) => mapping.get(key),
            TrustedDocuments::MatchingOnly(mapping) => mapping.get(key),
        }
        .map(|s| s.as_str())
    }

    fn sha256(query: &str) -> String {
        let query_hash = sha2::Sha256::digest(query.as_bytes());
        base16ct::lower::encode_string(&query_hash)
    }
}

#[derive(Error, Debug, PartialEq)]
pub enum TrustedDocumentResolutionError {
    #[error("Untrusted document: hash: {hash:?}, query: {query:?}")]
    NotTrusted {
        hash: Option<String>,
        query: Option<String>,
    },

    #[error("The hash present in the query does not match any trusted document")]
    NotFound,

    #[error("Both query and query_hash present in the payload, only one should be")]
    BothPresent,

    #[error("Neither query not query_hash are in the payload")]
    NonePresent,
}

#[cfg(test)]
mod tests {
    use super::*;
    use multiplatform_test::multiplatform_test;

    #[multiplatform_test]
    fn trusted_documents_resolve() {
        let mut map = HashMap::new();

        for i in 0..=2 {
            let query = format!("query{}", i);
            let hash = TrustedDocuments::sha256(&query);
            map.insert(hash, query);
        }

        let trusted_documents_matching_only = TrustedDocuments::from_map(map.clone(), false);
        let trusted_documents_all = TrustedDocuments::from_map(map, true);

        for i in 0..=2 {
            let query = format!("query{}", i);
            let hash = TrustedDocuments::sha256(&query);

            // Common cases for both types of trusted documents
            for trusted_documents in [&trusted_documents_matching_only, &trusted_documents_all] {
                // Should be able to resolve both by query and hash
                assert_eq!(
                    trusted_documents.resolve(
                        Some(&query),
                        None,
                        TrustedDocumentEnforcement::Enforce
                    ),
                    Ok(query.as_str())
                );
                assert_eq!(
                    trusted_documents.resolve(
                        None,
                        Some(&hash),
                        TrustedDocumentEnforcement::Enforce
                    ),
                    Ok(query.as_str())
                );

                // Should be able to resolve both by query and hash even in the unenforced case
                assert_eq!(
                    trusted_documents.resolve(
                        Some(&query),
                        None,
                        TrustedDocumentEnforcement::DoNotEnforce
                    ),
                    Ok(query.as_str())
                );
                assert_eq!(
                    trusted_documents.resolve(
                        None,
                        Some(&hash),
                        TrustedDocumentEnforcement::DoNotEnforce
                    ),
                    Ok(query.as_str())
                );

                // Should not be able to resolve both by query and hash
                assert_eq!(
                    trusted_documents.resolve(
                        Some(&query),
                        Some(&hash),
                        TrustedDocumentEnforcement::Enforce
                    ),
                    Err(TrustedDocumentResolutionError::BothPresent)
                );
                assert_eq!(
                    trusted_documents.resolve(
                        Some(&query),
                        Some(&hash),
                        TrustedDocumentEnforcement::DoNotEnforce
                    ),
                    Err(TrustedDocumentResolutionError::BothPresent)
                );

                // In unenforced mode, should be able to resolve by query, but not by hash (there would be no mapping for it)
                assert_eq!(
                    trusted_documents.resolve(
                        Some("query3"),
                        None,
                        TrustedDocumentEnforcement::DoNotEnforce
                    ),
                    Ok("query3")
                );
                assert_eq!(
                    trusted_documents.resolve(
                        None,
                        Some("hash3"),
                        TrustedDocumentEnforcement::DoNotEnforce
                    ),
                    Err(TrustedDocumentResolutionError::NotFound)
                );

                // At least one of query or hash should be present
                assert_eq!(
                    trusted_documents.resolve(None, None, TrustedDocumentEnforcement::Enforce),
                    Err(TrustedDocumentResolutionError::NonePresent)
                );
                assert_eq!(
                    trusted_documents.resolve(None, None, TrustedDocumentEnforcement::DoNotEnforce),
                    Err(TrustedDocumentResolutionError::NonePresent)
                );
            }
        }

        // In enforced mode with matching only, either by query or hash should result in "not trusted"
        assert_eq!(
            trusted_documents_matching_only.resolve(
                Some("query3"),
                None,
                TrustedDocumentEnforcement::Enforce
            ),
            Err(TrustedDocumentResolutionError::NotTrusted {
                hash: None,
                query: Some("query3".to_string())
            })
        );
        assert_eq!(
            trusted_documents_matching_only.resolve(
                None,
                Some("hash3"),
                TrustedDocumentEnforcement::Enforce
            ),
            Err(TrustedDocumentResolutionError::NotTrusted {
                hash: Some("hash3".to_string()),
                query: None
            })
        );

        // In enforced mode with all, should be able to resolve by query, but not by hash (there would be no mapping for it)
        assert_eq!(
            trusted_documents_all.resolve(
                Some("query3"),
                None,
                TrustedDocumentEnforcement::Enforce
            ),
            Ok("query3")
        );
        assert_eq!(
            trusted_documents_all.resolve(None, Some("hash3"), TrustedDocumentEnforcement::Enforce),
            Err(TrustedDocumentResolutionError::NotFound)
        );
    }
}
