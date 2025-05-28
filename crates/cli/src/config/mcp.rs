use core_plugin_shared::profile::{
    InclusionExclusion, OperationSet, SchemaProfile, SchemaProfiles,
};
use serde::Deserialize;
use wildmatch::WildMatch;

#[derive(Debug, Deserialize)]
pub struct SchemaProfilesSer {
    profiles: Vec<SchemaProfileSer>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct SchemaProfileSer {
    name: String,
    queries: Option<OperationSetSer>,
    mutations: Option<OperationSetSer>,
}

impl From<SchemaProfilesSer> for SchemaProfiles {
    fn from(serialized: SchemaProfilesSer) -> Self {
        SchemaProfiles {
            profiles: serialized
                .profiles
                .into_iter()
                .map(|profile| (profile.name.clone(), profile.into()))
                .collect(),
        }
    }
}

impl From<SchemaProfileSer> for SchemaProfile {
    fn from(serialized: SchemaProfileSer) -> Self {
        SchemaProfile {
            queries: serialized.queries.map_or(OperationSet::all(), |q| q.into()),
            mutations: serialized
                .mutations
                .map_or(OperationSet::none(), |m| m.into()),
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct OperationSetSer {
    models: InclusionExclusionSer,
    operations: InclusionExclusionSer,
}

impl From<OperationSetSer> for OperationSet {
    fn from(serialized: OperationSetSer) -> Self {
        OperationSet {
            models: serialized.models.into(),
            operations: serialized.operations.into(),
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct InclusionExclusionSer {
    #[serde(default)]
    include: Vec<String>,
    #[serde(default)]
    exclude: Vec<String>,
}

impl From<InclusionExclusionSer> for InclusionExclusion {
    fn from(serialized: InclusionExclusionSer) -> Self {
        InclusionExclusion {
            include: serialized
                .include
                .into_iter()
                .map(|s| WildMatch::new(&s))
                .collect(),
            exclude: serialized
                .exclude
                .into_iter()
                .map(|s| WildMatch::new(&s))
                .collect(),
        }
    }
}
