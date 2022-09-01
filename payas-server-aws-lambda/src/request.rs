use payas_resolver_core::request_context::Request;

// as lambda_http::Request and payas_resolver_core::request_context::Request are in different crates
// from this one, we must wrap the request with our own struct
pub struct LambdaRequest<'a>(&'a lambda_http::Request);

impl<'a> LambdaRequest<'a> {
    pub fn new(req: &'a lambda_http::Request) -> LambdaRequest {
        LambdaRequest(req)
    }
}

impl Request for LambdaRequest<'_> {
    fn get_headers(&self, key: &str) -> Vec<String> {
        self.0
            .headers()
            .get_all(key)
            .into_iter()
            .map(|h| h.to_str().unwrap().to_string())
            .collect()
    }
}
