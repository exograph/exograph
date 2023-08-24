// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

use std::io::stdin;

use rand::Rng;

pub const EXO_INTROSPECTION: &str = "EXO_INTROSPECTION";
pub const EXO_INTROSPECTION_LIVE_UPDATE: &str = "EXO_INTROSPECTION_LIVE_UPDATE";

pub const EXO_CORS_DOMAINS: &str = "EXO_CORS_DOMAINS";
pub const EXO_SERVER_PORT: &str = "EXO_SERVER_PORT";

pub const EXO_POSTGRES_URL: &str = "EXO_POSTGRES_URL";
pub const EXO_POSTGRES_USER: &str = "EXO_POSTGRES_USER";
pub const EXO_POSTGRES_PASSWORD: &str = "EXO_POSTGRES_PASSWORD";
pub const EXO_JWT_SECRET: &str = "EXO_JWT_SECRET";
pub const EXO_JWKS_ENDPOINT: &str = "EXO_JWKS_ENDPOINT";

pub(super) fn generate_random_string() -> String {
    rand::thread_rng()
        .sample_iter(&rand::distributions::Alphanumeric)
        .take(15)
        .map(char::from)
        .map(|c| c.to_ascii_lowercase())
        .collect()
}

pub(crate) fn wait_for_enter(prompt: &str) -> std::io::Result<()> {
    println!("{prompt}");

    let mut line = String::new();
    stdin().read_line(&mut line)?;

    Ok(())
}
