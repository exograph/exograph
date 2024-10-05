// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug)]
pub struct PathTemplate {
    pub segments: Vec<PathTemplateSegment>,
}

#[derive(Serialize, Deserialize, Debug)]
pub enum PathTemplateSegment {
    Literal(String), // e.g. "todos" in /todos/{id}
    // TODO: Replace `String` with an enum to specify more specific parameter types
    Parameter(String), // e.g. "id" in /todos/{id}
}
