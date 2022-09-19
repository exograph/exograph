use std::collections::{BTreeMap, HashMap};
use std::fmt::Debug;

use crate::ast::ast_types::AstAnnotation;

use super::Typed;
use codemap::Span;

use serde::{Deserialize, Serialize, Serializer};

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct AnnotationMap {
    #[serde(serialize_with = "ordered_map")] // serialize with ordered_map to sort by key
    pub annotations: HashMap<String, AstAnnotation<Typed>>,

    /// Spans of the annotations (also keeps track of duplicate annotations).
    #[serde(skip_serializing)]
    #[serde(skip_deserializing)]
    pub spans: HashMap<String, Vec<Span>>,
}

fn ordered_map<S: Serializer>(
    value: &HashMap<String, AstAnnotation<Typed>>,
    serializer: S,
) -> Result<S::Ok, S::Error> {
    let ordered = value.iter().collect::<BTreeMap<_, _>>();
    ordered.serialize(serializer)
}
