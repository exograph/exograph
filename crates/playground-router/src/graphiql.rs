// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

#![cfg(not(target_family = "wasm"))]

use common::env_const::{
    _EXO_UPSTREAM_ENDPOINT_URL, EXO_INTROSPECTION_LIVE_UPDATE, EXO_JWT_SOURCE_COOKIE,
    EXO_JWT_SOURCE_HEADER,
};
use exo_env::Environment;
use include_dir::{Dir, include_dir};
use serde::Serialize;
use std::path::Path;

use common::env_const::{get_graphql_http_path, get_playground_http_path};

static GRAPHIQL_DIR: Dir<'_> = include_dir!("$CARGO_MANIFEST_DIR/../../graphiql/app/dist");

pub fn get_asset_bytes<P: AsRef<Path>>(file_name: P, env: &dyn Environment) -> Option<Vec<u8>> {
    GRAPHIQL_DIR.get_file(file_name.as_ref()).map(|file| {
        if file_name.as_ref() == Path::new("index.html") {
            let str = file
                .contents_utf8()
                .expect("index.html for playground should be utf8");

            let playground_config = exo_playground_config(env);

            let str = str.replace(
                "window.exoConfig = {}",
                &format!(
                    "window.exoConfig = {}",
                    serde_json::to_string(&playground_config).unwrap()
                ),
            );
            let str = str.replace(
                "%%PLAYGROUND_HTTP_PATH%%",
                &playground_config.playground_http_path,
            );

            str.as_bytes().to_owned()
        } else {
            file.contents().to_owned()
        }
    })
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct PlaygroundConfig {
    playground_http_path: String,
    graphql_http_path: String,

    enable_schema_live_update: bool,

    #[serde(
        skip_serializing_if = "Option::is_none",
        rename = "upstreamGraphQLEndpoint"
    )]
    upstream_graphql_endpoint: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    oidc_url: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    jwt_source_header: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    jwt_source_cookie: Option<String>,
}

fn exo_playground_config(env: &dyn Environment) -> PlaygroundConfig {
    let enable_schema_live_update = env
        .enabled(EXO_INTROSPECTION_LIVE_UPDATE, false)
        .unwrap_or(false);
    // Normalize the OIDC URL to remove the trailing slash, if any
    let oidc_url = env
        .get("EXO_OIDC_URL")
        .map(|s| s.trim_end_matches('/').to_owned());

    let playground_http_path = get_playground_http_path(env);
    let graphql_http_path = get_graphql_http_path(env);
    let upstream_graphql_endpoint = env.get(_EXO_UPSTREAM_ENDPOINT_URL);

    let jwt_source_header = env.get(EXO_JWT_SOURCE_HEADER);
    let jwt_source_cookie = env.get(EXO_JWT_SOURCE_COOKIE);

    PlaygroundConfig {
        playground_http_path,
        graphql_http_path,

        enable_schema_live_update,
        oidc_url,
        upstream_graphql_endpoint,

        jwt_source_header,
        jwt_source_cookie,
    }
}
