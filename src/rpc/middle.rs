use http::Method;
use jsonrpc_http_server::{hyper, RequestMiddlewareAction, Response};
use serde_json::json;

pub fn request_middle(request: hyper::Request<hyper::Body>) -> RequestMiddlewareAction {
    if request.method() == Method::GET && request.uri() == "/version" {
        return handle_version(request);
    }

    request.into()
}

fn handle_version(request: hyper::Request<hyper::Body>) -> RequestMiddlewareAction {
    let mut request = request;
    let uri = request.uri_mut();
    *uri = "/net_version".parse().unwrap();
    let params_vec = match serde_json::to_vec(&json!({
        "jsonrpc": "2.0",
        "method": "net_version",
        "params": [],
        "id": "1"
    })) {
        Ok(v) => v,
        Err(e) => return Response::internal_error(e.to_string()).into(),
    };

    let net_version_requesr = hyper::Request::builder()
        .method("POST")
        .uri(uri.clone())
        .header("Content-Type", "application/json")
        .header("Connection", "close")
        .body(hyper::Body::from(params_vec))
        .unwrap();

    net_version_requesr.into()
}
