// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

use std::collections::HashSet;

use http::Method;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CorsAllowOrigin {
    None,
    All,
    Specific(HashSet<String>),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CorsAllowMethods {
    None,
    All,
    Specific(HashSet<Method>),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CorsAllowHeaders {
    None,
    All,
    Specific(HashSet<String>),
}

pub trait HeaderStringProvider {
    fn header_string(&self) -> Option<String>;
}

impl HeaderStringProvider for CorsAllowHeaders {
    fn header_string(&self) -> Option<String> {
        match self {
            CorsAllowHeaders::None => None,
            CorsAllowHeaders::All => Some("*".to_string()),
            CorsAllowHeaders::Specific(headers) => Some(
                headers
                    .iter()
                    .map(|header| header.as_str())
                    .collect::<Vec<&str>>()
                    .join(","),
            ),
        }
    }
}

impl HeaderStringProvider for CorsAllowMethods {
    fn header_string(&self) -> Option<String> {
        match self {
            CorsAllowMethods::None => None,
            CorsAllowMethods::All => Some("*".to_string()),
            CorsAllowMethods::Specific(methods) => Some(
                methods
                    .iter()
                    .map(|method| method.as_str())
                    .collect::<Vec<&str>>()
                    .join(","),
            ),
        }
    }
}
