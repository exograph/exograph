// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

//! Transform abstract operations into concrete operations for a specific database with an implementation for
//! Postgres.

pub(crate) mod pg;
pub(crate) mod transformer;

mod join_util;
mod table_dependency;
mod test_util;
