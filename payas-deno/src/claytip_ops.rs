use anyhow::{anyhow, bail, Result};
use deno_core::{error::AnyError, op, OpState};

use serde_json::Value;
use std::{cell::RefCell, rc::Rc};
use tokio::sync::mpsc::Sender;

use crate::clay_execution::{RequestFromDenoMessage, ResponseForDenoMessage};

pub struct InterceptedOperationName(pub Option<String>);

#[op]
pub async fn op_claytip_execute_query(
    state: Rc<RefCell<OpState>>,
    query_string: Value,
    variables: Option<Value>,
) -> Result<Value, AnyError> {
    let state = state.borrow();
    let sender = state.borrow::<Sender<RequestFromDenoMessage>>().to_owned();
    let (response_sender, response_receiver) = tokio::sync::oneshot::channel();

    sender
        .send(RequestFromDenoMessage::ClaytipExecute {
            query_string: query_string.as_str().unwrap().to_string(),
            variables: variables.as_ref().map(|o| o.as_object().unwrap().clone()),
            response_sender,
        })
        .await
        .map_err(|err| {
            anyhow!(
                "Could not send request from op_claytip_execute_query ({})",
                err
            )
        })?;

    if let ResponseForDenoMessage::ClaytipExecute(result) =
        response_receiver.await.map_err(|err| {
            anyhow!(
                "Could not receive result in op_claytip_execute_query ({})",
                err
            )
        })?
    {
        result
    } else {
        bail!("Wrong response type for op_claytip_execute_query")
    }
}

#[op]
pub fn op_intercepted_operation_name(state: &mut OpState) -> Result<String, AnyError> {
    // try to read the intercepted operation name out of Deno's GothamStorage
    if let InterceptedOperationName(Some(name)) = state.borrow() {
        Ok(name.clone())
    } else {
        Err(anyhow!("no stored operation name"))
    }
}

#[op]
pub async fn op_intercepted_proceed(state: Rc<RefCell<OpState>>) -> Result<Value, AnyError> {
    let state = state.borrow();
    let sender = state.borrow::<Sender<RequestFromDenoMessage>>().to_owned();
    let (response_sender, response_receiver) = tokio::sync::oneshot::channel();

    sender
        .send(RequestFromDenoMessage::InterceptedOperationProceed { response_sender })
        .await
        .map_err(|err| {
            anyhow!(
                "Could not send request from op_intercepted_proceed ({})",
                err
            )
        })?;

    if let ResponseForDenoMessage::InterceptedOperationProceed(result) =
        response_receiver.await.map_err(|err| {
            anyhow!(
                "Could not receive result in op_intercepted_proceed ({})",
                err
            )
        })?
    {
        result
    } else {
        bail!("Wrong response type for op_intercepted_proceed")
    }
}
