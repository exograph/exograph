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
use tokio_postgres::Config;
use tokio_postgres::NoTls;

/// Connect to the specified PostgreSQL database and attempt to run a query.
pub(crate) async fn run_psql(
    query: &str,
    db: &(dyn EphemeralDatabase + Send + Sync),
) -> Result<()> {
    let config = db.url().parse::<Config>()?;
    let (client, conn) = config.connect(NoTls).await?;

    tokio::spawn(async move {
        if let Err(e) = conn.await {
            eprintln!("connection error: {}", e);
        }
    });

    let res = client.simple_query(query).await;

    res.context(format!("PostgreSQL query failed: {query}"))
        .map(|_| ())
}
