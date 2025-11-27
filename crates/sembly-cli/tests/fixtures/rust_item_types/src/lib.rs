/// Processes an incoming request - should be indexed (function).
///
/// Validates the request, routes it to the appropriate handler,
/// and returns the response.
pub fn process_request() {
    todo!()
}

/// Represents a request - should be indexed (struct).
///
/// Contains the method, path, headers, and body of an
/// incoming request to the service.
pub struct Request {
    /// The request path
    pub path: String,
    /// The request method
    pub method: String,
}

/// Response status codes - should NOT be indexed (enum, not in item_types).
///
/// Standard status codes returned by the service.
pub enum ResponseStatus {
    /// Request succeeded
    Ok,
    /// Client error
    BadRequest,
    /// Server error
    InternalError,
}

/// Handler trait - should NOT be indexed (trait, not in item_types).
///
/// Implement this trait to handle specific endpoints.
pub trait RequestHandler {
    /// Handle the request and return a response
    fn handle(&self, request: &Request) -> ResponseStatus;
}

/// Type alias - should NOT be indexed (type alias, not in item_types).
pub type HandlerResult = Result<(), String>;
