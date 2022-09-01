use payas_resolver_core::request_context::Request;

// newtype
// trait comment
pub struct LambdaRequest<'a> {
    pub request: &'a lambda_http::Request,
}

impl Request for LambdaRequest<'_> {
    fn get_headers(&self, key: &str) -> Vec<String> {
        self.request
            .headers()
            .get_all(key)
            .into_iter()
            .map(|h| h.to_str().unwrap().to_string())
            .collect()
    }
}
