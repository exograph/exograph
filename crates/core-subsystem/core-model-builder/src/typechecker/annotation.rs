// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

/// Specification for an annotation.
pub struct AnnotationSpec {
    /// List of targets the annotation is allowed to be applied to.
    pub targets: &'static [AnnotationTarget],
    /// Is this annotation allowed to have no parameters?
    pub no_params: bool,
    /// Is this annotation allowed to have a single parameter?
    pub single_params: bool,
    /// List of mapped parameters if mapped parameters are allowed (`None` if not).
    pub mapped_params: Option<&'static [MappedAnnotationParamSpec]>,
}

/// Target for an annotation.
#[derive(Debug, PartialEq, Eq)]
pub enum AnnotationTarget {
    Type,
    Field,
    Argument,
    Module,
    Method,
    Interceptor,
}

/// Specification for a mapped parameter of an annotation.
pub struct MappedAnnotationParamSpec {
    /// Name of the parameter.
    pub name: &'static str,
    /// Is this parameter optional?
    pub optional: bool,
}
