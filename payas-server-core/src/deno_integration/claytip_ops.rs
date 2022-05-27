use anyhow::{anyhow, bail, Result};
use deno_core::{error::AnyError, op, OpState};

use serde_json::Value;
use std::{cell::RefCell, rc::Rc};
use tokio::sync::mpsc::Sender;

use super::clay_execution::{
    ClaytipMethodResponse, RequestFromDenoMessage, ResponseForDenoMessage,
};

#[derive(Debug)]
pub struct InterceptedOperationName(pub String);

#[op]
pub async fn op_claytip_execute_query(
    state: Rc<RefCell<OpState>>,
    query_string: Value,
    variables: Option<Value>,
) -> Result<Value, AnyError> {
    let mut state = state.borrow_mut();
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
        let result = result?;

        for (header, value) in result.headers.into_iter() {
            add_header(&mut state, header, value)?
        }

        Ok(result.body.to_json()?)
    } else {
        bail!("Wrong response type for op_claytip_execute_query")
    }
}

#[op]
pub fn op_intercepted_operation_name(state: &mut OpState) -> Result<String, AnyError> {
    // try to read the intercepted operation name out of Deno's GothamStorage
    if let Some(InterceptedOperationName(name)) = state.borrow() {
        Ok(name.clone())
    } else {
        Err(anyhow!("no stored operation name"))
    }
}

#[op]
pub async fn op_intercepted_proceed(state: Rc<RefCell<OpState>>) -> Result<Value, AnyError> {
    let mut state = state.borrow_mut();
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
        let result = result?;

        for (header, value) in result.headers.into_iter() {
            add_header(&mut state, header, value)?
        }

        Ok(result.body.to_json()?)
    } else {
        bail!("Wrong response type for op_intercepted_proceed")
    }
}

// add a header to ClaytipMethodResponse;
// this is eventually returned to Claytip through execute_and_get_r
pub fn add_header(state: &mut OpState, header: String, value: String) -> Result<(), AnyError> {
    // get response object out of GothamStorage
    // if no object exists, create one
    let mut response = if let Some(resp @ ClaytipMethodResponse { .. }) = state.try_take() {
        resp
    } else {
        ClaytipMethodResponse::default()
    };

    // add header
    response.headers.push((header, value));

    // put back response object
    state.put(response);

    Ok(())
}

#[op]
pub fn op_add_header(state: &mut OpState, header: String, value: String) -> Result<(), AnyError> {
    add_header(state, header, value)
}
