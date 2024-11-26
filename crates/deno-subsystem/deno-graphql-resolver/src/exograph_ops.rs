// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

use anyhow::{anyhow, bail, Result};
use exo_deno::deno_core::{error::AnyError, op2, OpState};

use core_plugin_interface::core_resolver::system_resolver::SystemResolutionError;
use serde_json::Value;
use std::{cell::RefCell, rc::Rc};
use tokio::sync::mpsc::Sender;

use crate::exo_execution::ExographMethodResponse;

use super::exo_execution::{RequestFromDenoMessage, ResponseForDenoMessage};

#[derive(Debug)]
pub struct InterceptedOperationInfo {
    pub name: String,
    pub query: Value,
}

pub async fn op_exograph_execute_query_helper(
    state: Rc<RefCell<OpState>>,
    query_string: Value,
    variables: Option<Value>,
    context_override: Value,
) -> Result<Value, AnyError> {
    let (response_sender, response_receiver) = tokio::sync::oneshot::channel();

    let sender = {
        let state = state.borrow();
        state.borrow::<Sender<RequestFromDenoMessage>>().to_owned()
    };

    sender
        .send(RequestFromDenoMessage::ExographExecute {
            query_string: query_string.as_str().unwrap().to_string(),
            variables: variables.as_ref().map(|o| o.as_object().unwrap().clone()),
            context_override,
            response_sender,
        })
        .await
        .map_err(|err| {
            anyhow!(
                "Could not send request from op_exograph_execute_query ({})",
                err
            )
        })?;

    if let ResponseForDenoMessage::ExographExecute(result) =
        response_receiver.await.map_err(|err| {
            anyhow!(
                "Could not receive result in op_exograph_execute_query ({})",
                err
            )
        })?
    {
        let result = process_execution_error(result)?;

        for (header, value) in result.headers.into_iter() {
            let mut state = state.borrow_mut();

            add_header(&mut state, header, value)?
        }

        Ok(result.body.to_json()?)
    } else {
        bail!("Wrong response type for op_exograph_execute_query")
    }
}

#[op2(async)]
#[serde]
pub async fn op_exograph_execute_query(
    state: Rc<RefCell<OpState>>,
    #[serde] query_string: serde_json::Value,
    #[serde] variables: Option<serde_json::Value>,
) -> Result<serde_json::Value, AnyError> {
    op_exograph_execute_query_helper(state, query_string, variables, Value::Null).await
}

#[op2(async)]
#[serde]
pub async fn op_exograph_execute_query_priv(
    state: Rc<RefCell<OpState>>,
    #[serde] query_string: serde_json::Value,
    #[serde] variables: Option<serde_json::Value>,
    #[serde] context_override: serde_json::Value,
) -> Result<serde_json::Value, AnyError> {
    op_exograph_execute_query_helper(state, query_string, variables, context_override).await
}

#[op2]
#[string]
pub fn op_exograph_version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

#[op2]
#[string]
pub fn op_operation_name(state: &mut OpState) -> Result<String, AnyError> {
    // try to read the intercepted operation name out of Deno's GothamStorage
    if let Some(InterceptedOperationInfo { name, .. }) = state.borrow() {
        Ok(name.clone())
    } else {
        Err(anyhow!("no stored operation name"))
    }
}

#[op2]
#[serde]
pub fn op_operation_query(state: &mut OpState) -> Result<serde_json::Value, AnyError> {
    if let Some(InterceptedOperationInfo { query, .. }) = state.borrow() {
        Ok(query.to_owned())
    } else {
        Err(anyhow!("no stored operation query"))
    }
}

#[op2(async)]
#[serde]
pub async fn op_operation_proceed(
    state: Rc<RefCell<OpState>>,
) -> Result<serde_json::Value, AnyError> {
    let (response_sender, response_receiver) = tokio::sync::oneshot::channel();

    let sender = {
        let state = state.borrow();
        state.borrow::<Sender<RequestFromDenoMessage>>().to_owned()
    };

    sender
        .send(RequestFromDenoMessage::InterceptedOperationProceed { response_sender })
        .await
        .map_err(|err| anyhow!("Could not send request from op_operation_proceed ({})", err))?;

    if let ResponseForDenoMessage::InterceptedOperationProceed(result) = response_receiver
        .await
        .map_err(|err| anyhow!("Could not receive result in op_operation_proceed ({})", err))?
    {
        let result = process_execution_error(result)?;

        for (header, value) in result.headers.into_iter() {
            let mut state = state.borrow_mut();
            add_header(&mut state, header, value)?
        }

        Ok(result.body.to_json()?)
    } else {
        bail!("Wrong response type for op_operation_proceed")
    }
}

// add a header to ExographMethodResponse;
// this is eventually returned to Exograph through execute_and_get_r
pub fn add_header(state: &mut OpState, header: String, value: String) -> Result<(), AnyError> {
    // get response object out of GothamStorage
    // if no object exists, create one
    let mut response: ExographMethodResponse = state.try_take().unwrap_or_default();

    // add header
    response.headers.push((header, value));

    // put back response object
    state.put(response);

    Ok(())
}

#[op2]
#[serde]
pub fn op_exograph_add_header(
    state: &mut OpState,
    #[string] header: String,
    #[string] value: String,
) -> Result<(), AnyError> {
    add_header(state, header, value)
}

// We need to propagate the explicit error if any. So here we check if the error has an explicit message (i.e. message
// thrown using ExographError) and if so, we throw a custom error with the message.
//
// Without this logic, the original error will be lost and a generic "Internal server error" will be sent to the client.
fn process_execution_error<T>(result: Result<T, SystemResolutionError>) -> Result<T, AnyError> {
    result.map_err(|err| match err.explicit_message() {
        Some(msg) => anyhow!(deno_core::error::custom_error("ExographError", msg)),
        None => anyhow!(err),
    })
}
