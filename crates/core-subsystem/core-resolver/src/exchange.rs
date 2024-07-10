use serde_json::Value;

use crate::context::Request;

pub trait Exchange {
    fn get_request(&self) -> &(dyn Request + Send + Sync);
    fn take_body(&mut self) -> Value;
}
