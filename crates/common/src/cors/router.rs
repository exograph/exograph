// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

use std::sync::Arc;

use http::{Method, StatusCode};

use crate::{
    cors::{allow::HeaderStringProvider, config::CorsConfig},
    http::{Headers, RequestPayload, ResponseBody, ResponsePayload},
    router::Router,
};

use super::config::CorsResponse;

/// Reference: https://fetch.spec.whatwg.org/#http-requests
pub struct CorsRouter {
    underlying: Arc<dyn Router + Send>,
    config: CorsConfig,
}

impl CorsRouter {
    pub fn new(underlying: Arc<dyn Router + Send>, config: CorsConfig) -> Self {
        Self { underlying, config }
    }
}

#[async_trait::async_trait]
impl Router for CorsRouter {
    /// Route a request applying CORS rules.
    ///
    /// For a denied cross-site request, we return 403 (Forbidden), since there is no
    /// specified standard, but https://github.com/whatwg/fetch/issues/172 makes sense.
    /// It suggests the possibility of adding more details in the body, but also cautions
    /// to not reveal too much information. Therefore, we don't add a body.
    async fn route(&self, request: &mut (dyn RequestPayload + Send)) -> Option<ResponsePayload> {
        let origin_header = request.get_head().get_header(http::header::ORIGIN.as_str());

        let add_cors_headers = |response: &mut ResponsePayload, origin: &str| {
            let headers = &mut response.headers;
            headers.insert(
                http::header::ACCESS_CONTROL_ALLOW_ORIGIN.to_string(),
                origin.to_string(),
            );
            if let Some(method_header) = self.config.allow_methods.header_string() {
                headers.insert(
                    http::header::ACCESS_CONTROL_ALLOW_METHODS.to_string(),
                    method_header,
                );
            }
            if let Some(headers_header) = self.config.allow_headers.header_string() {
                headers.insert(
                    http::header::ACCESS_CONTROL_ALLOW_HEADERS.to_string(),
                    headers_header,
                );
            }
            if let Some(max_age_seconds) = self.config.max_age_seconds {
                headers.insert(
                    http::header::ACCESS_CONTROL_MAX_AGE.to_string(),
                    max_age_seconds.to_string(),
                );
            }
            // Currently, we only vary by origin (specifically, we don't vary by
            // "Access-Control-Request-Method" or "Access-Control-Request-Headers")
            headers.insert(http::header::VARY.to_string(), "Origin".to_string());
        };

        let forbidden_response = || ResponsePayload {
            body: ResponseBody::None,
            headers: Headers::new(),
            status_code: StatusCode::FORBIDDEN,
        };

        let cors_response = self
            .config
            .allow_origin(origin_header.as_deref(), &request.get_head().get_method());

        if request.get_head().get_method() == Method::OPTIONS {
            match cors_response {
                CorsResponse::Allow(origin) => {
                    let mut response = ResponsePayload {
                        body: ResponseBody::None,
                        headers: Headers::new(),
                        status_code: StatusCode::OK,
                    };
                    add_cors_headers(&mut response, origin);

                    Some(response)
                }
                CorsResponse::NoCorsHeaders | CorsResponse::Deny => Some(forbidden_response()),
            }
        } else {
            match cors_response {
                CorsResponse::Allow(origin) => {
                    let mut response = self.underlying.route(request).await;

                    if let Some(ref mut response) = response {
                        add_cors_headers(response, origin);
                    }

                    response
                }
                CorsResponse::NoCorsHeaders => self.underlying.route(request).await,
                CorsResponse::Deny => Some(forbidden_response()),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use serde_json::Value;
    use tokio::task_local;

    use super::*;
    use crate::{
        http::{Headers, RequestHead},
        router::Router,
    };

    task_local! {
        pub static RESPONSE_PAYLOAD: Arc<MockResponsePayload>
    }

    async fn assert_cors_enforcement(
        config: CorsConfig,
        matching_origin: Option<String>,
        not_matching_origin: Option<String>,
    ) {
        let response = perform_cors_request(config.clone(), &Method::OPTIONS, None).await;
        assert_cors_forbidden(response.as_ref(), &Method::OPTIONS, None);

        for method in crate::cors::config::tests::NON_PREFLIGHT_METHODS {
            if let Some(origin) = &matching_origin {
                let response =
                    perform_cors_request(config.clone(), &method, Some(origin.clone())).await;
                assert_cors_allowed(response.as_ref(), &origin, &method, None);
            }

            let response = perform_cors_request(config.clone(), &method, None).await;
            assert_allowed_no_cors_headers(response.as_ref(), &method, None);

            if let Some(origin) = &not_matching_origin {
                let response =
                    perform_cors_request(config.clone(), &method, Some(origin.clone())).await;
                assert_allowed_no_cors_headers(response.as_ref(), &method, Some(origin.clone()));
            }
        }
    }

    fn assert_cors_allowed(
        response: Option<&ResponsePayload>,
        expected_allow_origin: &str,
        method: &Method,
        origin: Option<String>,
    ) {
        let context_message = || format!("(method: {:?}, origin: {:?})", method, origin);

        assert!(
            response.is_some(),
            "Expected some response {}",
            context_message()
        );

        let response = response.unwrap();

        assert_eq!(
            response.status_code,
            StatusCode::OK,
            "Expected status code OK {}",
            context_message()
        );
        assert_eq!(
            response
                .headers
                .get(http::header::ACCESS_CONTROL_ALLOW_ORIGIN.as_str()),
            Some(expected_allow_origin.to_string()),
            "Expected allow origin header {}",
            context_message()
        );
        assert_eq!(
            response
                .headers
                .get(http::header::ACCESS_CONTROL_ALLOW_METHODS.as_str()),
            Some("*".to_string()),
            "Expected allow methods header {}",
            context_message()
        );
        assert_eq!(
            response
                .headers
                .get(http::header::ACCESS_CONTROL_ALLOW_HEADERS.as_str()),
            Some("*".to_string()),
            "Expected allow headers header {}",
            context_message()
        );
        assert_eq!(
            response
                .headers
                .get(http::header::ACCESS_CONTROL_MAX_AGE.as_str()),
            Some("3600".to_string()),
            "Expected max age header {}",
            context_message()
        );

        assert_eq!(
            response.headers.get(http::header::VARY.as_str()),
            Some("Origin".to_string()),
            "Expected vary header {}",
            context_message()
        );
    }

    fn assert_cors_forbidden(
        response: Option<&ResponsePayload>,
        method: &Method,
        origin: Option<String>,
    ) {
        let context_message = || format!("(method: {:?}, origin: {:?})", method, origin);

        assert!(
            response.is_some(),
            "Expected some response {}",
            context_message()
        );

        let response = response.unwrap();

        assert_eq!(
            response.status_code,
            StatusCode::FORBIDDEN,
            "Expected forbidden status code {}",
            context_message()
        );

        for header in [
            http::header::ACCESS_CONTROL_ALLOW_ORIGIN,
            http::header::ACCESS_CONTROL_ALLOW_METHODS,
            http::header::ACCESS_CONTROL_ALLOW_HEADERS,
            http::header::ACCESS_CONTROL_MAX_AGE,
            http::header::VARY,
        ] {
            assert_eq!(
                response.headers.get(header.as_str()),
                None,
                "Expected no {} header {}",
                header.as_str(),
                context_message()
            );
        }
    }

    fn assert_allowed_no_cors_headers(
        response: Option<&ResponsePayload>,
        method: &Method,
        origin: Option<String>,
    ) {
        let context_message = || format!("(method: {:?}, origin: {:?})", method, origin);

        assert!(
            response.is_some(),
            "Expected some response {}",
            context_message()
        );

        let response = response.unwrap();

        assert_eq!(
            response.status_code,
            StatusCode::OK,
            "Expected starus ok {}",
            context_message()
        );

        assert_eq!(
            response
                .headers
                .get(http::header::ACCESS_CONTROL_ALLOW_ORIGIN.as_str()),
            None,
            "Expected no Access-Control-Allow-Origin header {}",
            context_message()
        );
        assert_eq!(
            response
                .headers
                .get(http::header::ACCESS_CONTROL_ALLOW_METHODS.as_str()),
            None,
            "Expected no Access-Control-Allow-Methods header {}",
            context_message()
        );
        assert_eq!(
            response
                .headers
                .get(http::header::ACCESS_CONTROL_ALLOW_HEADERS.as_str()),
            None,
            "Expected no Access-Control-Allow-Headers header {}",
            context_message()
        );
        assert_eq!(
            response
                .headers
                .get(http::header::ACCESS_CONTROL_MAX_AGE.as_str()),
            None,
            "Expected no Access-Control-Max-Age header {}",
            context_message()
        );
    }

    async fn perform_cors_request(
        config: CorsConfig,
        method: &Method,
        origin: Option<String>,
    ) -> Option<ResponsePayload> {
        let underlying = Arc::new(MockRouter {});
        let cors_router = CorsRouter::new(underlying.clone(), config);

        let mut request =
            MockRequestPayload::new(method.clone(), "/".to_string(), None, Headers::new());
        if let Some(origin) = origin {
            request.headers.insert("origin".to_string(), origin);
        }

        let underlying_response = MockResponsePayload {
            body: None,
            status_code: StatusCode::OK,
            headers: Headers::new(),
        };

        RESPONSE_PAYLOAD
            .scope(Arc::new(underlying_response), async move {
                cors_router.route(&mut request).await
            })
            .await
    }

    #[tokio::test]
    async fn options_request_allow_all() {
        assert_cors_enforcement(
            CorsConfig::from_env(Some("*".to_string())),
            Some("https://example.com".to_string()),
            None,
        )
        .await;
    }

    #[tokio::test]
    async fn options_request_allow_specific() {
        let cors_config =
            CorsConfig::from_env(Some("https://example.com, https://example.org".to_string()));

        assert_cors_enforcement(
            cors_config.clone(),
            Some("https://example.com".to_string()),
            Some("https://not-example.com".to_string()),
        )
        .await;
        assert_cors_enforcement(
            cors_config,
            Some("https://example.org".to_string()),
            Some("https://not-example.org".to_string()),
        )
        .await;
    }

    #[tokio::test]
    async fn options_request_allow_none() {
        let cors_config = CorsConfig::from_env(None);

        assert_cors_enforcement(cors_config, None, None).await;
    }

    struct MockRouter {}

    #[async_trait::async_trait]
    impl Router for MockRouter {
        async fn route(
            &self,
            _request: &mut (dyn RequestPayload + Send),
        ) -> Option<ResponsePayload> {
            let mock_response = RESPONSE_PAYLOAD.get();

            Some(ResponsePayload {
                body: ResponseBody::Bytes(
                    mock_response
                        .as_ref()
                        .body
                        .clone()
                        .unwrap_or(Value::Null)
                        .to_string()
                        .as_bytes()
                        .to_vec(),
                ),
                headers: mock_response.headers.clone(),
                status_code: mock_response.status_code,
            })
        }
    }

    #[derive(Clone)]
    struct MockRequestPayload {
        method: Method,
        path: String,
        body: Option<Value>,
        headers: Headers,
    }

    impl MockRequestPayload {
        fn new(method: Method, path: String, body: Option<Value>, headers: Headers) -> Self {
            Self {
                method,
                path,
                body,
                headers,
            }
        }
    }

    impl RequestPayload for MockRequestPayload {
        fn get_head(&self) -> &(dyn RequestHead + Send + Sync) {
            self
        }

        fn take_body(&mut self) -> Value {
            self.body.take().unwrap_or(Value::Null)
        }
    }

    impl RequestHead for MockRequestPayload {
        fn get_headers(&self, key: &str) -> Vec<String> {
            match self.headers.get(key) {
                Some(value) => vec![value.to_string()],
                None => vec![],
            }
        }

        fn get_ip(&self) -> Option<std::net::IpAddr> {
            None
        }

        fn get_path(&self) -> String {
            self.path.clone()
        }

        fn get_query(&self) -> serde_json::Value {
            Value::Null
        }

        fn get_method(&self) -> http::Method {
            self.method.clone()
        }
    }

    struct MockResponsePayload {
        body: Option<Value>,
        status_code: StatusCode,
        headers: Headers,
    }
}
