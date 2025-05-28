// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

use super::inclusion_exclusion::InclusionExclusion;
use serde::{Deserialize, Serialize};

#[derive(Debug, PartialEq, Clone, Serialize, Deserialize)]
pub struct OperationSet {
    pub models: InclusionExclusion,
    pub operations: InclusionExclusion,
}

impl OperationSet {
    pub fn all() -> Self {
        Self {
            models: InclusionExclusion::all(),
            operations: InclusionExclusion::all(),
        }
    }

    pub fn none() -> Self {
        Self {
            models: InclusionExclusion::none(),
            operations: InclusionExclusion::none(),
        }
    }
}
