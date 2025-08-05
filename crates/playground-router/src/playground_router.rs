// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

#![cfg(not(target_family = "wasm"))]

use std::sync::Arc;

use async_trait::async_trait;
use common::context::RequestContext;
use common::http::{Headers, RequestHead, RequestPayload, ResponsePayload};
use common::router::Router;
use common::{
    env_const::get_playground_http_path,
    http::{ResponseBody, strip_leading, strip_leading_slash},
    introspection::allow_introspection,
};
use exo_env::Environment;
use http::StatusCode;

use crate::playground;

pub struct PlaygroundRouterConfig {
    playground_path: String,
    env: Arc<dyn Environment>,
}

impl PlaygroundRouterConfig {
    pub fn new(env: Arc<dyn Environment>) -> Self {
        Self {
            playground_path: strip_leading_slash(&get_playground_http_path(env.as_ref()))
                .to_string(),
            env: env.clone(),
        }
    }

    pub fn suitable(&self, request_path: &str, request_method: http::Method) -> bool {
        let request_path = strip_leading_slash(request_path);

        (request_path.starts_with(&self.playground_path) || request_path.is_empty())
            && request_method == http::Method::GET
    }
}

pub struct PlaygroundRouter {
    config: Arc<PlaygroundRouterConfig>,
}

#[cfg(not(target_family = "wasm"))]
impl PlaygroundRouter {
    pub fn new(config: Arc<PlaygroundRouterConfig>) -> Self {
        Self { config }
    }

    fn suitable(&self, request_head: &(dyn RequestHead + Sync)) -> bool {
        self.config
            .suitable(&request_head.get_path(), request_head.get_method())
    }
}

#[async_trait]
impl<'a> Router<RequestContext<'a>> for PlaygroundRouter {
    async fn route(&self, request_context: &RequestContext<'a>) -> Option<ResponsePayload> {
        if !self.suitable(request_context.get_head()) {
            return None;
        }

        let env = self.config.env.as_ref();

        let introspection_enabled = allow_introspection(env);
        if !introspection_enabled {
            return Some(ResponsePayload {
                body: ResponseBody::Bytes("Introspection is disabled".as_bytes().to_vec()),
                headers: Headers::new(),
                status_code: StatusCode::OK,
            });
        }

        let path = strip_leading_slash(&request_context.get_head().get_path());
        let playground_path = &self.config.playground_path;

        // We redirect to the playground path if the path is empty. This provides a better user experience
        // as the user will be redirected to the playground path without having to add it manually.
        if path.is_empty() {
            return Some(ResponsePayload {
                body: ResponseBody::Redirect(playground_path.clone()),
                headers: Headers::new(),
                status_code: StatusCode::PERMANENT_REDIRECT,
            });
        }

        // remove the leading self.playground_path from the path
        let path = strip_leading(&path, playground_path);

        let index_path = "index.html";
        let asset_path = if path.is_empty() {
            index_path.to_string()
        } else {
            strip_leading_slash(&path)
        };

        let content_type = mime_guess::from_path(&asset_path).first_or_octet_stream();

        // we shouldn't cache the index page, as we substitute in the endpoint path dynamically
        let cache_control = if asset_path == index_path {
            vec![(
                http::header::CACHE_CONTROL.to_string(),
                "no-cache".to_string(),
            )]
        } else {
            vec![(
                http::header::CACHE_CONTROL.to_string(),
                format!("public, max-age={}", 60 * 60 * 24 * 365), //seconds in one year
            )]
        };

        match playground::get_asset_bytes(asset_path, env) {
            Some(asset) => {
                let headers = cache_control.into_iter().chain(vec![(
                    http::header::CONTENT_TYPE.to_string(),
                    content_type.to_string(),
                )]);

                Some(ResponsePayload {
                    body: ResponseBody::Bytes(asset),
                    headers: Headers::from_vec(headers.collect()),
                    status_code: StatusCode::OK,
                })
            }
            None => Some(ResponsePayload {
                body: ResponseBody::None,
                headers: Headers::new(),
                status_code: StatusCode::NOT_FOUND,
            }),
        }
    }
}
