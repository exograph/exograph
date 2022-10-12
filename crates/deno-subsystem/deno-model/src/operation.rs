use std::ops::Deref;

use serde::{Deserialize, Serialize};
use subsystem_model_util::operation::{ServiceMutation, ServiceQuery};

pub use subsystem_model_util::operation::GraphQLOperation;
pub use subsystem_model_util::operation::OperationReturnType;

#[derive(Serialize, Deserialize, Debug)]
pub struct DenoQuery(pub ServiceQuery);

impl Deref for DenoQuery {
    type Target = ServiceQuery;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl GraphQLOperation for DenoQuery {
    fn name(&self) -> &str {
        &self.name
    }

    fn is_query(&self) -> bool {
        true
    }
}

#[derive(Serialize, Deserialize, Debug)]
pub struct DenoMutation(pub ServiceMutation);

impl Deref for DenoMutation {
    type Target = ServiceMutation;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl GraphQLOperation for DenoMutation {
    fn name(&self) -> &str {
        &self.name
    }

    fn is_query(&self) -> bool {
        false
    }
}
