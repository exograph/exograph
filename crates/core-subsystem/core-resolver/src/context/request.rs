/// Represents a HTTP request from which information can be extracted
pub trait Request {
    // return all header values that have the following key
    fn get_headers(&self, key: &str) -> Vec<String>;

    // return the first header
    fn get_header(&self, key: &str) -> Option<String> {
        self.get_headers(&key.to_lowercase()).get(0).cloned()
    }

    // return the IP address used to make the request
    fn get_ip(&self) -> Option<std::net::IpAddr>;
}
