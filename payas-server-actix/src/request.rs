use actix_web::{http::header::HeaderMap, HttpRequest};
use payas_resolver_core::request_context::Request;

pub struct ActixRequest {
    // we cannot refer to HttpRequest directly, as it holds an Rc (and therefore does
    // not impl Send or Sync)
    //
    // request: &'a actix_web::HttpRequest,
    headers: HeaderMap,
}

impl ActixRequest {
    pub fn from_request(req: HttpRequest) -> ActixRequest {
        ActixRequest {
            headers: req.headers().clone(),
        }
    }
}

impl Request for ActixRequest {
    fn get_headers(&self, key: &str) -> Vec<String> {
        self.headers
            .get_all(key.to_lowercase())
            .into_iter()
            .map(|h| h.to_str().unwrap().to_string())
            .collect()
    }
}
