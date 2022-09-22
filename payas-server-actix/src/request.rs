use actix_web::{dev::ConnectionInfo, http::header::HeaderMap, HttpRequest};
use payas_resolver_core::request_context::Request;

pub struct ActixRequest {
    // we cannot refer to HttpRequest directly, as it holds an Rc (and therefore does
    // not impl Send or Sync)
    //
    // request: &'a actix_web::HttpRequest,
    headers: HeaderMap,
    connection_info: ConnectionInfo,
}

impl ActixRequest {
    pub fn from_request(req: HttpRequest) -> ActixRequest {
        ActixRequest {
            headers: req.headers().clone(),
            connection_info: req.connection_info().clone(),
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

    fn get_ip(&self) -> Option<std::net::IpAddr> {
        self.connection_info
            .realip_remote_addr()
            .and_then(|realip| realip.parse().ok())
    }
}
