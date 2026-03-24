// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

use crate::pg_extension::PgExtension;

/// A function in a Postgres context (uses PgExtension for VectorDistance etc.)
pub type Function = exo_sql_core::operation::Function<PgExtension>;
