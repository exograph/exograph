// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

use anyhow::{Context, Result};
use exo_sql::testing::db::EphemeralDatabase;
use postgres::Config;
use postgres::NoTls;

/// Connect to the specified PostgreSQL database and attempt to run a query.
pub(crate) fn run_psql(query: &str, db: &(dyn EphemeralDatabase + Send + Sync)) -> Result<()> {
    let mut client = db.url().parse::<Config>()?.connect(NoTls)?;
    client
        .simple_query(query)
        .context(format!("PostgreSQL query failed: {query}"))
        .map(|_| ())
}
