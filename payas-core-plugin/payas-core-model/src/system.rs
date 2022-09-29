use std::collections::HashMap;

use serde::{Deserialize, Serialize};

/// A type to represent the index of an interceptor within a subsystem.
///
/// This (instead of a simple `usize`) is used to make it intentional that the index is not
/// used for anything else than indexing into the interceptor list.
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct InterceptorIndex(pub usize);

/// A type to represent the index of an interceptor across subsystems.
#[derive(Serialize, Deserialize, Debug)]
pub struct InterceptorIndexWithSubsystemIndex {
    pub subsystem_index: usize,
    pub interceptor_index: InterceptorIndex,
}

impl InterceptorIndexWithSubsystemIndex {
    pub fn new(subsystem_index: usize, interceptor_index: InterceptorIndex) -> Self {
        Self {
            subsystem_index,
            interceptor_index,
        }
    }
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Subsystem {
    pub id: String,
    pub subsystem_index: usize,
    pub serialized_subsystem: Vec<u8>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct System {
    pub subsystems: Vec<Subsystem>,
    pub query_interception_map: InterceptionMap,
    pub mutation_interception_map: InterceptionMap,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct InterceptionMap {
    pub map: HashMap<String, Vec<InterceptorIndexWithSubsystemIndex>>,
}

impl InterceptionMap {
    pub fn get(&self, operation_name: &str) -> Option<&Vec<InterceptorIndexWithSubsystemIndex>> {
        self.map.get(operation_name)
    }
}
