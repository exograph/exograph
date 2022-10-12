use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// A map of interceptors, where the key the the name of the operation and the value is the interception tree.
#[derive(Serialize, Deserialize, Debug)]
pub struct InterceptionMap {
    pub map: HashMap<String, InterceptionTree>,
}

impl InterceptionMap {
    pub fn get(&self, operation_name: &str) -> Option<&InterceptionTree> {
        self.map.get(operation_name)
    }
}

/// Nested structure of interceptors to be applied to an operation.
#[derive(Serialize, Deserialize, Debug)]
pub enum InterceptionTree {
    // before/after
    Intercepted {
        before: Vec<InterceptorIndexWithSubsystemIndex>,
        core: Box<InterceptionTree>,
        after: Vec<InterceptorIndexWithSubsystemIndex>,
    },
    Around {
        core: Box<InterceptionTree>,
        interceptor: InterceptorIndexWithSubsystemIndex,
    },
    // query/mutation
    Operation,
}

/// A type to represent the index of an interceptor across subsystems.
#[derive(Serialize, Deserialize, Clone, Copy, Debug)]
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

/// A type to represent the index of an interceptor within a subsystem.
///
/// This (instead of a simple `usize`) is used to make it intentional that the index is not
/// used for anything else than indexing into the interceptor list.
#[derive(Serialize, Deserialize, Clone, Copy, Debug)]
pub struct InterceptorIndex(pub usize);

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub enum InterceptorKind {
    Before,
    After,
    Around,
}
