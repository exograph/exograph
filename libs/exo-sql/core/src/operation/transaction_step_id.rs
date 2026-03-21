// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

/// An identifier for a step in a transaction script.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TransactionStepId(pub usize);
