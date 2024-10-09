// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

use http::Method;

use super::{CorsAllowHeaders, CorsAllowMethods, CorsAllowOrigin};

/// CORS configuration for a router
///
/// The CORS configuration is used to determine the CORS headers to add to a response
/// given the origin and the method of the request.
///
/// Current implementation mainly targets the Exograph use case (especially, the way it
/// treats env variables and sets default values)
#[derive(Debug, Clone)]
pub struct CorsConfig {
    pub allow_origin: CorsAllowOrigin,
    pub allow_methods: CorsAllowMethods,
    pub allow_headers: CorsAllowHeaders,
    pub max_age_seconds: Option<u32>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CorsResponse<'a> {
    Allow(&'a str), // Allow and add CORS headers with the given origin
    NoCorsHeaders,  // Allow but do not add CORS headers
    Deny,           // Deny (and do not add CORS headers
}

impl CorsConfig {
    pub fn default() -> Self {
        Self {
            allow_origin: CorsAllowOrigin::None,
            allow_methods: CorsAllowMethods::None,
            allow_headers: CorsAllowHeaders::None,
            max_age_seconds: None,
        }
    }

    /// Create a CorsConfig from a list of domains
    ///
    /// The domains list is a comma-separated list of domains, with special values:
    /// - `*` means all origins are allowed.
    /// - `""` (empty string) means no origins are allowed.
    pub fn from_env(domains_list: Option<String>) -> Self {
        let allow_origin = match domains_list {
            Some(domains) => match domains.trim() {
                "*" => CorsAllowOrigin::All,
                "" => CorsAllowOrigin::None,
                _ => CorsAllowOrigin::Specific(
                    domains
                        .split(',')
                        .map(|domain| domain.trim().to_string())
                        .collect(),
                ),
            },
            None => CorsAllowOrigin::None,
        };

        Self {
            allow_origin,
            allow_methods: CorsAllowMethods::All,
            allow_headers: CorsAllowHeaders::All,
            max_age_seconds: Some(3600),
        }
    }

    pub fn with_allow_origin(mut self, allow_origin: CorsAllowOrigin) -> Self {
        self.allow_origin = allow_origin;
        self
    }

    pub fn with_allow_methods(mut self, allow_methods: CorsAllowMethods) -> Self {
        self.allow_methods = allow_methods;
        self
    }

    pub fn with_allow_headers(mut self, allow_headers: CorsAllowHeaders) -> Self {
        self.allow_headers = allow_headers;
        self
    }

    pub fn with_max_age_seconds(mut self, max_age_seconds: Option<u32>) -> Self {
        self.max_age_seconds = max_age_seconds;
        self
    }

    /// Determine the CORS response for a given origin and method
    ///
    /// From the CORS spec:
    /// "A CORS request is an HTTP request that includes an `Origin` header.
    /// It cannot be reliably identified as participating in the CORS protocol
    /// as the `Origin` header is also included for all requests whose method
    /// is neither `GET` nor `HEAD`.".
    ///
    /// Therefore, we allow all requests that do not have an `Origin` header, but
    /// indicate that CORS headers should not be added.
    ///
    /// Note that even for same-origin requests, for non-GET/HEAD, the browser sends
    /// an `Origin` header, and we must allow this request (but not add CORS headers).
    ///
    /// Here are a few examples (assume that the CORS config allows "https://a.com" as the only origin).
    /// Also for brevity, we assume `Allow` to mean Allow(<origin>).
    ///
    /// - GET (Origin: none) -> Allow (not a CORS request)
    /// - HEAD (Origin: none) -> Allow (not a CORS request)
    /// - OPTIONS (Origin: none) -> Deny (OPTIONS request cannot be made without an Origin)
    ///
    /// - GET (Origin: "https://a.com") -> Allow (in CORS config)
    /// - GET (Origin: "https://b.com") -> NoCorsHeaders (may be same-origin and
    ///                                                   if not, the browser will enforce CORS due to the lack of CORS headers)
    ///
    /// - POST (Origin: "https://a.com") -> Allow (in CORS config)
    /// - POST (Origin: "https://b.com") -> NoCorsHeaders (must be same-origin)
    pub fn allow_origin<'a>(&self, origin: Option<&'a str>, method: &Method) -> CorsResponse<'a> {
        let deny_response = || match method {
            &Method::OPTIONS => CorsResponse::Deny,
            _ => CorsResponse::NoCorsHeaders,
        };

        match origin {
            Some(origin) => match &self.allow_origin {
                CorsAllowOrigin::All => CorsResponse::Allow(origin),
                CorsAllowOrigin::Specific(allowed_origins) => {
                    if allowed_origins.contains(&origin.to_string()) {
                        CorsResponse::Allow(origin)
                    } else {
                        deny_response()
                    }
                }
                CorsAllowOrigin::None => deny_response(),
            },
            None => CorsResponse::NoCorsHeaders,
        }
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use std::collections::HashSet;

    use super::*;

    #[test]
    fn cors_config_from_env_specific() {
        let config = CorsConfig::from_env(Some("https://example.com".to_string()));
        assert_eq!(
            config.allow_origin,
            CorsAllowOrigin::Specific(HashSet::from(["https://example.com".to_string()]))
        );

        for config_string in [
            // space variations
            "https://example.com,https://example.org",
            "https://example.com, https://example.org",
            " https://example.com,https://example.org",
            "https://example.com,https://example.org ",
        ] {
            let config = CorsConfig::from_env(Some(config_string.to_string()));
            assert_eq!(
                config.allow_origin,
                CorsAllowOrigin::Specific(HashSet::from([
                    "https://example.com".to_string(),
                    "https://example.org".to_string()
                ]))
            );
        }

        let config = CorsConfig::from_env(Some("".to_string()));
        assert_eq!(config.allow_origin, CorsAllowOrigin::None);
    }

    #[test]
    fn cors_config_from_env_all() {
        let config = CorsConfig::from_env(Some("*".to_string()));
        assert_eq!(config.allow_origin, CorsAllowOrigin::All);
    }

    #[test]
    fn cors_config_from_env_none() {
        let config = CorsConfig::from_env(None);
        assert_eq!(config.allow_origin, CorsAllowOrigin::None);
    }

    pub(crate) const NON_PREFLIGHT_METHODS: [Method; 6] = [
        Method::GET,
        Method::HEAD,
        Method::POST,
        Method::PUT,
        Method::DELETE,
        Method::PATCH,
    ];

    fn assert_cors_enforcement(
        config: &CorsConfig,
        matching_origin: Option<&str>,
        mismatch_origin: Option<&str>,
    ) {
        for method in NON_PREFLIGHT_METHODS {
            if let Some(matching_origin) = matching_origin {
                assert_eq!(
                    config.allow_origin(Some(matching_origin), &method),
                    CorsResponse::Allow(matching_origin)
                );
            }

            assert_eq!(
                config.allow_origin(None, &method),
                CorsResponse::NoCorsHeaders
            );

            if let Some(mismatch_origin) = mismatch_origin {
                assert_eq!(
                    config.allow_origin(Some(mismatch_origin), &method),
                    CorsResponse::NoCorsHeaders
                );
            }
        }

        if let Some(mismatch_origin) = mismatch_origin {
            assert_eq!(
                config.allow_origin(Some(mismatch_origin), &Method::OPTIONS),
                CorsResponse::Deny
            );
        }
    }

    #[test]
    fn cors_config_allow_origin_specific() {
        let single_allowed_domain_config =
            CorsConfig::from_env(Some("https://example.com".to_string()));
        assert_cors_enforcement(
            &single_allowed_domain_config,
            Some("https://example.com"),
            Some("https://not-example.com"),
        );

        let multiple_allowed_domains_config =
            CorsConfig::from_env(Some("https://example.com,https://example.org".to_string()));
        assert_cors_enforcement(
            &multiple_allowed_domains_config,
            Some("https://example.com"),
            Some("https://not-example.com"),
        );
        assert_cors_enforcement(
            &multiple_allowed_domains_config,
            Some("https://example.org"),
            Some("https://not-example.org"),
        );
    }

    #[test]
    fn cors_config_allow_origin_all() {
        let config = CorsConfig::from_env(Some("*".to_string()));
        assert_cors_enforcement(&config, Some("https://example.com"), None);
    }

    #[test]
    fn cors_config_allow_origin_none() {
        let config = CorsConfig::from_env(None);
        assert_cors_enforcement(&config, None, Some("https://example.com"));
    }
}
