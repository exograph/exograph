// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

//! Shared utilities for mapping a create's data between GraphQL and RPC resolvers.

use common::context::{ContextExtractionError, RequestContext};
use common::value::Val;
use core_resolver::context_extractor::ContextExtractor;
use postgres_core_model::{
    subsystem::PostgresCoreSubsystem,
    types::{PostgresField, PostgresFieldDefaultValue},
};

use crate::predicate_util::get_argument_field;

/// The value a create stores for a field
#[derive(Clone, Copy)]
pub enum CreateFieldValue<'a> {
    /// The value in the create's data
    Supplied(&'a Val),
    /// The field's context-based default (such as `AuthContext.id`)
    ContextDefault(&'a Val),
}

impl<'a> CreateFieldValue<'a> {
    pub fn value(&self) -> &'a Val {
        match self {
            CreateFieldValue::Supplied(value) | CreateFieldValue::ContextDefault(value) => value,
        }
    }

    /// For a many-to-one field, the key for the target's pk field `pk_field_name`. A supplied
    /// reference has the key as a field (`{id: 5}`), but a context default is the key itself (`5`).
    pub fn reference_key(&self, pk_field_name: &str) -> Option<&'a Val> {
        match self {
            CreateFieldValue::Supplied(reference) => get_argument_field(reference, pk_field_name),
            CreateFieldValue::ContextDefault(key) => Some(key),
        }
    }
}

/// The value a create stores for `field`: the value in `data` or, when `data` omits the field, the
/// field's context-based default.
///
/// The database fills a static, function, or `autoIncrement()` default, but it can't fill a
/// context-based one, so the resolver must. The access check also assumes that the row holds that
/// default.
pub async fn create_field_value<'a, CT>(
    field: &PostgresField<CT>,
    data: &'a Val,
    core_subsystem: &PostgresCoreSubsystem,
    request_context: &'a RequestContext<'a>,
) -> Result<Option<CreateFieldValue<'a>>, ContextExtractionError> {
    if let Some(value) = get_argument_field(data, &field.name) {
        return Ok(Some(CreateFieldValue::Supplied(value)));
    }

    match &field.default_value {
        Some(PostgresFieldDefaultValue::Dynamic(selection)) => Ok(core_subsystem
            .extract_context_selection(request_context, selection)
            .await?
            .map(CreateFieldValue::ContextDefault)),
        _ => Ok(None),
    }
}
